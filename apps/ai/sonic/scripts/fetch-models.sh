#!/bin/sh
# CED ses etiketleme modellerini indirir (sherpa-onnx ONNX exportlari).
# Kullanim: ./fetch-models.sh [ced-tiny|ced-mini|ced-small|ced-base]
set -eu

MODEL="${1:-ced-base}"
case "$MODEL" in
  ced-tiny|ced-mini|ced-small|ced-base) ;;
  *) echo "Bilinmeyen model: $MODEL (ced-tiny|ced-mini|ced-small|ced-base)" >&2; exit 1 ;;
esac

BASE="https://huggingface.co/k2-fsa/sherpa-onnx-${MODEL}-audio-tagging-2024-04-19/resolve/main"
DIR="$(dirname "$0")/../models/$MODEL"
mkdir -p "$DIR/test_wavs"

fetch() {
  if [ -f "$DIR/$1" ]; then
    echo "atlandi (var): $1"
  else
    echo "indiriliyor: $1"
    curl -fL --retry 3 -o "$DIR/$1" "$BASE/$1"
  fi
}

fetch model.onnx
fetch model.int8.onnx
fetch class_labels_indices.csv
# Faz 1 dogrulama kapisi icin referans ses dosyalari (bkz. plan §10)
for w in 1 2 3 4; do fetch "test_wavs/$w.wav"; done

echo "Tamam: $DIR"
