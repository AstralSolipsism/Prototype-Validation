#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/validate_architecture.py
python3 scripts/validate_workspace.py
python3 scripts/validate_test_vectors.py
python3 -m compileall -q scripts
printf 'Static preproduction checks passed.\n'
