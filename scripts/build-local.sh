#!/usr/bin/env bash
# Builds Parla for this Mac, signed with your Apple Development certificate so
# macOS keeps Microphone/Accessibility permissions across rebuilds, then
# installs it to /Applications.
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/fetch-model.sh
IDENTITY="${APPLE_SIGNING_IDENTITY:-$(security find-identity -v -p codesigning | grep -m1 -o '"Apple Development[^"]*"' | tr -d '"' || true)}"
if [ -n "$IDENTITY" ]; then
  echo "Signing with: $IDENTITY"
  export APPLE_SIGNING_IDENTITY="$IDENTITY"
else
  echo "No signing certificate found; building unsigned (permissions reset on every rebuild)."
fi
npm run tauri build -- --bundles app
osascript -e 'quit app "Parla"' 2>/dev/null || true
sleep 1
rm -rf /Applications/Parla.app
cp -R src-tauri/target/release/bundle/macos/Parla.app /Applications/
echo "Installed /Applications/Parla.app"
