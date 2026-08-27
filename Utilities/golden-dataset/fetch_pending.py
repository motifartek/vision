#!/usr/bin/env python3
"""Katalogdaki indirilmemiş videoları çeker.

Aday listesinden onaylanan videolar kataloğa `local_path: null` ile giriyor;
bu script onları indirip metadata'yı dolduruyor. Videolar depoya konmuyor
(`.gitignore`), katalog yalnızca kaynağı gösteriyor — şartname veri setinin
herkese açık indirilebilir olmasını istiyor, kaynak bağlantısı bunu karşılıyor.

    python fetch_pending.py            # bekleyen tüm videoları indir
    python fetch_pending.py --id ABC   # yalnızca birini
    python fetch_pending.py --plan     # ne inecek, indirmeden göster

Gereksinim: yt-dlp (`pip install yt-dlp`) ve ffmpeg.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).parent
VIDEO_DIR = HERE / "videos"
CATALOG = HERE / "catalog.json"

# CCTV görüntüsüyle çalışıyoruz; 4K indirmenin anlamı yok ve boyutu şişiriyor.
# 720p tavanı gerçekçi ve yeterli.
FORMAT = "bestvideo[height<=720]+bestaudio/best[height<=720]/best"


def probe(path: Path) -> dict | None:
    result = subprocess.run(
        [
            "ffprobe", "-v", "error", "-select_streams", "v:0",
            "-show_entries", "stream=width,height,r_frame_rate,codec_name",
            "-show_entries", "format=duration,size", "-of", "json", str(path),
        ],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        return None

    data = json.loads(result.stdout)
    stream, fmt = data["streams"][0], data["format"]
    num, _, den = stream["r_frame_rate"].partition("/")

    return {
        "duration_ms": round(float(fmt["duration"]) * 1000),
        "width": stream["width"],
        "height": stream["height"],
        "fps": round(float(num) / float(den or 1), 3),
        "codec": stream["codec_name"],
        "size_bytes": int(fmt["size"]),
    }


def indir(video: dict) -> tuple[bool, str]:
    hedef = VIDEO_DIR / f"{video['id']}.mp4"
    if hedef.exists() and hedef.stat().st_size > 0:
        return True, "zaten var"

    result = subprocess.run(
        [
            sys.executable, "-m", "yt_dlp",
            "-f", FORMAT,
            "--merge-output-format", "mp4",
            "--no-playlist",
            "--no-warnings",
            "-o", str(hedef),
            video["source_url"],
        ],
        capture_output=True, text=True,
    )

    if result.returncode != 0 or not hedef.exists():
        # yt-dlp hata metni uzun; son anlamlı satır yeterli.
        hata = [l for l in (result.stderr or "").splitlines() if l.strip()]
        return False, (hata[-1][:120] if hata else "bilinmeyen hata")

    return True, "indi"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--id", help="yalnızca bu kimliği indir")
    parser.add_argument("--plan", action="store_true", help="indirmeden göster")
    args = parser.parse_args()

    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
    bekleyen = [
        v for v in catalog["videos"]
        if not v.get("local_path") and v.get("source_url")
        and (not args.id or v["id"] == args.id)
    ]

    if not bekleyen:
        print("bekleyen video yok.")
        return 0

    print(f"{len(bekleyen)} video bekliyor")
    if args.plan:
        for v in bekleyen:
            print(f"  {v['id']:<22} {v['source_url']}")
        return 0

    VIDEO_DIR.mkdir(parents=True, exist_ok=True)
    ok, basarisiz, olcut_disi = 0, [], []

    for i, v in enumerate(bekleyen, 1):
        print(f"\n[{i}/{len(bekleyen)}] {v['id']}")
        basarili, mesaj = indir(v)

        if not basarili:
            print(f"  ! {mesaj}")
            basarisiz.append((v["id"], mesaj))
            continue

        meta = probe(VIDEO_DIR / f"{v['id']}.mp4")
        if not meta:
            print("  ! indi ama okunamadı")
            basarisiz.append((v["id"], "ffprobe okuyamadı"))
            continue

        v["local_path"] = f"videos/{v['id']}.mp4"
        v.update(meta)
        ok += 1

        sure = meta["duration_ms"] / 1000
        print(f"  {sure:.0f} sn · {meta['width']}x{meta['height']} · {meta['size_bytes']/1e6:.1f} MB")

        # Ölçüt dışı süreler engellenmiyor ama bildiriliyor: uzun bir video
        # derleme olabilir ve kırpılması gerekebilir.
        if sure > 180:
            olcut_disi.append((v["id"], f"{sure:.0f} sn — 3 dk üstü, derleme olabilir"))
        elif sure < 30:
            olcut_disi.append((v["id"], f"{sure:.0f} sn — 30 sn altı, yay oluşmayabilir"))

    CATALOG.write_text(
        json.dumps(catalog, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )

    print(f"\nindi: {ok} · başarısız: {len(basarisiz)}")
    for name, hata in basarisiz:
        print(f"  ! {name}: {hata}")

    if olcut_disi:
        print("\nölçüt dışı süreler (gözden geçir):")
        for name, uyari in olcut_disi:
            print(f"  ~ {name}: {uyari}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
