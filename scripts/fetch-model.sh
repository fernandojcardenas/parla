#!/usr/bin/env bash
# Downloads the speech model that gets bundled inside the Parla app.
# The model is too large for git, so it is fetched at build time instead.
set -euo pipefail
MODEL="ggml-large-v3-turbo-q5_0.bin"
URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$MODEL"
DIR="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/resources/models"
mkdir -p "$DIR"
if [ -s "$DIR/$MODEL" ]; then
  echo "Model already present: $DIR/$MODEL"
  exit 0
fi
echo "Downloading $MODEL ..."
curl -L --fail --progress-bar -o "$DIR/$MODEL.part" "$URL"
mv "$DIR/$MODEL.part" "$DIR/$MODEL"
echo "Saved to $DIR/$MODEL"
