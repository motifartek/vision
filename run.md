# Çalıştırma — sırayla

Sıfırdan kuran biri için. Rust kurmanıza **gerek yok**, derleme konteynerlerin
içinde oluyor.


## Gerekenler

- **Docker Desktop** (çalışır durumda)
- **Node.js 20+ ve pnpm** — yalnız panel için (`npm i -g pnpm`)

## 1. Anahtarları koyun

```bash
cp platform/docker/.env.example platform/docker/.env
```

Dört değeri doldurun: `EVREN_KEY`, `EVREN_LLM_URL`, `QDRANT_URL`, `QDRANT_KEY`.
`EVREN_KEY` boşsa compose hiç açılmaz, hata verir.

## 2. Yığını kaldırın

```bash
docker compose -f platform/docker/compose.yaml up -d --build
```

İlk sefer uzun sürer: servisler derlenir ve sonic'in model ağırlıkları
(~500 MB) indirilir. Sonraki açılışlar ağa hiç çıkmaz.

Hazır olduğunu görmek için:

```bash
docker compose -f platform/docker/compose.yaml ps
```

## 3. Paneli başlatın

```bash
pnpm --dir apps/dashboard install
```

```bash
pnpm --dir apps/dashboard dev
```

Panel **Docker'da değil**, ana makinede çalışır — bu yüzden Docker Desktop'ta
göremezsiniz.

## 4. Arayüzü açın

**http://localhost:3001**

## 5. Hesap açın

"Kayıt Ol" ile üye olun. Doğrulama e-postası dışarı gitmez, yerel posta
kutusuna düşer:

**http://localhost:4436** — MailSlurper. Koddaki 6 haneli sayıyı panele girin.

Kodu komut satırından da okuyabilirsiniz:

```bash
curl -s http://127.0.0.1:4437/mail | grep -oE "[0-9]{6}"
```

## 6. Admin yetkisi

```bash
make admin EMAIL=eposta@ornek.com
```

> Bu script `python` çağırıyor; Python kurulu değilse çalışmaz. Bilinen bir
> eksik, henüz düzeltilmedi.

---

## İsteğe bağlı: hızlı mod (yalnız Windows)

Ses analizini ~2 kat hızlandırır. Gerekmez — CPU ile 10 dakikalık kayıt zaten
~13 saniyede bitiyor.

Bu mod için **Rust** ve **Windows SDK** gerekiyor (yukarıdaki temel kurulumda
gerekmiyordu). Script ikisini de baştan denetliyor, eksikse ne yapılacağını
söylüyor.

```powershell
.\tools\scripts\setup-model-host.ps1
```

```powershell
$env:SONIC_DML_DEVICE = "1"
.\target\release\model-host.exe
```

Bu pencere açık kalmalı. Sonra yığını şu katmanla kaldırın:

```bash
docker compose -f platform/docker/compose.yaml \
               -f apps/ai/sonic/compose.modelhost.yaml up -d
```

Ölçüm (11 dk 58 sn'lik kayıt): Docker CPU 13,2 sn → host DirectML 6,5 sn.
Ayrıntı: `apps/ai/sonic/README.md`.

Geri dönmek için `-f apps/ai/sonic/compose.modelhost.yaml` katmanını kaldırın.

---

## Adresler

| Adres | Ne |
|---|---|
| http://localhost:3001 | **Panel** |
| http://localhost:4436 | MailSlurper — doğrulama e-postaları |
| http://localhost:5000 | Grafana |
| http://localhost:9090 | Prometheus |
| http://localhost:8100/healthz | stream |
| http://localhost:8110/healthz | vision |
| http://127.0.0.1:8081/healthz | sonic |

## Durdurmak

```bash
docker compose -f platform/docker/compose.yaml down
```

Videolar ve model ağırlıkları Docker birimlerinde kalır. Onları da silmek için
`down -v` — ama o zaman model yeniden inecek.
