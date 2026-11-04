-- v1.0.0: extraction is deterministic and selector-driven.
-- The optional AI provider registry and its encrypted credentials are removed.
-- Existing rows are dropped; no API keys or model settings are retained.
DROP INDEX IF EXISTS idx_ai_providers_default;
DROP TABLE IF EXISTS ai_providers;
