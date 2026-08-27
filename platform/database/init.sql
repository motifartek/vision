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
('call_ambulance', 'Ambulans Çaðýr', 'Acil bir týbbi durum algýlandýðýnda ambulans yönlendirmesi yapar.'),
('notify_police', 'Polise Haber Ver', 'Güvenlik ihlali veya þiddet durumunda kolluk kuvvetlerine otomatik bildirim yapar.'),
('lock_doors', 'Kapýlarý Kilitle', 'Tehlike anýnda tüm elektronik kapýlarý otomatik olarak kilitler.')
ON CONFLICT (name) DO NOTHING;
