#!/usr/bin/env bash
# Sketch / docs helper — prefer ./fetch.sh for real downloads.
#
# Cited: jsmolka/gba-tests (MIT) prebuilt .gba layout
#   https://github.com/jsmolka/gba-tests
# Note: prints curl examples only. Never fetches BIOS or commercial ROMs.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
PIN="${JSMOLKA_REF:-a7113b67e63f83a9b321696ddd7042ccfad6c881}"
BASE_URL="https://raw.githubusercontent.com/jsmolka/gba-tests/${PIN}"

echo "fetch-sketch: curl examples for MIT prebuilts into ${ROOT}/{arm,thumb,memory}/"
echo "  pin: ${PIN}"
echo "  preferred: ${ROOT}/fetch.sh"
echo "  example (manual):"
echo "    curl -fsSL ${BASE_URL}/arm/arm.gba       -o ${ROOT}/arm/arm.gba"
echo "    curl -fsSL ${BASE_URL}/thumb/thumb.gba   -o ${ROOT}/thumb/thumb.gba"
echo "    curl -fsSL ${BASE_URL}/memory/memory.gba -o ${ROOT}/memory/memory.gba"
echo "refusing to download (exit 0). Use fetch.sh to actually vendor."
exit 0
