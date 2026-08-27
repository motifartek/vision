CREATE TABLE IF NOT EXISTS ai_events (
    video_id VARCHAR(255) PRIMARY KEY,
    summary TEXT,
    events JSONB,
    risk VARCHAR(50),
    actions JSONB
);
