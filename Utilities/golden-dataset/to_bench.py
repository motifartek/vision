#!/usr/bin/env python3
"""Kataloğu `tools/bench` girdisine dönüştürür.

Katalog tek dosyada tüm videoları tutuyor; bench ise video başına bir
`GroundTruth` JSON'u ve yanında video dosyasını bekliyor (sentetik kümede
`bench generate` böyle üretiyor). Bu script aradaki çeviriyi yapıyor.

Yalnızca **etiketlenmiş** videolar yazılıyor: olayı olmayan bir kayıt event
coverage recall'a katılamaz, katılırsa da sonucu yanıltır.

    python to_bench.py                    # videos/ içine yaz
    python to_bench.py --out <dizin>      # başka bir yere yaz
"""

from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path

HERE = Path(__file__).parent
CATALOG = HERE / "catalog.json"
VIDEO_DIR = HERE / "videos"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--out", type=Path, default=VIDEO_DIR,
                        help="ground truth dosyalarının yazılacağı dizin")
    parser.add_argument("--min-guven", default="orta",
                        choices=["yuksek", "orta"],
                        help="bu güven düzeyinin altındakileri dışarıda bırak")
    args = parser.parse_args()

    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
    sirali = {"yuksek": 2, "orta": 1}
    esik = sirali[args.min_guven]

    args.out.mkdir(parents=True, exist_ok=True)
    yazilan, atlanan = [], []

    for v in catalog["videos"]:
        gt = v.get("ground_truth") or {}
        events = gt.get("events") or []
        guven = (v.get("annotation") or {}).get("guven", "")

        if not events:
            atlanan.append((v["id"], "etiketlenmemiş"))
            continue
        if sirali.get(guven, 0) < esik:
            atlanan.append((v["id"], f"güven '{guven}' eşiğin altında"))
            continue

        video_dosyasi = Path(v["local_path"]).name
        # Bench, videoyu ground truth ile aynı dizinde arıyor.
        if args.out.resolve() != VIDEO_DIR.resolve():
            shutil.copy2(VIDEO_DIR / video_dosyasi, args.out / video_dosyasi)

        (args.out / f"{v['id']}.json").write_text(
            json.dumps(
                {
                    "video": video_dosyasi,
                    "duration_ms": v["duration_ms"],
                    "notes": gt.get("summary"),
                    "events": [
                        {
                            "t_ms": e["t_ms"],
                            "time": e["time"],
                            "event": e["event"],
                            "severity": e["severity"],
                        }
                        for e in events
                    ],
                },
                ensure_ascii=False,
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        yazilan.append((v["id"], len(events)))

    print(f"{len(yazilan)} video yazıldı -> {args.out}")
    for name, n in yazilan:
        print(f"  {name:<24} {n} olay")

    if atlanan:
        print(f"\n{len(atlanan)} video dışarıda:")
        for name, sebep in atlanan:
            print(f"  {name:<24} {sebep}")

    toplam = sum(n for _, n in yazilan)
    print(f"\ntoplam {toplam} olay")
    print(f"\n  cargo run --release -p motif-bench -- run --dataset {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
