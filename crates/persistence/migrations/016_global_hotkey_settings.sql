-- Migration 016: Global Hotkey Settings
-- Add settings for customizable global hotkey to show/restore window

-- Add hotkey settings columns to app_settings
ALTER TABLE app_settings ADD COLUMN hotkey_enabled INTEGER NOT NULL DEFAULT 1;  -- Boolean: 1 = enabled, 0 = disabled
ALTER TABLE app_settings ADD COLUMN hotkey_modifiers TEXT NOT NULL DEFAULT 'Ctrl+Shift';  -- e.g., 'Ctrl+Shift', 'Alt', 'Ctrl+Alt'
ALTER TABLE app_settings ADD COLUMN hotkey_key TEXT NOT NULL DEFAULT 'A';  -- e.g., 'A', 'Space', 'F1', 'F12'
