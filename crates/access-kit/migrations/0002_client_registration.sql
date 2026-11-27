-- Existing client registrations remain administratively managed.
ALTER TABLE access_clients ADD COLUMN public_registration INTEGER NOT NULL DEFAULT 0 CHECK(public_registration IN (0,1));
UPDATE access_meta SET version=2 WHERE version=1;
