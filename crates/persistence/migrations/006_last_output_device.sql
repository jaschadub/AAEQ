-- Add last_output_device column to app_settings
ALTER TABLE app_settings ADD COLUMN last_output_device TEXT;
