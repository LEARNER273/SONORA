#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# SONORA Android APK build (no Gradle — raw aapt2/javac/d8/zipalign/apksigner).
#
# Output: artifacts/apk/SONORA-debug.apk (arm64-v8a)
# Requires: Android NDK (ANDROID_NDK_HOME), SDK build-tools 34 + android-34
# platform at ~/Android/Sdk, Rust with aarch64-linux-android target, python3+PIL.
# ---------------------------------------------------------------------------
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDK="${ANDROID_SDK_HOME:-$HOME/Android/Sdk}"
NDK="${ANDROID_NDK_HOME:-$HOME/Android/android-ndk-r26b}"
BT="$SDK/build-tools/34.0.0"
PLATFORM_JAR="$SDK/platforms/android-34/android.jar"

BUILD="$ROOT/build-apk"
OUT_DIR="$ROOT/artifacts/apk"
APK="$OUT_DIR/SONORA-debug.apk"

for tool in "$BT/aapt2" "$BT/d8" "$BT/zipalign" "$BT/apksigner"; do
    [ -x "$tool" ] || { echo "ERROR: missing $tool (install Android SDK build-tools 34)"; exit 1; }
done
[ -f "$PLATFORM_JAR" ] || { echo "ERROR: missing $PLATFORM_JAR (install android-34 platform)"; exit 1; }

export ANDROID_NDK_HOME="$NDK"
source "$HOME/.cargo/env" 2>/dev/null || true

echo "== [1/9] Cross-compiling libsonora.so (arm64-v8a + x86_64, release)"
cd "$ROOT"
cargo ndk --platform 26 -t arm64-v8a -t x86_64 build --release -p sonora-android
SO_ARM64="$ROOT/target/aarch64-linux-android/release/libsonora.so"
SO_X86="$ROOT/target/x86_64-linux-android/release/libsonora.so"
[ -f "$SO_ARM64" ] || { echo "ERROR: arm64 libsonora.so not produced"; exit 1; }
[ -f "$SO_X86" ] || { echo "ERROR: x86_64 libsonora.so not produced"; exit 1; }

echo "== [2/9] Generating launcher icons"
python3 "$ROOT/scripts/gen_icons.py"

echo "== [3/9] Staging assets + compiling/linking resources"
rm -rf "$BUILD"
mkdir -p "$BUILD/gen" "$BUILD/classes" "$BUILD/dex" "$BUILD/assets"
cp "$ROOT/apps/sonora-android/assets/index.html" "$BUILD/assets/"
cp "$ROOT/apps/sonora-android/styles/sonora.css" "$BUILD/assets/"

"$BT/aapt2" compile --dir "$ROOT/platform/res" -o "$BUILD/res.zip"
"$BT/aapt2" link -o "$BUILD/base.apk" \
    --manifest "$ROOT/platform/AndroidManifest.xml" \
    -I "$PLATFORM_JAR" \
    --java "$BUILD/gen" \
    --min-sdk-version 26 --target-sdk-version 34 \
    -A "$BUILD/assets" \
    "$BUILD/res.zip"

echo "== [4/9] Compiling Java"
javac --release 11 -classpath "$PLATFORM_JAR" -d "$BUILD/classes" \
    "$BUILD/gen/com/sonora/app/R.java" \
    "$ROOT/platform/java/com/sonora/app/SonoraNative.java" \
    "$ROOT/platform/java/com/sonora/app/MainActivity.java"

echo "== [5/9] Dexing (d8)"
"$BT/d8" --release --lib "$PLATFORM_JAR" --min-api 26 --output "$BUILD/dex" \
    $(find "$BUILD/classes" -name '*.class')

echo "== [6/9] Packing classes.dex + native libraries (universal: arm64 + x86_64)"
python3 - "$BUILD/base.apk" "$BUILD/dex/classes.dex" "$SO_ARM64" "$SO_X86" <<'PY'
import sys, zipfile
apk_path, dex_path, so_arm64, so_x86 = sys.argv[1:5]
with zipfile.ZipFile(apk_path, "a", zipfile.ZIP_DEFLATED) as z:
    z.write(dex_path, "classes.dex")
    z.write(so_arm64, "lib/arm64-v8a/libsonora.so")
    z.write(so_x86, "lib/x86_64-linux-android/libsonora.so")
print("  packed classes.dex + lib/arm64-v8a/libsonora.so + lib/x86_64-linux-android/libsonora.so")
PY

echo "== [7/9] Zipalign"
mkdir -p "$OUT_DIR"
"$BT/zipalign" -f 4 "$BUILD/base.apk" "$BUILD/aligned.apk"

echo "== [8/9] Signing with debug keystore"
KS="$ROOT/artifacts/keystores/debug.keystore"
if [ ! -f "$KS" ]; then
    mkdir -p "$(dirname "$KS")"
    keytool -genkeypair -keystore "$KS" -alias androiddebugkey \
        -storepass android -keypass android -keyalg RSA -keysize 2048 \
        -validity 10000 -dname "CN=Android Debug,O=SONORA,C=US" >/dev/null 2>&1
    echo "  generated fresh debug keystore"
fi
"$BT/apksigner" sign --ks "$KS" --ks-pass pass:android --key-pass pass:android \
    --out "$APK" "$BUILD/aligned.apk"

echo "== [9/9] Verifying signature"
"$BT/apksigner" verify --print-certs "$APK" | head -4

echo ""
echo "✔ SONORA-debug.apk ready: $APK"
ls -la "$APK"
