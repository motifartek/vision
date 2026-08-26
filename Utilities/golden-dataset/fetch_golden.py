#!/usr/bin/env python3
"""Golden dataset: şartname olay listesiyle hizalanmış gerçek İSG videoları.

Neden bu script var
-------------------
Şartname (3. Senaryo) ve mentör maili sistemin hangi olayları tespit etmesi
gerektiğini sayıyor: alan ihlali, uygunsuz ekipman kullanımı, iş kazası, düşme,
yerde hareketsiz kişi, forklift kaynaklı riskler. Golden dataset'in bu listeyle
**birebir** örtüşmesi gerekiyor; genel amaçlı bir anomali kümesi işimizi
görmüyor.

İki kaynak birleştiriliyor, ikisi de CC BY 4.0 ve gerçek endüstriyel görüntü:

1. UnsafeNet — Eskişehir'de bir organize sanayi bölgesindeki üretim tesisinin
   güvenlik kameralarından, şirket ve çalışan izinleriyle toplanmış. 691 klip,
   1920x1080, 24 fps. **Türk fabrikası** olması ayrıca değerli: finalde
   karşılaşacağımız görüntülerle aynı görsel alan.
   Önal & Dandıl, Data in Brief (2024). https://doi.org/10.1016/j.dib.2024.110819

2. iSafetyBench — fabrika, depo, şantiye ve otoparklardan 1100 klip (420
   tehlikeli, 680 rutin), 4-8 sn. Video-dil benchmark'ı olarak tasarlanmış,
   yani doğrudan VLM değerlendirmesine uygun.
   https://github.com/iSafetyBench/data

Videolar depoya konmuyor (boyut ve telif); bu script çalışma anında indiriyor —
Berat'ın görüntü kümesinde kullandığı desenin aynısı.

Kullanım
--------
    python fetch_golden.py --plan              # ne inecek, ne kadar yer tutacak
    python fetch_golden.py --limit 20          # kategori başına en fazla 20 klip
    python fetch_golden.py --only dusme,forklift_riski

Çıktı `videos/` altına klipler, `catalog.json` içine de her klibin kategorisi,
süresi ve kaynak etiketi.

Zaman damgası notu
------------------
Her iki kaynak da **klip seviyesinde** etiketli; kare seviyesinde zaman damgası
vermiyorlar. Klipler 4-20 saniye ve zaten davranışı içerecek şekilde kırpılmış
olduğu için kritik anın işaretlenmesi kısa bir insan işi. `catalog.json` bu
alanı `event_ms: null` olarak bırakıyor; işaretleme adımı ayrı
(bkz. README.md).
"""

from __future__ import annotations

import argparse
import json
import random
import sys
import urllib.error
import urllib.request
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

HERE = Path(__file__).parent
VIDEO_DIR = HERE / "videos"
CATALOG = HERE / "catalog.json"

UNSAFENET_REPO = "Voxel51/Safe_and_Unsafe_Behaviours"
ISAFETY_REPO = "raiyaanabdullah/isafety-bench"
ISAFETY_ANNOTATIONS = "https://raw.githubusercontent.com/iSafetyBench/data/main"

TIMEOUT = 60
WORKERS = 8

# --- Şartname eşlemesi -------------------------------------------------------
#
# Bu tablo kümenin şartnameyle hizalı olduğunun denetlenebilir kaydı. Kaynak
# etiketleri buradaki kategorilere düşmüyorsa klip **alınmıyor**; genel amaçlı
# bir anomali kümesi değil, hedefli bir küme istiyoruz.

UNSAFENET_MAP = {
    "Safe Walkway Violation": "alan_ihlali",
    "Unauthorized Intervention": "alan_ihlali",
    "Opened Panel Cover": "uygunsuz_ekipman",
    "Carrying Overload with Forklift": "forklift_riski",
    "Safe Walkway": "normal_operasyon",
    "Authorized Intervention": "normal_operasyon",
    "Closed Panel Cover": "normal_operasyon",
    "Safe Carrying": "normal_operasyon",
}

ISAFETY_MAP = {
    # düşme
    "person falling down": "dusme",
    "hanging from something after slip": "dusme",
    # forklift ve ağır ekipman
    "operating forklift": "forklift_riski",
    "operating heavy equipment dangerously": "uygunsuz_ekipman",
    "misusing lift platform": "uygunsuz_ekipman",
    # iş kazası
    "falling load": "is_kazasi",
    "heavy object slipping": "is_kazasi",
    "carrying heavy load": "is_kazasi",
    "body pulled into machine": "is_kazasi",
    "foot stuck in conveyor": "is_kazasi",
    "structural collapse": "is_kazasi",
    "warehouse shelves toppling": "is_kazasi",
    "platform failure": "is_kazasi",
    "vehicle losing control": "is_kazasi",
    "vehicle crash into building or stationary object": "is_kazasi",
    # yangın
    "fire incident": "yangin",
    "extinguishing fire": "yangin",
    # alan ihlali
    "moving in a suspicious manner": "alan_ihlali",
    "intention of theft": "alan_ihlali",
}

