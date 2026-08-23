#!/bin/sh
# vision — tek komutluk kurulum (Linux / macOS / Git Bash)
#
#   ./tools/scripts/setup.sh                 # CPU
#   ./tools/scripts/setup.sh --gpu cuda      # CUDA (CUDA 12 + cuDNN 9 gerekir)
#   ./tools/scripts/setup.sh --model ced-small
#
# Kurulum internet ister (model agirliklari, crate'ler, npm paketleri).
# Kurulduktan sonra sistem tamamen cevrimdisi calisir.
set -eu

MODEL="ced-base"
FEATURES=""
SKIP_DASHBOARD=0

while [ $# -gt 0 ]; do
  case "$1" in
    --model) MODEL="$2"; shift 2 ;;
    --gpu)
      case "$2" in
        directml)
          echo "HATA: directml yalnizca Windows'ta calisir." >&2
          echo "      Linux/macOS icin: --gpu cuda  (CUDA 12 + cuDNN 9 gerekir)" >&2
          exit 1 ;;
        cuda|tensorrt) ;;
        *) echo "HATA: bilinmeyen saglayici '$2' (cuda | tensorrt)" >&2; exit 1 ;;
      esac
      FEATURES="--features $2"; shift 2 ;;
    --skip-dashboard) SKIP_DASHBOARD=1; shift ;;
    *) echo "bilinmeyen secenek: $1" >&2; exit 1 ;;
  esac
done

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
echo "vision kurulumu — $REPO"

need() {
  if command -v "$1" >/dev/null 2>&1; then
    echo "  [tamam] $1"
  else
    echo "  [HATA ] '$1' bulunamadi. $2" >&2
    exit 1
  fi
}

echo
echo "[1/4] Gereksinimler"
need cargo "Rust kurun: https://rustup.rs"
if [ "$SKIP_DASHBOARD" -eq 0 ]; then
  need node "Node.js kurun: https://nodejs.org"
  need pnpm "Kurulum: npm install -g pnpm"
fi
# Zorunlu degil: ses cozme surec icinde symphonia ile yapiliyor.
command -v ffmpeg >/dev/null 2>&1 \
  && echo "  [tamam] ffmpeg (yedek cozucu)" \
  || echo "  [not  ] ffmpeg yok — yaygin formatlar yine calisir"

echo
echo "[2/4] Model agirliklari ($MODEL)"
sh "$REPO/apps/ai/inference/scripts/fetch-models.sh" "$MODEL"

echo
echo "[3/4] Rust derlemesi"
cd "$REPO"
# shellcheck disable=SC2086
cargo build -p inference --release $FEATURES

echo
echo "  Dogrulama kapisi (mel hatti referansla uyumlu mu)"
cargo run -p inference --release --bin verify-mel

if [ "$SKIP_DASHBOARD" -eq 0 ]; then
  echo
  echo "[4/4] Dashboard bagimliliklari"
  pnpm --dir "$REPO/apps/dashboard" install
else
  echo
  echo "[4/4] Dashboard atlandi"
fi

MEDIA="$REPO/apps/dashboard/public/media"
mkdir -p "$MEDIA"

cat <<EOF

=======================================================
  Kurulum basariyla tamamlandi!
=======================================================

Sistemi baslatmak icin tek komut calistirin:

  ./tools/scripts/start.sh

Bu komut:
  - Ses analiz servisini arka planda baslatir
  - Dashboard'u ayaga kaldirir
  - Tarayicinizi otomatik acar (http://localhost:3000/videos)

Videolarinizi web arayuzundeki 'Video yukle' butonuyla
surukleyip birakarak dogrudan yukleyebilirsiniz.
(Dosya boyutu siniri yoktur, internet gerekmez.)
EOF
