-- App settings table for storing application preferences
CREATE TABLE IF NOT EXISTS app_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),  -- Ensure only one row
    last_connected_host TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Insert default settings row
INSERT OR IGNORE INTO app_settings (id, last_connected_host, created_at, updated_at)
VALUES (1, NULL, strftime('%s', 'now'), strftime('%s', 'now'));
