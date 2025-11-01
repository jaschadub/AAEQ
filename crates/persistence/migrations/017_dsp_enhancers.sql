-- Migration 017: Add DSP Enhancers and Filters to DSP Profile Settings
-- Store enable flags for tone enhancers, dynamic processors, and spatial effects per profile

-- Tone / Character Enhancers
ALTER TABLE dsp_profile_settings ADD COLUMN tube_warmth_enabled INTEGER NOT NULL DEFAULT 0;  -- Boolean: 0 = false, 1 = true
ALTER TABLE dsp_profile_settings ADD COLUMN tape_saturation_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dsp_profile_settings ADD COLUMN transformer_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dsp_profile_settings ADD COLUMN exciter_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dsp_profile_settings ADD COLUMN transient_enhancer_enabled INTEGER NOT NULL DEFAULT 0;

-- Dynamic Processors
ALTER TABLE dsp_profile_settings ADD COLUMN compressor_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dsp_profile_settings ADD COLUMN limiter_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dsp_profile_settings ADD COLUMN expander_enabled INTEGER NOT NULL DEFAULT 0;

-- Spatial & Psychoacoustic
ALTER TABLE dsp_profile_settings ADD COLUMN stereo_width_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dsp_profile_settings ADD COLUMN crossfeed_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dsp_profile_settings ADD COLUMN room_ambience_enabled INTEGER NOT NULL DEFAULT 0;

-- Note: Mutual exclusivity rules enforced in application logic:
-- - Only one tone enhancer can be active at a time
-- - Only one dynamic processor can be active at a time
-- - Spatial effects can be stacked
