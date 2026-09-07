#!/bin/bash
# Checks the project-owned published-documentation verifier without network IO.
set -euo pipefail

PROJECT_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
exec python3 -B -m unittest discover -s "$PROJECT_ROOT/scripts/tests" -v
