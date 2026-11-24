#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# SONORA emulator test rig (no physical device needed).
#
#   ./scripts/run_emulator.sh
#
# Creates the AVD on first run (Android 11 x86_64, KVM-accelerated), boots it
# in a window on the desktop, installs artifacts/apk/SONORA-debug.apk, launches
# the app, and verifies the process is alive. Ctrl+C shuts the emulator down.
#
# One-time prerequisites (already done if scripts were run):
#   sdkmanager --sdk_root=$HOME/Android/Sdk "emulator" \
#       "system-images;android-31;google_apis;x86_64"
# ---------------------------------------------------------------------------
set -euo pipefail

SDK="${ANDROID_SDK_HOME:-$HOME/Android/Sdk}"
EMU="$SDK/emulator/emulator"
ADB="$SDK/platform-tools/adb"
AVDMAN="$SDK/cmdline-tools/latest/bin/avdmanager"
AVD_NAME="sonora"
IMG="system-images;android-31;google_apis;x86_64"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

[ -x "$EMU" ] || { echo "ERROR: emulator not installed at $EMU"; exit 1; }
[ -f "$ROOT/artifacts/apk/SONORA-debug.apk" ] || {
    echo "ERROR: APK missing — run scripts/build_apk.sh first"; exit 1; }

if ! "$EMU" -list-avds 2>/dev/null | grep -qx "$AVD_NAME"; then
    echo "== creating AVD '$AVD_NAME' ($IMG)"
    [ -x "$AVDMAN" ] || { echo "ERROR: avdmanager not found at $AVDMAN"; exit 1; }
    echo no | "$AVDMAN" create avd -n "$AVD_NAME" -k "$IMG" -d pixel_5 --force
fi

echo "== booting emulator (window opens on the desktop)"
"$EMU" -avd "$AVD_NAME" -no-snapshot -gpu swiftshader_indirect &
EMU_PID=$!
cleanup() { kill "$EMU_PID" 2>/dev/null || true; }
trap cleanup EXIT

echo "== waiting for Android boot (first boot can take a couple of minutes)"
"$ADB" wait-for-device
until [ "$("$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do
    sleep 2
done
echo "== boot complete"

echo "== installing APK"
"$ADB" install -r "$ROOT/artifacts/apk/SONORA-debug.apk"

echo "== launching SONORA"
"$ADB" shell am start -n com.sonora.app/.MainActivity
sleep 3
PID=$("$ADB" shell pidof com.sonora.app 2>/dev/null | tr -d '\r')
if [ -n "$PID" ]; then
    echo "✔ SONORA is running on the emulator (pid $PID)."
    echo "  → Tap Play in the emulator window; audio comes from your PC speakers."
    echo "  → Live engine logs:  $ADB logcat -s SonoraAudio:V AAudio:V"
    echo ""
    echo "The emulator keeps running until you press Ctrl+C here."
    wait "$EMU_PID"
else
    echo "✘ app process not found — debug with:"
    echo "    $ADB logcat -d | grep -iE 'sonora|AndroidRuntime'"
    exit 1
fi
