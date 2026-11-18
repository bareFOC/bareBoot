#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/bareflash"
exec cargo run -p bareboot-tui "$@"