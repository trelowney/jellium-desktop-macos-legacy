#!/bin/bash
# Smoke test of a built app on a CI Mac: start it against a stand-in Jellyfin
# server and play a short clip over HTTPS, the way a user's first playback
# goes. Catches a player that can't open HTTPS streams, a startup that never
# finishes, and a bundle that still needs the build machine's libraries
# (/opt/local is moved away while the app runs).
#
# Usage: smoke_test.sh <path to Jellium Desktop.app> <https media URL>
set -euo pipefail

app=$1
media_url=$2
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
mkdir "$work/config" "$work/cache"
printf '{"serverUrl":"http://127.0.0.1:8096"}' > "$work/config/settings.json"
log=$work/app.log
exe="$app/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$app/Contents/Info.plist")"

hidden=
if [ -d /opt/local ]; then
  sudo mv /opt/local /opt/local.smoke
  hidden=1
fi
cleanup() {
  kill "${app_pid:-}" "${server_pid:-}" 2>/dev/null || true
  if [ -n "$hidden" ]; then sudo mv /opt/local.smoke /opt/local; fi
  rm -rf "$work"
}
trap cleanup EXIT

python3 "$here/smoke_server.py" "$media_url" 8096 &
server_pid=$!

"$exe" --config-dir "$work/config" --cache-dir "$work/cache" \
  --log-file "$log" --log-level info >"$work/output.log" 2>&1 &
app_pid=$!

result="no playback within 120 s"
for _ in $(seq 120); do
  sleep 1
  if grep -q 'VO: \[' "$log" 2>/dev/null; then result=; break; fi
  if grep -qE 'Failed to open|tls: ' "$log" 2>/dev/null; then result="the player could not open the stream"; break; fi
  if ! kill -0 "$app_pid" 2>/dev/null; then result="the app exited"; break; fi
done
if [ -z "$result" ] && ! grep -q 'Device profile:' "$log"; then
  result="startup did not publish the device profile"
fi

if [ -n "$result" ]; then
  echo "::error::Smoke test failed: $result"
  grep -v 'Device profile:' "$log" | tail -80 || true
  echo "--- app output"
  tail -40 "$work/output.log" || true
  report=$(ls -t ~/Library/Logs/DiagnosticReports/*.ips 2>/dev/null | head -1)
  if [ -n "$report" ]; then
    echo "--- crash report $report"
    python3 "$here/crash_summary.py" "$report" || true
  fi
  exit 1
fi
echo "Smoke test passed: the app started and played $media_url"
