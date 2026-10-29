#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 1 ]; then
    echo "usage: $0 <firmware-binary> [max-bytes]" >&2
    exit 2
fi

binary="$1"
max_bytes="${2:-4096}"
size="$(wc -c < "$binary")"

if [ "$size" -gt "$max_bytes" ]; then
    echo "firmware size ${size} bytes exceeds limit ${max_bytes} bytes" >&2
    exit 1
fi

echo "firmware size ${size} bytes is within limit ${max_bytes} bytes"
