-- Add enable_debug_logging column to app_settings
ALTER TABLE app_settings ADD COLUMN enable_debug_logging INTEGER DEFAULT 0;
