CREATE TABLE access_registration_policy(id INTEGER PRIMARY KEY CHECK(id=1),mode TEXT NOT NULL CHECK(mode IN('closed','approval','open')));
INSERT INTO access_registration_policy(id,mode) VALUES(1,'closed');
CREATE TABLE access_registrations(id INTEGER PRIMARY KEY AUTOINCREMENT,user_id INTEGER NOT NULL UNIQUE,username TEXT NOT NULL,display_name TEXT NOT NULL,provider TEXT NOT NULL CHECK(provider IN('password','google','github')),status TEXT NOT NULL CHECK(status IN('active','pending','rejected')),created_at INTEGER NOT NULL,reviewed_at INTEGER,reviewed_by INTEGER,rejection_reason TEXT);
CREATE INDEX access_registrations_status ON access_registrations(status,id);
CREATE TABLE access_registration_events(id INTEGER PRIMARY KEY AUTOINCREMENT,actor_id INTEGER,actor_username TEXT,action TEXT NOT NULL CHECK(action IN('registered','approved','rejected','policy_changed')),registration_id INTEGER,mode TEXT,reason TEXT,created_at INTEGER NOT NULL);
UPDATE access_meta SET version=3 WHERE version=2;
