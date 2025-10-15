-- Add last_input_device column to app_settings
ALTER TABLE app_settings ADD COLUMN last_input_device TEXT;