# Kazanın kendisi değil, **sonrası**. Ayrı kategori değil; ikincil etiket olarak
# taşınıyorlar çünkü tasarımın dayandığı varsayımı doğruluyorlar: kalabalık bir
# sahnede olayın kendisi zayıf bir sinyal olsa bile, iş durması ve insanların
# toplanması güçlü ve uzun süren bir imza bırakıyor.
AFTERMATH_ACTIONS = {
    "rescue effort",
    "watching incident passively",
    "escaping from danger",
    "calling for help",
}

CATEGORIES = [
    "alan_ihlali",
    "uygunsuz_ekipman",
    "is_kazasi",
    "dusme",
    "forklift_riski",
    "yangin",
    "normal_operasyon",
]


def fetch_json(url: str):
    req = urllib.request.Request(url, headers={"User-Agent": "motifai-golden/1.0"})
    with urllib.request.urlopen(req, timeout=TIMEOUT) as response:
        return json.loads(response.read())


def collect_unsafenet() -> list[dict]:
    """Türk fabrikası kliplerini kataloğa çevirir."""
    url = f"https://huggingface.co/datasets/{UNSAFENET_REPO}/resolve/main/samples.json"
    payload = fetch_json(url)
    samples = payload["samples"] if isinstance(payload, dict) else payload

    items = []
    for sample in samples:
        label = sample["ground_truth"]["label"]
        category = UNSAFENET_MAP.get(label)
        if category is None:
            continue

        meta = sample["metadata"]
        name = Path(sample["filepath"]).name
        items.append(
            {
                "id": f"unsafenet/{name}",
                "source": "unsafenet",
                "category": category,
                "source_label": label,
                "url": f"https://huggingface.co/datasets/{UNSAFENET_REPO}/resolve/main/{sample['filepath']}",
                "duration_ms": round(meta["duration"] * 1000),
                "width": meta["frame_width"],
                "height": meta["frame_height"],
                "fps": meta["frame_rate"],
                "size_bytes": meta["size_bytes"],
                "caption": None,
                "aftermath": [],
                # Kritik an; işaretleme adımında doldurulacak.
                "event_ms": None,
            }
        )
    return items


def collect_isafety() -> list[dict]:
    """iSafetyBench kliplerini kataloğa çevirir."""
    items = []

    for filename, is_hazard in (("annotations_hazard.json", True), ("annotations_normal.json", False)):
        records = fetch_json(f"{ISAFETY_ANNOTATIONS}/{filename}")
        folder = "hazard" if is_hazard else "normal"

        for record in records:
            actions = record.get("gt_actions", [])

            if is_hazard:
                # Şartname kategorisine düşen ilk eylem belirleyici.
                mapped = [ISAFETY_MAP[a] for a in actions if a in ISAFETY_MAP]
                if not mapped:
                    continue
                category = Counter(mapped).most_common(1)[0][0]
            else:
                category = "normal_operasyon"

            name = record["video_name"]
            items.append(
                {
                    "id": f"isafety/{name}",
                    "source": "isafety",
                    "category": category,
                    "source_label": ", ".join(actions),
                    "url": f"https://huggingface.co/datasets/{ISAFETY_REPO}/resolve/main/{folder}/{name}",
                    # Süre/çözünürlük etiketlerde yok; indirdikten sonra ölçülüyor.
                    "duration_ms": None,
                    "width": None,
                    "height": None,
                    "fps": None,
                    "size_bytes": None,
                    # Serbest metin açıklama. Türkçe özet kalitesini
                    # değerlendirirken referans olarak kullanılabilir.
                    "caption": record.get("caption"),
                    "aftermath": sorted(set(actions) & AFTERMATH_ACTIONS),
                    "event_ms": None,
                }
            )

    return items


def select(items: list[dict], limit: int, only: set[str] | None, seed: int) -> list[dict]:
    """Kategori başına dengeli alt küme seçer.

    Rastgele ama **tohumlu**: aynı tohum aynı kümeyi verir, yani ölçümler
    tekrar üretilebilir kalır.
    """
    by_category: dict[str, list[dict]] = defaultdict(list)
    for item in items:
        if only and item["category"] not in only:
            continue
        by_category[item["category"]].append(item)

    rng = random.Random(seed)
    chosen = []
    for category in CATEGORIES:
        bucket = by_category.get(category, [])
        if not bucket:
            continue
        # Yansız seçim: boyuta göre sıralayıp en küçükleri almak indirmeyi
        # küçültürdü ama kümeyi taraflı yapardı — kısa klipler sistematik
        # olarak farklı olaylar olabilir. Golden dataset'te temsil,
        # indirme boyutundan önce gelir.
        rng.shuffle(bucket)
        chosen.extend(bucket[:limit])

    return chosen


