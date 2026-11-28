#!/bin/bash
# run-probes.sh <script.txt> [prefix]: one probe script through GDI+ (GdiProbe.exe) and nm-draw (probe example), then
# the per-canvas diff. Build GdiProbe.exe first (see the step 1 note); NM_FONTS names the Noto folder for text ops.
OUT="$(cygpath -u "${PROBE_OUT:-${NATIVEMONS_WORK:?set NATIVEMONS_WORK or PROBE_OUT}/gdiplus-probe}")"
T="$(cygpath -u "${PROBE_RS:-${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR or PROBE_RS}/debug/examples/probe.exe}")"
HERE="$(cd "$(dirname "$0")" && pwd)"
n=$(basename "$1" .txt); rm -rf "$OUT/g/$n" "$OUT/o/$n"; mkdir -p "$OUT/g/$n" "$OUT/o/$n"
"$OUT/GdiProbe.exe" "$(cygpath -w "$HERE/scripts/$1")" "$(cygpath -w "$OUT/g/$n")" || exit 1
"$T" run "$(cygpath -w "$HERE/scripts/$1")" "$(cygpath -w "$OUT/o/$n")"
"$T" diff "$(cygpath -w "$OUT/g/$n")" "$(cygpath -w "$OUT/o/$n")" $2
