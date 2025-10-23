-- Add icon and color fields to profile table for visual identification

-- Add icon column (emoji) with default
ALTER TABLE profile ADD COLUMN icon TEXT NOT NULL DEFAULT '📁';

-- Add color column (hex code) with default gray
ALTER TABLE profile ADD COLUMN color TEXT NOT NULL DEFAULT '#808080';

-- Update built-in profiles with appropriate icons and colors
UPDATE profile SET icon = '🏠', color = '#4A90E2' WHERE name = 'Default';  -- Blue
UPDATE profile SET icon = '🎧', color = '#9B59B6' WHERE name = 'Headphones';  -- Purple
