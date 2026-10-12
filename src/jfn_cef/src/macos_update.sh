#!/bin/bash
# macOS self-update, run detached by the app (see updater.rs) right before it
# quits. Downloads the release DMG, waits for the app to exit, copies the new
# bundle over the old one and starts it again. On any failure the old app is
# kept (or put back), started again, and the user is told what went wrong.
#
# Usage: macos_update.sh <dmg-url> <size-bytes> <app-pid> <app-bundle-path> <version>
# JFN_UPDATE_TEST=1 skips notifications, dialogs and the relaunch (used by CI).

set -u
url=$1
size=$2
pid=$3
app=$4
version=$5
testing=${JFN_UPDATE_TEST:-}

work=$(mktemp -d "${TMPDIR:-/tmp}/jellium-update.XXXXXX") || exit 1
mnt="$work/mnt"
logdir="$HOME/Library/Logs/Jellium Desktop"
mkdir -p "$logdir" && exec >"$logdir/update.log" 2>&1
echo "$(date) updating $app to $version from $url"

# Messages go to AppleScript as arguments, so no quoting can break them.
notify() {
    [ -n "$testing" ] && return
    osascript -e 'on run argv' -e 'display notification (item 1 of argv) with title "Jellium Desktop"' \
        -e 'end run' "$1" >/dev/null 2>&1 || true
}

cleanup() {
    [ -d "$mnt" ] && hdiutil detach -quiet -force "$mnt" >/dev/null 2>&1
    rm -rf "$app.new" "$work"
}

fail() {
    echo "error: $1"
    if [ ! -d "$app" ] && [ -d "$work/old.app" ]; then
        mv "$work/old.app" "$app"
    fi
    cleanup
    if [ -z "$testing" ]; then
        [ -d "$app" ] && open "$app"
        osascript -e 'on run argv' \
            -e 'display alert "Jellium Desktop could not update" message (item 1 of argv)' \
            -e 'end run' "$1 Download the new version from the Releases page instead." \
            >/dev/null 2>&1 || true
    fi
    exit 1
}

notify "Downloading version $version…"
curl -fL --retry 3 --connect-timeout 20 -o "$work/update.dmg" "$url" \
    || fail "The download failed."
got=$(stat -f%z "$work/update.dmg")
if [ "$size" -gt 0 ] && [ "$got" != "$size" ]; then
    fail "The download is incomplete ($got of $size bytes)."
fi

# The app quits on its own right after starting us; give it 30 s, then make sure.
for _ in $(seq 60); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.5
done
if kill -0 "$pid" 2>/dev/null; then
    echo "app $pid still running, killing it"
    kill -9 "$pid" 2>/dev/null
    sleep 1
fi

notify "Installing version $version…"
mkdir -p "$mnt"
hdiutil attach -nobrowse -readonly -noautoopen -mountpoint "$mnt" "$work/update.dmg" >/dev/null \
    || fail "The downloaded disk image could not be opened."
src=$(find "$mnt" -maxdepth 1 -name '*.app' -type d | head -n 1)
[ -n "$src" ] || fail "The disk image contains no app."
rm -rf "$app.new"
ditto "$src" "$app.new" || fail "The new version could not be copied next to the old one."
hdiutil detach -quiet -force "$mnt" >/dev/null 2>&1
[ -x "$app.new/Contents/MacOS/jellium-desktop" ] || fail "The new version is incomplete."

# Swap: old bundle aside, new one in, old one deleted only once that worked.
mv "$app" "$work/old.app" || fail "The old version could not be moved aside."
mv "$app.new" "$app" || fail "The new version could not be moved into place."
echo "$(date) installed $version"
rm -rf "$work"

[ -z "$testing" ] && open "$app"
exit 0
