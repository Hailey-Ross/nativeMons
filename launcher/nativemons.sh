#!/usr/bin/env bash
# nativeMons on Linux and the Steam Deck: runs the Windows launcher under Proton (umu-launcher) or Wine.
#
#   ./nativemons.sh                    start nativeMons
#   ./nativemons.sh --setup-prefix DIR put Microsoft's GDI+ into another prefix, such as Steam's
#                                      compatdata/<id>/pfx when nativeMons is added to Steam as a non-Steam game
#
# Wine's own GDI+ draws the launcher without smooth edges, and Proton 10's crashes in it. So on first start this
# fetches Microsoft's GDI+ security update KB3045171 (4.2 MB) from Microsoft's update server, checks it, unpacks
# gdiplus.dll from it without running anything, and puts it in the prefix. NATIVEMONS_NO_GDIPLUS=1 skips that.
#
# Settings (environment): WINEPREFIX (default ~/.local/share/nativeMons/prefix), NATIVEMONS_RUNNER=umu|wine,
# PROTONPATH (for umu; default its own UMU-Proton).
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXE="$HERE/NativeMonsLauncher.exe"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/nativeMons"
GDIP_URL="https://catalog.s.download.windowsupdate.com/d/msdownload/update/software/secu/2015/04/windowsserver2003-kb3045171-x64-enu_a3129f8ea90ce3f49d432be087bcb2c086b22728.exe"
GDIP_PKG_SHA="461bb729719e9bc3abda29d6d5b4c8d20fba74fb4cc40f886effc4ae100bdc86"
GDIP_DLL_SHA="0dd597597986de2cf05966c9c032d3ac69b59acb25787a303df1ff6004d58942"
GDIP_PATH="SP2QFE/asms/10/msft/windows/gdiplus/gdiplus.dll"

say() { printf 'nativeMons: %s\n' "$*" >&2; }
have() { command -v "$1" >/dev/null 2>&1; }
sha() { sha256sum "$1" 2>/dev/null | cut -d' ' -f1; }

# gdiplus.dll in the cache, fetched and checked once. Prints its path, or nothing if it couldn't be had.
gdiplus_dll() {
    local dll="$CACHE/gdiplus-kb3045171/gdiplus.dll" pkg="$CACHE/kb3045171.exe" tmp
    if [ -f "$dll" ] && [ "$(sha "$dll")" = "$GDIP_DLL_SHA" ]; then echo "$dll"; return; fi
    mkdir -p "$CACHE" || return
    if [ ! -f "$pkg" ] || [ "$(sha "$pkg")" != "$GDIP_PKG_SHA" ]; then
        say "Fetching Microsoft's GDI+ update (KB3045171, 4.2 MB) from Microsoft, for smooth drawing."
        rm -f "$pkg.part"
        if have curl; then curl -fsSL --retry 2 -o "$pkg.part" "$GDIP_URL"
        elif have wget; then wget -q -O "$pkg.part" "$GDIP_URL"
        else say "Neither curl nor wget is installed, so GDI+ can't be fetched."; return; fi
        if [ "$(sha "$pkg.part")" != "$GDIP_PKG_SHA" ]; then say "The GDI+ download didn't match its checksum; not used."; rm -f "$pkg.part"; return; fi
        mv -f "$pkg.part" "$pkg"
    fi
    tmp="$(mktemp -d "$CACHE/unpack.XXXXXX")" || return
    # Unpacked as an archive, never run.
    if have cabextract; then cabextract -q -d "$tmp" -F "$GDIP_PATH" "$pkg" >/dev/null 2>&1
    elif have 7z; then 7z x -y -o"$tmp" "$pkg" "$GDIP_PATH" >/dev/null 2>&1
    elif have 7zz; then 7zz x -y -o"$tmp" "$pkg" "$GDIP_PATH" >/dev/null 2>&1
    else say "Install cabextract or 7-Zip (7z) to unpack GDI+; the launcher starts without it."; rm -rf "$tmp"; return; fi
    if [ "$(sha "$tmp/$GDIP_PATH")" != "$GDIP_DLL_SHA" ]; then say "gdiplus.dll didn't match its checksum; not used."; rm -rf "$tmp"; return; fi
    mkdir -p "$(dirname "$dll")" && mv -f "$tmp/$GDIP_PATH" "$dll"
    rm -rf "$tmp"
    echo "$dll"
}

# Into a prefix's system32, checked every start: a Proton update puts its own back.
put_gdiplus() {
    local prefix="$1" dll target
    [ "${NATIVEMONS_NO_GDIPLUS:-0}" = "1" ] && return 1
    target="$prefix/drive_c/windows/system32/gdiplus.dll"
    [ -d "$(dirname "$target")" ] || { say "No Windows folder in $prefix yet."; return 1; }
    [ -f "$target" ] && [ "$(sha "$target")" = "$GDIP_DLL_SHA" ] && return 0
    dll="$(gdiplus_dll)"
    [ -n "$dll" ] || return 1
    cp -f "$dll" "$target"
}

if [ "${1:-}" = "--setup-prefix" ]; then
    [ -n "${2:-}" ] || { say "Usage: $0 --setup-prefix <prefix folder>"; exit 2; }
    if put_gdiplus "$2"; then
        say "Done. Add this to the game's Steam launch options: WINEDLLOVERRIDES=\"gdiplus=n,b\" %command%"; exit 0
    fi
    exit 1
fi

[ -f "$EXE" ] || { say "NativeMonsLauncher.exe isn't next to this script ($HERE)."; exit 1; }

# The runner: umu-launcher (Proton) first, then Wine.
runner="${NATIVEMONS_RUNNER:-}"
if [ -z "$runner" ]; then
    if have umu-run; then runner=umu; elif have wine; then runner=wine; fi
fi
case "$runner" in
    umu) have umu-run || { say "umu-run isn't installed."; exit 1; }; run() { umu-run "$@"; } ;;
    wine) have wine || { say "wine isn't installed."; exit 1; }; run() { wine "$@"; } ;;
    *) say "Install umu-launcher (Proton) or Wine to run nativeMons."; exit 1 ;;
esac

export WINEPREFIX="${WINEPREFIX:-$HOME/.local/share/nativeMons/prefix}"
export GAMEID="${GAMEID:-umu-default}" STORE="${STORE:-none}"
mkdir -p "$WINEPREFIX"
if [ ! -d "$WINEPREFIX/drive_c/windows/system32" ]; then
    say "Setting up the Windows prefix in $WINEPREFIX (first start only)."
    WINEDEBUG=-all run wineboot -u >/dev/null 2>&1
fi

if put_gdiplus "$WINEPREFIX"; then
    export WINEDLLOVERRIDES="gdiplus=n,b${WINEDLLOVERRIDES:+;$WINEDLLOVERRIDES}"
else
    [ "${NATIVEMONS_NO_GDIPLUS:-0}" = "1" ] || say "Starting with Wine's own GDI+: edges look rough (and Proton 10 may crash; set PROTONPATH=GE-Proton)."
fi

cd "$HERE" || exit 1
export WINEDEBUG="${WINEDEBUG:--all}"
if [ "$runner" = umu ]; then exec umu-run ./NativeMonsLauncher.exe "$@"; else exec wine ./NativeMonsLauncher.exe "$@"; fi
