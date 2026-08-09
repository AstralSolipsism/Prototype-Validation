#![forbid(unsafe_code)]

use authoritative_world_core::{
    AuthorityCommandEnvelope, AuthorityError, AuthorityServer, CommandProcessResult,
    CommandReceipt, ReceiptDisposition, WorldRevision, WorldState,
};
use authority_persistence::{FilePersistenceStore, PersistenceError};
use replay_core::StateFingerprint;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, BufWriter, Write},
    net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use thiserror::Error;
use world_ids::{CommandId, SessionId, SnapshotId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireRequest {
    Command(AuthorityCommandEnvelope),
    SnapshotNow { snapshot_id: SnapshotId },
    Inspect,
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireResponse {
    Receipts(Vec<CommandReceipt>),
    Buffered {
        session_id: SessionId,
        received_sequence: u64,
        expected_sequence: u64,
    },
    StaleSequence {
        session_id: SessionId,
        received_sequence: u64,
        expected_sequence: u64,
    },
    SnapshotWritten {
        revision: WorldRevision,
        state_fingerprint: StateFingerprint,
    },
    Summary(ServerSummary),
    Ack,
    Error {
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerSummary {
    pub revision: WorldRevision,
    pub state_fingerprint: StateFingerprint,
    pub connected_sessions: usize,
    pub next_journal_sequence: u64,
}

#[derive(Clone, Debug, Default)]
pub struct OrderedIngress {
    next_sequence: BTreeMap<SessionId, u64>,
    processed_sequences: BTreeMap<(SessionId, u64), CommandId>,
    pending: BTreeMap<(SessionId, u64), AuthorityCommandEnvelope>,
}

#[derive(Clone, Debug)]
pub enum IngressOutcome {
    Processed(Vec<CommandProcessResult>),
    Buffered {
        session_id: SessionId,
        received_sequence: u64,
        expected_sequence: u64,
    },
    StaleSequence {
        session_id: SessionId,
        received_sequence: u64,
        expected_sequence: u64,
    },
}

impl OrderedIngress {
    pub fn ingest(
        &mut self,
        server: &mut AuthorityServer,
        envelope: AuthorityCommandEnvelope,
    ) -> Result<IngressOutcome, AuthorityError> {
        let session_id = envelope.audit.session_id;
        let sequence = envelope.audit.request_sequence;
        let expected = *self.next_sequence.entry(session_id).or_insert(0);

        if sequence < expected {
            let key = (session_id, sequence);
            if self.processed_sequences.get(&key) == Some(&envelope.command_id) {
                return Ok(IngressOutcome::Processed(vec![server.process(envelope)?]));
            }
            return Ok(IngressOutcome::StaleSequence {
                session_id,
                received_sequence: sequence,
                expected_sequence: expected,
            });
        }

        if sequence > expected {
            let key = (session_id, sequence);
            match self.pending.get(&key) {
                Some(existing) if existing.command_id != envelope.command_id => {
                    return Ok(IngressOutcome::StaleSequence {
                        session_id,
                        received_sequence: sequence,
                        expected_sequence: expected,
                    });
                }
                Some(_) => {}
                None => {
                    self.pending.insert(key, envelope);
                }
            }
            return Ok(IngressOutcome::Buffered {
                session_id,
                received_sequence: sequence,
                expected_sequence: expected,
            });
        }

        let mut results = Vec::new();
        self.process_in_order(server, envelope, &mut results)?;
        loop {
            let next = self.next_sequence[&session_id];
            let Some(pending) = self.pending.remove(&(session_id, next)) else {
                break;
            };
            self.process_in_order(server, pending, &mut results)?;
        }
        Ok(IngressOutcome::Processed(results))
    }

    fn process_in_order(
        &mut self,
        server: &mut AuthorityServer,
        envelope: AuthorityCommandEnvelope,
        results: &mut Vec<CommandProcessResult>,
    ) -> Result<(), AuthorityError> {
        let session_id = envelope.audit.session_id;
        let sequence = envelope.audit.request_sequence;
        let result = server.process(envelope.clone())?;
        self.processed_sequences
            .insert((session_id, sequence), envelope.command_id);
        self.next_sequence.insert(session_id, sequence + 1);
        results.push(result);
        Ok(())
    }
}

#[derive(Clone)]
struct ServerHost {
    server: AuthorityServer,
    ingress: OrderedIngress,
    store: FilePersistenceStore,
}

impl ServerHost {
    fn summary(&self) -> Result<ServerSummary, TransportError> {
        Ok(ServerSummary {
            revision: self.server.world().revision,
            state_fingerprint: self.server.world().semantic_fingerprint()?,
            connected_sessions: self.server.sessions().len(),
            next_journal_sequence: self.server.next_journal_sequence(),
        })
    }

    fn handle_command(
        &mut self,
        envelope: AuthorityCommandEnvelope,
    ) -> Result<WireResponse, TransportError> {
        let mut candidate_server = self.server.clone();
        let mut candidate_ingress = self.ingress.clone();
        let outcome = candidate_ingress.ingest(&mut candidate_server, envelope)?;
        match outcome {
            IngressOutcome::Processed(results) => {
                let records = results
                    .iter()
                    .filter_map(|result| result.journal_record.clone())
                    .collect::<Vec<_>>();
                self.store.append_records_transactional(&records)?;
                self.server = candidate_server;
                self.ingress = candidate_ingress;
                Ok(WireResponse::Receipts(
                    results.into_iter().map(|result| result.receipt).collect(),
                ))
            }
            IngressOutcome::Buffered {
                session_id,
                received_sequence,
                expected_sequence,
            } => {
                self.ingress = candidate_ingress;
                Ok(WireResponse::Buffered {
                    session_id,
                    received_sequence,
                    expected_sequence,
                })
            }
            IngressOutcome::StaleSequence {
                session_id,
                received_sequence,
                expected_sequence,
            } => Ok(WireResponse::StaleSequence {
                session_id,
                received_sequence,
                expected_sequence,
            }),
        }
    }
}

pub struct RunningAuthorityServer {
    address: SocketAddr,
    running: Arc<AtomicBool>,
    host: Arc<Mutex<ServerHost>>,
    join: Option<JoinHandle<()>>,
}

impl RunningAuthorityServer {
    pub fn start(
        bind: impl ToSocketAddrs,
        baseline: WorldState,
        store: FilePersistenceStore,
        recover: bool,
    ) -> Result<Self, TransportError> {
        store.ensure_ready()?;
        let server = if recover {
            store.recover(baseline)?
        } else {
            AuthorityServer::new(baseline)?
        };
        let listener = TcpListener::bind(bind)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let running = Arc::new(AtomicBool::new(true));
        let host = Arc::new(Mutex::new(ServerHost {
            server,
            ingress: OrderedIngress::default(),
            store,
        }));
        let thread_running = Arc::clone(&running);
        let thread_host = Arc::clone(&host);
        let join = thread::spawn(move || {
            while thread_running.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let connection_host = Arc::clone(&thread_host);
                        let connection_running = Arc::clone(&thread_running);
                        thread::spawn(move || {
                            if let Err(error) =
                                handle_connection(stream, connection_host, connection_running)
                            {
                                eprintln!("P5 authority connection ended: {error}");
                            }
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(8));
                    }
                    Err(error) => {
                        eprintln!("P5 authority accept failed: {error}");
                        break;
                    }
                }
            }
        });
        Ok(Self {
            address,
            running,
            host,
            join: Some(join),
        })
    }

    pub const fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn summary(&self) -> Result<ServerSummary, TransportError> {
        self.host
            .lock()
            .map_err(|_| TransportError::Poisoned)?
            .summary()
    }

    pub fn write_snapshot(&self, snapshot_id: SnapshotId) -> Result<ServerSummary, TransportError> {
        let host = self.host.lock().map_err(|_| TransportError::Poisoned)?;
        host.store.write_snapshot(&host.server, snapshot_id)?;
        host.summary()
    }

    pub fn stop(mut self) -> Result<ServerSummary, TransportError> {
        let summary = self.summary()?;
        self.running.store(false, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_millis(200));
        if let Some(join) = self.join.take() {
            join.join().map_err(|_| TransportError::ThreadPanicked)?;
        }
        Ok(summary)
    }
}

fn handle_connection(
    stream: TcpStream,
    host: Arc<Mutex<ServerHost>>,
    running: Arc<AtomicBool>,
) -> Result<(), TransportError> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let reader_stream = stream.try_clone()?;
    let mut reader = BufReader::new(reader_stream);
    let mut writer = BufWriter::new(stream);
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(());
        }
        let request: WireRequest = serde_json::from_str(line.trim())?;
        let response = match request {
            WireRequest::Command(envelope) => host
                .lock()
                .map_err(|_| TransportError::Poisoned)?
                .handle_command(envelope),
            WireRequest::SnapshotNow { snapshot_id } => {
                let host = host.lock().map_err(|_| TransportError::Poisoned)?;
                let snapshot = host.store.write_snapshot(&host.server, snapshot_id)?;
                Ok(WireResponse::SnapshotWritten {
                    revision: snapshot.world.revision,
                    state_fingerprint: snapshot.state_fingerprint,
                })
            }
            WireRequest::Inspect => host
                .lock()
                .map_err(|_| TransportError::Poisoned)?
                .summary()
                .map(WireResponse::Summary),
            WireRequest::Shutdown => {
                running.store(false, Ordering::SeqCst);
                Ok(WireResponse::Ack)
            }
        }
        .unwrap_or_else(|error| WireResponse::Error {
            message: error.to_string(),
        });
        serde_json::to_writer(&mut writer, &response)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        if matches!(response, WireResponse::Ack) && !running.load(Ordering::SeqCst) {
            return Ok(());
        }
    }
}

