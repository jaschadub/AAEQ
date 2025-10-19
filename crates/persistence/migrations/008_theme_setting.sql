-- Add theme column to app_settings
ALTER TABLE app_settings ADD COLUMN theme TEXT DEFAULT 'dark';
