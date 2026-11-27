-- A retained per-device high-water mark protects identity independently of event retention.
ALTER TABLE devices ADD COLUMN identity_revision INTEGER NOT NULL DEFAULT 0;
ALTER TABLE devices ADD COLUMN identity_mutation_id TEXT NOT NULL DEFAULT '';
ALTER TABLE devices ADD COLUMN identity_at_ms INTEGER NOT NULL DEFAULT 0;
