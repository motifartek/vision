#!/usr/bin/env python3
"""Golden dataset'e tek video ekler.

Küme toplu indirmeyle değil, tek tek küratörlükle büyüyor: her video insan
onayından geçiyor. Kabul ölçütleri README.md'de — özeti şu: kırpılmamış,
30 sn–3 dk, en az iki zaman damgalı olay, gerçek İSG bağlamı, tercihen sabit
kamera.

Bu script yalnızca kaydı açar; olay zaman çizelgesi `annotate.html` ile
işaretlenir.

    python add_video.py <video-dosyasi> --kaynak <url> [--not "..."]
    python add_video.py --liste          # kataloğu göster
    python add_video.py --sil <id>
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import unicodedata
from pathlib import Path

HERE = Path(__file__).parent
VIDEO_DIR = HERE / "videos"
CATALOG = HERE / "catalog.json"

# README'deki kabul ölçütleriyle aynı sayılar; script uyarı veriyor ama
# engellemiyor — son karar insanda.
MIN_SURE_SN = 30
MAX_SURE_SN = 180


def slugify(text: str) -> str:
    text = unicodedata.normalize("NFKD", text)
    text = "".join(c for c in text if not unicodedata.combining(c))
    text = re.sub(r"[^a-zA-Z0-9]+", "-", text).strip("-").lower()
    return text or "video"


def probe(path: Path) -> dict:
    """ffprobe ile temel metadata."""
    result = subprocess.run(
        [
            "ffprobe", "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height,r_frame_rate,codec_name",
            "-show_entries", "format=duration,size",
            "-of", "json",
            str(path),
        ],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise SystemExit(f"ffprobe başarısız: {result.stderr.strip()}")

    data = json.loads(result.stdout)
    stream = data["streams"][0]
    fmt = data["format"]

    # r_frame_rate "30000/1001" gibi bir kesir olarak gelir.
    num, _, den = stream["r_frame_rate"].partition("/")
    fps = float(num) / float(den or 1)

    return {
        "duration_ms": round(float(fmt["duration"]) * 1000),
        "width": stream["width"],
        "height": stream["height"],
        "fps": round(fps, 3),
        "codec": stream["codec_name"],
        "size_bytes": int(fmt["size"]),
    }


def load_catalog() -> dict:
    if CATALOG.exists():
        return json.loads(CATALOG.read_text(encoding="utf-8"))
    return {
        "aciklama": "Şartname 3. Senaryo ile hizalı, elle kürate edilmiş İSG olay videoları",
        "olcutler": "bkz. README.md",
        "videos": [],
    }


def save_catalog(catalog: dict) -> None:
    CATALOG.write_text(
        json.dumps(catalog, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


def format_ms(ms: int | None) -> str:
    """Süreyi biçimler. Henüz indirilmemiş kayıtlarda süre bilinmiyor."""
    if not ms:
        return "  —  "
    total = ms // 1000
    return f"{total // 60:02d}:{total % 60:02d}"


def liste(catalog: dict) -> int:
    videos = catalog["videos"]
    if not videos:
        print("katalog boş. `python add_video.py <dosya>` ile ekleyin.")
        return 0

    inen = sum(1 for v in videos if v.get("local_path"))
    print(f"{len(videos)} video · {inen} indirilmiş · {len(videos) - inen} bekliyor" + chr(10))
    isaretli = 0
    for v in videos:
        olaylar = v.get("ground_truth", {}).get("events", [])
        if olaylar:
            isaretli += 1
        if not v.get("local_path"):
            durum = "video bekleniyor"
        elif olaylar:
            durum = f"{len(olaylar)} olay"
        else:
            durum = "işaretlenmemiş"
        print(f"  {v['id']:<26} {format_ms(v.get('duration_ms')):>7}  {durum}")
        if v.get("note"):
            print(f"      {v['note']}")

    print(f"\nişaretlenmiş: {isaretli}/{len(videos)}")
    return 0


def sil(catalog: dict, video_id: str) -> int:
    before = len(catalog["videos"])
    kalan = [v for v in catalog["videos"] if v["id"] != video_id]
    if len(kalan) == before:
        print(f"bulunamadı: {video_id}", file=sys.stderr)
        return 1

    catalog["videos"] = kalan
    save_catalog(catalog)

    hedef = VIDEO_DIR / f"{video_id}.mp4"
    if hedef.exists():
        hedef.unlink()

    print(f"silindi: {video_id}")
    return 0


def ekle(catalog: dict, args) -> int:
    kaynak_dosya = Path(args.video)
    if not kaynak_dosya.exists():
        print(f"dosya yok: {kaynak_dosya}", file=sys.stderr)
        return 1

    meta = probe(kaynak_dosya)
    sure_sn = meta["duration_ms"] / 1000

    print(f"  süre        : {sure_sn:.1f} sn")
    print(f"  çözünürlük  : {meta['width']}x{meta['height']} @ {meta['fps']:g} fps")
    print(f"  codec       : {meta['codec']}")
    print(f"  boyut       : {meta['size_bytes'] / 1e6:.1f} MB")

    # Ölçüt ihlalleri engellemiyor, uyarıyor: bazen kural dışı bir video
    # bilinçli olarak istenir (ör. çok kısa ama çok net bir kaza).
    uyarilar = []
    if sure_sn < MIN_SURE_SN:
        uyarilar.append(
            f"süre {sure_sn:.0f} sn — {MIN_SURE_SN} sn altında olay yayı zor oluşur"
        )
    if sure_sn > MAX_SURE_SN:
        uyarilar.append(f"süre {sure_sn:.0f} sn — {MAX_SURE_SN} sn üstü, etiketleme pahalı")

    if uyarilar:
        print()
        for u in uyarilar:
            print(f"  ! {u}")
        if not args.zorla:
            print("\n  yine de eklemek için --zorla")
            return 1

    video_id = args.id or slugify(kaynak_dosya.stem)
    if any(v["id"] == video_id for v in catalog["videos"]):
        print(f"\nbu kimlik zaten var: {video_id} (--id ile başka bir ad verin)", file=sys.stderr)
        return 1

    VIDEO_DIR.mkdir(parents=True, exist_ok=True)
    hedef = VIDEO_DIR / f"{video_id}.mp4"
    shutil.copy2(kaynak_dosya, hedef)

    catalog["videos"].append(
        {
            "id": video_id,
            "local_path": f"videos/{hedef.name}",
            "original_name": kaynak_dosya.name,
            "source_url": args.kaynak,
            "license": args.lisans,
            "note": args.not_,
            **meta,
            # Şartnamenin çıktı biçiminin aynısı: ground truth, sistemin
            # üretmesi gereken ideal çıktıdır. annotate.html dolduruyor.
            "ground_truth": {
                "summary": None,
                "events": [],
                "risk": None,
                "actions": [],
            },
        }
    )
    save_catalog(catalog)

    print(f"\neklendi: {video_id}")
    print(f"toplam: {len(catalog['videos'])} video")
    print("\nsıradaki adım — zaman çizelgesini işaretle:")
    print("  python -m http.server 8200   ->  localhost:8200/annotate.html")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("video", nargs="?", help="eklenecek video dosyası")
    parser.add_argument("--id", help="katalog kimliği (varsayılan: dosya adından)")
    parser.add_argument("--kaynak", help="videonun geldiği URL")
    parser.add_argument("--lisans", help="bilinen lisans/kullanım durumu")
    parser.add_argument("--not", dest="not_", help="neyi gösterdiğine dair kısa not")
    parser.add_argument("--zorla", action="store_true", help="ölçüt uyarılarına rağmen ekle")
    parser.add_argument("--liste", action="store_true", help="kataloğu göster")
    parser.add_argument("--sil", metavar="ID", help="kataloğdan çıkar")
    args = parser.parse_args()

    catalog = load_catalog()

    if args.liste:
        return liste(catalog)
    if args.sil:
        return sil(catalog, args.sil)
    if not args.video:
        parser.print_help()
        return 2

    return ekle(catalog, args)


if __name__ == "__main__":
    raise SystemExit(main())
