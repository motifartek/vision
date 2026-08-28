# Motif

Motif, çoklu yapay zeka ajanlarının (Vision, Humanizer, Sonic, Toolbox vb.) bir arada çalıştığı, video analiz ve doküman üretim platformudur. Kullanıcıların yüklediği videolar, farklı özelliklere sahip yapay zeka modelleri tarafından paralel olarak incelenir, detaylı bir analiz raporu sunulur ve gerektiğinde dilekçe/tutanak gibi yasal belgeler üretilir.

## 📚 Dokümantasyon

Mimari ve projenin detaylı çalışma prensipleri için `documents/` klasöründeki kaynakları inceleyebilirsiniz:

- [Mimari Kararlar ve Yapı](documents/architecture/)
- [Sistem Özellikleri](documents/features/)
- [Performans Ölçümleri](documents/measurements/)
- [Prompt Sistemi ve Özelleştirmeler](documents/prompt-system-overview.md)

## 🚀 Çalışma Pipeline'ı

1. **Video Yükleme & Stream:** Kullanıcı videoyu yükler. `gateway` servisi bu isteği alır ve NATS üzerinden ilgili servislere duyurur.
2. **Klip ve Kare Çıkarımı:** Sistem videoyu saniyelik parçalara (frame) ve daha küçük kliplere ayırır.
3. **Vision Analizi:** `vision` (Görsel Model) ajanı bu kareleri inceleyerek bir olay örgüsü (timeline) oluşturur ve potansiyel aksiyon tavsiyelerinde bulunur.
4. **Humanizer & Toolbox Entegrasyonu:** VLM'den çıkan rapor, `humanizer` ajanı (Büyük Dil Modeli) tarafından okunur.
   - Humanizer, kullanıcının `/prompts` sayfasında belirlediği kurallara ve bağlama uygun şekilde bu ham raporu anlamlı bir **Asistan Anlatımına** çevirir.
   - LLM ayrıca veritabanındaki **Araçları (Tools)** görebilir. Eğer olay anında ambulans çağırmak, polis çağırmak gibi gerçek bir API/veritabanı aksiyonuna ihtiyaç duyarsa bunu "Araç Çağrısı" olarak sisteme önerir. Kullanıcı bu aracı UI üzerinden onayladığında, `toolbox` servisi aracılığıyla API çağrısı gerçekleşir.
5. **Belge Üretimi:** Analiz edilen video baz alınarak, Humanizer üzerinden Dilekçe veya Tutanak üretimi tetiklenebilir.

## 🛠️ Kurulum ve Başlatma

Proje, hem Docker tabanlı tam izole bir altyapıya hem de lokal (cargo) geliştirme için kısayollara sahiptir. Geliştirme akışını `make` komutları üzerinden yönetebilirsiniz.

### Ön Koşullar

- Docker & Docker Compose
- Rust (Lokal geliştirme için)
- Node.js & pnpm (Dashboard için)

### 1. Altyapıyı Başlatma (Infrastructure)

Kratos (Kimlik doğrulama), Keto (Yetkilendirme), NATS, PostgreSQL vb. tüm altyapı servislerini başlatmak için:

```bash
make infra:up
```

Bu komut `platform/docker/compose.yaml` içerisindeki bağımlılıkları ayağa kaldırır. Altyapıyı durdurmak veya loglarını izlemek için:
- `make infra:down`
- `make infra:logs`

### 2. Uygulamaları Başlatma (Servisler)

Altyapı çalıştıktan sonra uygulama servislerini (Gateway, Dashboard, Vision, Humanizer vs.) Docker üzerinden başlatabilirsiniz:

```bash
# Tüm servisleri Docker'da çalıştırır
make run:dev

# Sadece belirli bir servisi çalıştırır
make run:dev APP=dashboard

# Gateway dışındaki tüm servisleri çalıştırır
make run:dev EXCLUDE=gateway
```

### 3. Lokal Geliştirme (Cargo Watch)

Eğer bir serviste (Örn: `humanizer`) kod geliştiriyorsanız ve anında derlenmesini (hot-reload) istiyorsanız:

```bash
make watch APP=humanizer
```

Bu komut:
1. Docker içindeki `humanizer` konteynerini durdurur (Port çakışmasını engellemek için).
2. Sizin lokalinizde `cargo watch` komutunu başlatarak, kod değiştikçe servisi lokalden tekrar derler ve ayağa kaldırır.

*(İlgili projenin .env dosyaları `platform/docker/.env` üzerinden veya `.cargo/config.toml` ile otomatik okunmaktadır.)*

### 4. İlk Admin Kullanıcısını Oluşturma

Sistemde henüz bir yetki mekanizması (Keto) bulunmadığı için ilk admin kullanıcısını CLI üzerinden eklemelisiniz. Uygulamaya kayıt olduktan sonra email adresinize admin yetkisi vermek için:

```bash
make admin EMAIL=eposta@ornek.com
```

Bu yetkiyi verdikten sonra `/roles` sayfasına erişebilir ve diğer kullanıcıların yetkilerini UI üzerinden yönetebilirsiniz.
