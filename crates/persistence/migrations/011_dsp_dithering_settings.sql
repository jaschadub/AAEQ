-- Migration 011: Add Dithering Settings to DSP Profile Settings
-- Store dithering configuration (enabled, mode, noise shaping, target bits) per profile

-- Add dithering columns to existing dsp_profile_settings table
ALTER TABLE dsp_profile_settings ADD COLUMN dither_enabled INTEGER NOT NULL DEFAULT 0;  -- Boolean: 0 = false, 1 = true
ALTER TABLE dsp_profile_settings ADD COLUMN dither_mode TEXT NOT NULL DEFAULT 'Triangular';  -- DitherMode: None, Rectangular, Triangular, Gaussian
ALTER TABLE dsp_profile_settings ADD COLUMN noise_shaping TEXT NOT NULL DEFAULT 'None';  -- NoiseShaping: None, FirstOrder, SecondOrder, Gesemann
ALTER TABLE dsp_profile_settings ADD COLUMN target_bits INTEGER NOT NULL DEFAULT 16;  -- Target bit depth: 16, 24, or 32