pub struct TcpAuthorityClient {
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
}

impl TcpAuthorityClient {
    pub fn connect(address: SocketAddr) -> Result<Self, TransportError> {
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        let reader = BufReader::new(stream.try_clone()?);
        let writer = BufWriter::new(stream);
        Ok(Self { reader, writer })
    }

    pub fn request(&mut self, request: &WireRequest) -> Result<WireResponse, TransportError> {
        serde_json::to_writer(&mut self.writer, request)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        if line.is_empty() {
            return Err(TransportError::Disconnected);
        }
        Ok(serde_json::from_str(line.trim())?)
    }

    pub fn command(
        &mut self,
        envelope: AuthorityCommandEnvelope,
    ) -> Result<Vec<CommandReceipt>, TransportError> {
        match self.request(&WireRequest::Command(envelope))? {
            WireResponse::Receipts(receipts) => Ok(receipts),
            WireResponse::Buffered { .. } => Ok(Vec::new()),
            WireResponse::StaleSequence { .. } => Err(TransportError::StaleSequence),
            WireResponse::Error { message } => Err(TransportError::Remote(message)),
            other => Err(TransportError::Unexpected(format!("{other:?}"))),
        }
    }
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON transport failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("authority command failed: {0}")]
    Authority(#[from] AuthorityError),
    #[error("persistence failed: {0}")]
    Persistence(#[from] PersistenceError),
    #[error("authority mutex was poisoned")]
    Poisoned,
    #[error("authority server thread panicked")]
    ThreadPanicked,
    #[error("connection closed before a response")]
    Disconnected,
    #[error("stale request sequence was rejected")]
    StaleSequence,
    #[error("remote authority error: {0}")]
    Remote(String),
    #[error("unexpected wire response: {0}")]
    Unexpected(String),
}

pub fn receipt_is_duplicate(receipt: &CommandReceipt) -> bool {
    receipt.disposition == ReceiptDisposition::Duplicate
}
