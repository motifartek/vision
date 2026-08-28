-- Yalnızca veri dizini boşken çalışır (docker-entrypoint-initdb.d kuralı).
-- Şema değişirse konteyneri `docker compose down -v` ile sıfırlamak gerekiyor.
--
-- Not: prompt_override tablosu burada değil; `motif-database` açılışta kendisi
-- kuruyor, çünkü vision veritabanısız da ayağa kalkabilmeli.

CREATE TABLE IF NOT EXISTS ai_events (
    video_id VARCHAR(255) PRIMARY KEY,
    summary TEXT,
    events JSONB,
    risk VARCHAR(50),
    actions JSONB
);

CREATE TABLE IF NOT EXISTS external_tools (
    id SERIAL PRIMARY KEY,
    name VARCHAR(50) UNIQUE NOT NULL,
    title VARCHAR(100) NOT NULL,
    description TEXT NOT NULL
);

INSERT INTO external_tools (name, title, description) VALUES
('call_ambulance', 'Ambulans Çağır', 'Acil bir tıbbi durum algılandığında ambulans yönlendirmesi yapar.'),
('notify_police', 'Polise Haber Ver', 'Güvenlik ihlali veya şiddet durumunda kolluk kuvvetlerine otomatik bildirim yapar.'),
('lock_doors', 'Kapıları Kilitle', 'Tehlike anında tüm elektronik kapıları otomatik olarak kilitler.')
ON CONFLICT (name) DO NOTHING;

CREATE TABLE IF NOT EXISTS chat_sessions (
    id VARCHAR(255) PRIMARY KEY,
    video_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS chat_messages (
    id SERIAL PRIMARY KEY,
    session_id VARCHAR(255) REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role VARCHAR(50) NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
