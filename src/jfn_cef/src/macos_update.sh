#!/bin/bash
# macOS self-update, driven by the app (see updater.rs) in two steps:
#
#   prepare <dmg-url> <size-bytes> <app-bundle-path> <work-dir>
#       While the app keeps running and shows progress: download the release
#       DMG into <work-dir>/update.dmg and copy its app next to the installed
#       one as "<app>.new". On failure nothing is changed and the app says so.
#   finish <app-pid> <app-bundle-path> <work-dir>
#       Run detached once prepare succeeded and the app is quitting: wait for
#       it to exit, swap "<app>.new" in and start it again. On failure the old
#       app is kept (or put back), started again, and the user is told why.
#
# JFN_UPDATE_TEST=1 skips dialogs and the relaunch, and allows file:// URLs
# (used by CI).

set -u
mode=${1:-}
testing=${JFN_UPDATE_TEST:-}
logdir="$HOME/Library/Logs/Jellium Desktop"
mkdir -p "$logdir"

case "$mode" in
prepare)
    url=$2 size=$3 app=$4 work=$5
    exec >"$logdir/update.log" 2>&1
    ;;
finish)
    pid=$2 app=$3 work=$4
    exec >>"$logdir/update.log" 2>&1
    ;;
*)
    echo "usage: $0 prepare|finish ..." >&2
    exit 2
    ;;
esac
mnt="$work/mnt"
echo "$(date) $mode $app"

fail() {
    echo "error: $1"
    [ -d "$mnt" ] && hdiutil detach -quiet -force "$mnt" >/dev/null 2>&1
    if [ "$mode" = finish ]; then
        if [ ! -d "$app" ] && [ -d "$work/old.app" ]; then
            mv "$work/old.app" "$app"
        fi
        rm -rf "$app.new" "$work"
        if [ -z "$testing" ]; then
            [ -d "$app" ] && open "$app"
            # The message goes to AppleScript as an argument, so no quoting
            # can break it.
            osascript -e 'on run argv' \
                -e 'display alert "Jellium Desktop could not update" message (item 1 of argv)' \
                -e 'end run' "$1 Download the new version from the Releases page instead." \
                >/dev/null 2>&1 || true
        fi
    else
        rm -rf "$app.new" "$work"
    fi
    exit 1
}

if [ "$mode" = prepare ]; then
    mkdir -p "$work" || fail "No room for the download."
    # HTTPS only, redirects included (CI's test reads a local file:// DMG).
    proto="=https"
    [ -n "$testing" ] && proto="=https,file"
    curl -fsSL --proto "$proto" --proto-redir =https --retry 3 --connect-timeout 20 \
        -o "$work/update.dmg" -- "$url" \
        || fail "The download failed."
    got=$(stat -f%z "$work/update.dmg")
    if [ "$size" -gt 0 ] && [ "$got" != "$size" ]; then
        fail "The download is incomplete ($got of $size bytes)."
    fi
    mkdir -p "$mnt"
    hdiutil attach -nobrowse -readonly -noautoopen -mountpoint "$mnt" "$work/update.dmg" >/dev/null \
        || fail "The downloaded disk image could not be opened."
    src=$(find "$mnt" -maxdepth 1 -name '*.app' -type d | head -n 1)
    [ -n "$src" ] || fail "The disk image contains no app."
    rm -rf "$app.new"
    ditto "$src" "$app.new" || fail "The new version could not be copied next to the old one."
    hdiutil detach -quiet -force "$mnt" >/dev/null 2>&1
    rm -f "$work/update.dmg"
    [ -x "$app.new/Contents/MacOS/jellium-desktop" ] || fail "The new version is incomplete."
    echo "$(date) prepared"
    exit 0
fi

# finish: the app quits on its own right after starting us; give it 30 s.
for _ in $(seq 60); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.5
done
if kill -0 "$pid" 2>/dev/null; then
    echo "app $pid still running, killing it"
    kill -9 "$pid" 2>/dev/null
    sleep 1
fi
[ -x "$app.new/Contents/MacOS/jellium-desktop" ] || fail "The prepared new version is missing."

# Swap: old bundle aside, new one in, old one deleted only once that worked.
mv "$app" "$work/old.app" || fail "The old version could not be moved aside."
mv "$app.new" "$app" || fail "The new version could not be moved into place."
echo "$(date) installed"
rm -rf "$work"

[ -z "$testing" ] && open "$app"
exit 0