def download_one(item: dict) -> tuple[dict, str | None]:
    target = VIDEO_DIR / item["id"].replace("/", "__")
    if target.exists() and target.stat().st_size > 0:
        return item, None

    try:
        req = urllib.request.Request(
            item["url"], headers={"User-Agent": "motifai-golden/1.0"}
        )
        with urllib.request.urlopen(req, timeout=TIMEOUT) as response:
            data = response.read()
    except (urllib.error.URLError, TimeoutError) as err:
        return item, str(err)

    if not data:
        return item, "boş yanıt"

    # Önce geçici dosya, sonra taşı: yarım inen bir klip katalogda
    # "inmiş" görünmesin.
    temp = target.with_suffix(target.suffix + ".part")
    temp.write_bytes(data)
    temp.replace(target)

    item["local_path"] = f"videos/{target.name}"
    item["size_bytes"] = len(data)
    return item, None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--limit", type=int, default=25, help="kategori başına klip")
    parser.add_argument("--only", help="virgülle ayrılmış kategoriler")
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--plan", action="store_true", help="indirmeden özet göster")
    args = parser.parse_args()

    only = set(args.only.split(",")) if args.only else None
    if only:
        bilinmeyen = only - set(CATEGORIES)
        if bilinmeyen:
            print(f"bilinmeyen kategori: {', '.join(sorted(bilinmeyen))}", file=sys.stderr)
            print(f"seçenekler: {', '.join(CATEGORIES)}", file=sys.stderr)
            return 2

    print("etiketler çekiliyor…")
    items = collect_unsafenet() + collect_isafety()
    print(f"  şartname kategorilerine düşen toplam klip: {len(items)}")

    chosen = select(items, args.limit, only, args.seed)

    print("\nseçim:")
    by_category = Counter(x["category"] for x in chosen)
    known = sum(x["size_bytes"] or 0 for x in chosen)
    unknown = sum(1 for x in chosen if not x["size_bytes"])

    for category in CATEGORIES:
        count = by_category.get(category, 0)
        if not count:
            continue
        size = sum(x["size_bytes"] or 0 for x in chosen if x["category"] == category)
        sources = Counter(x["source"] for x in chosen if x["category"] == category)
        detail = " + ".join(f"{v} {k}" for k, v in sources.items())
        print(f"  {category:<20} {count:>3} klip  {size / 1e6:>7.1f} MB   ({detail})")

    print(f"\n  toplam {len(chosen)} klip, bilinen boyut {known / 1e6:.0f} MB", end="")
    if unknown:
        # iSafetyBench boyut bildirmiyor; ölçülen ortalama ~300 KB.
        print(f" + {unknown} klip boyutu bilinmiyor (~{unknown * 0.3:.0f} MB tahmini)")
    else:
        print()

    if args.plan:
        print("\n--plan verildi, indirme yapılmadı.")
        return 0

    VIDEO_DIR.mkdir(parents=True, exist_ok=True)
    print(f"\n{len(chosen)} klip indiriliyor…")

    ok, failed = [], []
    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        futures = {pool.submit(download_one, item): item for item in chosen}
        for i, future in enumerate(as_completed(futures), 1):
            item, error = future.result()
            if error:
                failed.append((item["id"], error))
            else:
                ok.append(item)
            if i % 20 == 0 or i == len(chosen):
                print(f"  {i}/{len(chosen)}")

    CATALOG.write_text(
        json.dumps(
            {
                "sources": {
                    "unsafenet": {
                        "license": "CC BY 4.0",
                        "citation": "Önal & Dandıl, Data in Brief (2024), doi:10.1016/j.dib.2024.110819",
                        "note": "Eskişehir organize sanayi bölgesi, izinle toplanmış",
                    },
                    "isafety": {
                        "license": "CC BY 4.0",
                        "citation": "iSafetyBench, arXiv:2508.00399",
                        "url": "https://github.com/iSafetyBench/data",
                    },
                },
                "categories": CATEGORIES,
                "clips": ok,
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )

    print(f"\nindi: {len(ok)} · başarısız: {len(failed)}")
    for name, error in failed[:10]:
        print(f"  ! {name}: {error}")
    print(f"katalog: {CATALOG}")
    print("\nsıradaki adım: kritik anların işaretlenmesi (bkz. README.md)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
