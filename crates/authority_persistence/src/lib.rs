#![forbid(unsafe_code)]

use authoritative_world_core::{
    AuthorityError, AuthorityServer, AuthoritySnapshot, JournalRecord, WorldState,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;
use world_ids::SnapshotId;

#[derive(Clone, Debug)]
pub struct FilePersistenceStore {
    root: PathBuf,
}

impl FilePersistenceStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn snapshot_path(&self) -> PathBuf {
        self.root.join("region-snapshot.json")
    }

    pub fn journal_path(&self) -> PathBuf {
        self.root.join("command-journal.ndjson")
    }

    pub fn reset(&self) -> Result<(), PersistenceError> {
        if self.root.exists() {
            fs::remove_dir_all(&self.root)?;
        }
        fs::create_dir_all(&self.root)?;
        Ok(())
    }

    pub fn ensure_ready(&self) -> Result<(), PersistenceError> {
        fs::create_dir_all(&self.root)?;
        Ok(())
    }

    pub fn append_record(&self, record: &JournalRecord) -> Result<(), PersistenceError> {
        self.ensure_ready()?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.journal_path())?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, record)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        writer.get_ref().sync_data()?;
        Ok(())
    }

    pub fn append_records_transactional(
        &self,
        additions: &[JournalRecord],
    ) -> Result<(), PersistenceError> {
        if additions.is_empty() {
            return Ok(());
        }
        self.ensure_ready()?;
        let mut records = self.load_journal()?;
        let first_expected = records
            .last()
            .map_or(0, |record| record.sequence.saturating_add(1));
        for (expected, record) in (first_expected..).zip(additions.iter()) {
            if record.sequence != expected {
                return Err(PersistenceError::JournalSequence {
                    expected,
                    actual: record.sequence,
                });
            }
            records.push(record.clone());
        }

        let target = self.journal_path();
        let temporary = self.root.join("command-journal.ndjson.tmp");
        {
            let file = File::create(&temporary)?;
            let mut writer = BufWriter::new(file);
            for record in &records {
                serde_json::to_writer(&mut writer, record)?;
                writer.write_all(b"\n")?;
            }
            writer.flush()?;
            writer.get_ref().sync_all()?;
        }
        if target.exists() {
            fs::remove_file(&target)?;
        }
        fs::rename(&temporary, &target)?;
        Ok(())
    }

    pub fn write_snapshot(
        &self,
        server: &AuthorityServer,
        snapshot_id: SnapshotId,
    ) -> Result<AuthoritySnapshot, PersistenceError> {
        self.ensure_ready()?;
        let snapshot = server.snapshot(snapshot_id)?;
        let target = self.snapshot_path();
        let temporary = self.root.join("region-snapshot.json.tmp");
        {
            let file = File::create(&temporary)?;
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, &snapshot)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
            writer.get_ref().sync_all()?;
        }
        if target.exists() {
            fs::remove_file(&target)?;
        }
        fs::rename(&temporary, &target)?;
        Ok(snapshot)
    }

    pub fn load_snapshot(&self) -> Result<Option<AuthoritySnapshot>, PersistenceError> {
        let path = self.snapshot_path();
        if !path.exists() {
            return Ok(None);
        }
        let file = File::open(path)?;
        Ok(Some(serde_json::from_reader(BufReader::new(file))?))
    }

    pub fn load_journal(&self) -> Result<Vec<JournalRecord>, PersistenceError> {
        let path = self.journal_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let reader = BufReader::new(File::open(path)?);
        let mut records = Vec::new();
        for (line_index, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let record =
                serde_json::from_str(&line).map_err(|source| PersistenceError::JournalLine {
                    line: line_index + 1,
                    source,
                })?;
            records.push(record);
        }
        records.sort_by_key(|record: &JournalRecord| record.sequence);
        Ok(records)
    }

    pub fn recover(&self, baseline: WorldState) -> Result<AuthorityServer, PersistenceError> {
        self.ensure_ready()?;
        let mut server = match self.load_snapshot()? {
            Some(snapshot) => AuthorityServer::from_snapshot(snapshot)?,
            None => AuthorityServer::new(baseline)?,
        };
        for record in self.load_journal()? {
            if record.sequence < server.next_journal_sequence() {
                continue;
            }
            server.apply_journal_record(record)?;
        }
        Ok(server)
    }
}

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON persistence failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("authority recovery failed: {0}")]
    Authority(#[from] AuthorityError),
    #[error("invalid journal JSON at line {line}: {source}")]
    JournalLine {
        line: usize,
        source: serde_json::Error,
    },
    #[error("journal sequence mismatch: expected {expected}, actual {actual}")]
    JournalSequence { expected: u64, actual: u64 },
}
