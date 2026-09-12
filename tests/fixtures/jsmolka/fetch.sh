#!/usr/bin/env bash
# Fetch MIT jsmolka prebuilts into path stubs (reproducible re-vendor).
#
# Cited: jsmolka/gba-tests (MIT) prebuilt .gba layout
#   https://github.com/jsmolka/gba-tests
# Note: arm/thumb/memory + ppu/{hello,shades,stripes}. Never fetches BIOS or commercial carts.
# Pin is the commit SHA below; override with JSMOLKA_REF=<sha> if needed.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
# Pinned upstream commit (2026-09-12 tip of master at vendor time).
PIN="${JSMOLKA_REF:-a7113b67e63f83a9b321696ddd7042ccfad6c881}"
BASE_URL="https://raw.githubusercontent.com/jsmolka/gba-tests/${PIN}"

fetch_one() {
  local rel="$1"
  local dest="${ROOT}/${rel}"
  mkdir -p "$(dirname "${dest}")"
  echo "fetching ${rel} @ ${PIN}"
  curl -fsSL "${BASE_URL}/${rel}" -o "${dest}"
}

fetch_one "arm/arm.gba"
fetch_one "thumb/thumb.gba"
fetch_one "memory/memory.gba"
fetch_one "ppu/hello.gba"
fetch_one "ppu/shades.gba"
fetch_one "ppu/stripes.gba"
fetch_one "bios/bios.gba"
fetch_one "save/none.gba"
fetch_one "save/sram.gba"
fetch_one "save/flash64.gba"
fetch_one "save/flash128.gba"

# Layout under fixtures uses per-ROM dirs for harness paths.
mkdir -p "${ROOT}/ppu/hello" "${ROOT}/ppu/shades" "${ROOT}/ppu/stripes"
mkdir -p "${ROOT}/save/none" "${ROOT}/save/sram" "${ROOT}/save/flash64" "${ROOT}/save/flash128"
mv -f "${ROOT}/ppu/hello.gba" "${ROOT}/ppu/hello/hello.gba"
mv -f "${ROOT}/ppu/shades.gba" "${ROOT}/ppu/shades/shades.gba"
mv -f "${ROOT}/ppu/stripes.gba" "${ROOT}/ppu/stripes/stripes.gba"
mv -f "${ROOT}/save/none.gba" "${ROOT}/save/none/none.gba"
mv -f "${ROOT}/save/sram.gba" "${ROOT}/save/sram/sram.gba"
mv -f "${ROOT}/save/flash64.gba" "${ROOT}/save/flash64/flash64.gba"
mv -f "${ROOT}/save/flash128.gba" "${ROOT}/save/flash128/flash128.gba"

echo "done. LICENSE remains ${ROOT}/LICENSE (upstream MIT)."
echo "sha256:"
sha256sum \
  "${ROOT}/arm/arm.gba" \
  "${ROOT}/thumb/thumb.gba" \
  "${ROOT}/memory/memory.gba" \
  "${ROOT}/ppu/hello/hello.gba" \
  "${ROOT}/ppu/shades/shades.gba" \
  "${ROOT}/ppu/stripes/stripes.gba" \
  "${ROOT}/bios/bios.gba" \
  "${ROOT}/save/none/none.gba" \
  "${ROOT}/save/sram/sram.gba" \
  "${ROOT}/save/flash64/flash64.gba" \
  "${ROOT}/save/flash128/flash128.gba"
