ALTER TABLE sites ADD COLUMN collection_policy TEXT NOT NULL DEFAULT '{}';
ALTER TABLE sites ADD COLUMN identity_secret TEXT NOT NULL DEFAULT '';
UPDATE sites SET identity_secret = lower(hex(randomblob(32)));
ALTER TABLE people ADD COLUMN user_id TEXT NOT NULL DEFAULT '';
ALTER TABLE people ADD COLUMN identity_verified INTEGER NOT NULL DEFAULT 0;
DROP INDEX people_email;
CREATE UNIQUE INDEX people_legacy_email ON people(site_id,email) WHERE email != '' AND user_id = '';
CREATE UNIQUE INDEX people_user_id ON people(site_id,user_id) WHERE user_id != '';
ALTER TABLE events ADD COLUMN happened_at TEXT;
ALTER TABLE events ADD COLUMN event_id TEXT NOT NULL DEFAULT '';
ALTER TABLE events ADD COLUMN received_at TEXT NOT NULL DEFAULT '';
UPDATE events SET received_at = created_at;
CREATE UNIQUE INDEX events_deduplicate ON events(site_id,event_id) WHERE event_id != '';
CREATE TABLE installation_tests (
 token TEXT PRIMARY KEY,
 site_id INTEGER NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
 expires_at TEXT NOT NULL,
 receipt TEXT,
 received_at TEXT
);
CREATE INDEX installation_tests_site ON installation_tests(site_id,expires_at);
