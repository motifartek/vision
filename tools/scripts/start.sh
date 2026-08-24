#!/bin/sh
# vision - tek komutla inference + dashboard baslat (Linux / macOS / Git Bash)
#
#   ./tools/scripts/start.sh             # CPU
#   ./tools/scripts/start.sh --model ced-tiny
#
# Ctrl+C ile her iki servisi de kapatir.
set -eu

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
EXE="$REPO/target/release/inference"
MEDIA="$REPO/apps/dashboard/public/media"

# --- inference binary kontrol ---
if [ ! -f "$EXE" ]; then
    echo "[HATA] inference bulunamadi. Once kurulumu calistirin:"
    echo "       ./tools/scripts/setup.sh"
    exit 1
fi

# --- zaten calisiyor mu? ---
if pgrep -x inference >/dev/null 2>&1; then
    echo "[!] inference zaten calisiyor."
    printf "    Yeniden baslatmak ister misiniz? (E/h): "
    read -r answer
    case "$answer" in
        [Hh]*) echo "    Mevcut inference kullaniliyor." ;;
        *)
            pkill -x inference || true
            sleep 0.5
            echo "    Durduruldu."
            ;;
    esac
fi

# --- ortam degiskenleri ---
mkdir -p "$MEDIA"
export INFERENCE_MEDIA_ROOT="$MEDIA"

# Model parametresi
while [ $# -gt 0 ]; do
    case "$1" in
        --model) export INFERENCE_MODEL="$2"; shift 2 ;;
        *) echo "bilinmeyen secenek: $1" >&2; exit 1 ;;
    esac
done

# --- temiz cikis ---
INFERENCE_PID=""
cleanup() {
    echo ""
    echo "Inference kapatiliyor..."
    [ -n "$INFERENCE_PID" ] && kill "$INFERENCE_PID" 2>/dev/null || true
    wait "$INFERENCE_PID" 2>/dev/null || true
    echo "Tamam. Her iki servis de kapatildi."
}
trap cleanup EXIT INT TERM

# --- inference arka planda ---
echo ""
echo "Inference servisi baslatiliyor..."
"$EXE" &
INFERENCE_PID=$!

# healthz ping (en fazla 30 sn)
READY=0
for i in $(seq 1 60); do
    sleep 0.5
    if curl -sf http://127.0.0.1:8081/healthz >/dev/null 2>&1; then
        READY=1
        break
    fi
done

if [ "$READY" -eq 0 ]; then
    echo "[HATA] Inference servisi 30 saniyede hazir olmadi."
    exit 1
fi

echo "Inference hazir (PID $INFERENCE_PID, port 8081)"

# --- tarayici ac ---
echo "Dashboard baslatiliyor..."
sleep 1
(
    if command -v xdg-open >/dev/null 2>&1; then
        xdg-open "http://localhost:3000/videos" 2>/dev/null
    elif command -v open >/dev/null 2>&1; then
        open "http://localhost:3000/videos"
    fi
) &

echo ""
echo "  Tarayici acildi -> http://localhost:3000/videos"
echo "  Kapatmak icin Ctrl+C basin."
echo ""

# --- dashboard on planda ---
pnpm --dir "$REPO/apps/dashboard" dev
