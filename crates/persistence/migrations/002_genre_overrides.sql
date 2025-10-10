-- Genre overrides for tracks
-- Allows users to manually specify genre for tracks that don't have genre metadata
CREATE TABLE IF NOT EXISTS genre_override (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_key TEXT NOT NULL UNIQUE,  -- Format: "artist|title|album|genre"
    genre TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_genre_override_track_key ON genre_override(track_key);
