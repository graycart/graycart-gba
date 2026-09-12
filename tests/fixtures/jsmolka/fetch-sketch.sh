#!/usr/bin/env bash
# Sketch only — do not run in CI. Workstream C will harden this.
#
# Cited: jsmolka/gba-tests (MIT) prebuilt .gba layout
#   https://github.com/jsmolka/gba-tests
# Note: fetches upstream MIT prebuilts into path stubs; never fetches BIOS or
# commercial ROMs. Pin a commit SHA before treating as a gate.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
# Replace with a pinned commit once workstream C lands.
UPSTREAM_REF="${JSMOLKA_REF:-master}"
BASE_URL="https://raw.githubusercontent.com/jsmolka/gba-tests/${UPSTREAM_REF}"

echo "fetch-sketch: would download MIT prebuilts into ${ROOT}/{arm,thumb,memory}/"
echo "  pin JSMOLKA_REF=<git-sha> before relying on hashes"
echo "  example (manual):"
echo "    curl -fsSL ${BASE_URL}/arm/arm.gba       -o ${ROOT}/arm/arm.gba"
echo "    curl -fsSL ${BASE_URL}/thumb/thumb.gba   -o ${ROOT}/thumb/thumb.gba"
echo "    curl -fsSL ${BASE_URL}/memory/memory.gba -o ${ROOT}/memory/memory.gba"
echo "refusing to download automatically in this sketch (exit 0)."
exit 0
