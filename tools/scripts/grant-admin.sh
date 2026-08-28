#!/usr/bin/env bash
# İlk admini oluşturur.
#
# Yumurta-tavuk sorunu: /roles sayfasından yetki verilebiliyor, ama o sayfa da
# admin kapısının arkasında. Keto boşken hiç kimse giremiyor, dolayısıyla ilk
# üyelik dışarıdan yazılmak zorunda. Bu script o tek seferlik kapıyı açıyor;
# sonraki yetkilendirmeler arayüzden yapılabilir.
#
# Kullanım:  ./scripts/admin-yetkilendir.sh eposta@ornek.com
set -euo pipefail

KRATOS_ADMIN="${KRATOS_ADMIN:-http://localhost:4434}"
KETO_WRITE="${KETO_WRITE:-http://localhost:4467}"
KETO_READ="${KETO_READ:-http://localhost:4466}"

eposta="${1:-}"
if [[ -z "$eposta" ]]; then
  echo "kullanım: $0 <eposta>" >&2
  exit 1
fi

# E-postadan Kratos kimliğini bul. Keto özneyi kimlik kimliğiyle tanıyor,
# e-postayla değil.
kimlik=$(curl -fsS --max-time 15 "$KRATOS_ADMIN/admin/identities" \
  | python -c "
import json,sys
e=sys.argv[1]
for i in json.load(sys.stdin):
    if (i.get('traits') or {}).get('email')==e:
        print(i['id']); break
" "$eposta")

if [[ -z "$kimlik" ]]; then
  echo "kimlik bulunamadı: $eposta" >&2
  echo "kayıtlı olanlar:" >&2
  curl -fsS --max-time 15 "$KRATOS_ADMIN/admin/identities" \
    | python -c "
import json,sys
for i in json.load(sys.stdin): print('  ', (i.get('traits') or {}).get('email'))
" >&2
  exit 1
fi

curl -fsS --max-time 15 -X PUT "$KETO_WRITE/admin/relation-tuples" \
  -H 'content-type: application/json' \
  -d "{\"namespace\":\"Group\",\"object\":\"admin\",\"relation\":\"members\",\"subject_id\":\"$kimlik\"}" \
  >/dev/null

# Yazdığımızı Keto'ya sorarak doğrula; sessiz başarısızlık en kötüsü olurdu.
izin=$(curl -fsS --max-time 15 \
  "$KETO_READ/relation-tuples/check?namespace=Group&object=admin&relation=members&subject_id=$kimlik" \
  | python -c "import json,sys; print(json.load(sys.stdin)['allowed'])")

if [[ "$izin" != "True" ]]; then
  echo "yazıldı ama Keto hâlâ reddediyor: $eposta ($kimlik)" >&2
  exit 1
fi

echo "admin: $eposta ($kimlik)"