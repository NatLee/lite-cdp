CREATE TABLE users (
    id            INTEGER PRIMARY KEY,
    username      TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    display_name  TEXT    NOT NULL,
    password_hash TEXT    NOT NULL,
    role          TEXT    NOT NULL DEFAULT 'member' CHECK (role IN ('admin', 'member')),
    disabled      INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT    NOT NULL
);

-- 只存 session token 的 SHA-256
CREATE TABLE sessions (
    token_hash TEXT    PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    expires_at TEXT    NOT NULL,
    created_at TEXT    NOT NULL
);

-- 一個被追蹤的網站一筆；追蹤程式碼用 key 送資料進來
CREATE TABLE sites (
    id              INTEGER PRIMARY KEY,
    name            TEXT    NOT NULL UNIQUE,
    url             TEXT    NOT NULL DEFAULT '',
    key             TEXT    NOT NULL UNIQUE,
    -- JSON 陣列：只收哪些 origin 的瀏覽器送來的資料；空陣列是不限制
    allowed_origins TEXT    NOT NULL DEFAULT '[]',
    active          INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT    NOT NULL
);

-- 誰看得到哪個網站的資料（管理員全部都看得到）
CREATE TABLE site_members (
    site_id INTEGER NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    PRIMARY KEY (site_id, user_id)
);
CREATE INDEX site_members_user ON site_members (user_id);

-- 一個人一筆。email 是空字串的是還不知道是誰的匿名訪客
CREATE TABLE people (
    id            INTEGER PRIMARY KEY,
    site_id       INTEGER NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    -- 一律存小寫
    email         TEXT    NOT NULL DEFAULT '',
    name          TEXT    NOT NULL DEFAULT '',
    -- 在對方系統裡的帳號或會員編號
    account       TEXT    NOT NULL DEFAULT '',
    -- JSON 物件：identify 時帶進來的其他資料
    traits        TEXT    NOT NULL DEFAULT '{}',
    -- 瀏覽了幾頁；跟著 events 一起更新，排序和分群才不用每次重算
    pageviews     INTEGER NOT NULL DEFAULT 0,
    first_seen    TEXT    NOT NULL,
    last_seen     TEXT    NOT NULL,
    identified_at TEXT
);
CREATE UNIQUE INDEX people_email ON people (site_id, email) WHERE email != '';
CREATE INDEX people_last_seen ON people (site_id, last_seen);

-- 一個瀏覽器一筆（追蹤程式碼存在 localStorage 的編號）；同一個人可以有好幾個
CREATE TABLE devices (
    id         INTEGER PRIMARY KEY,
    site_id    INTEGER NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    anon_id    TEXT    NOT NULL,
    person_id  INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    user_agent TEXT    NOT NULL DEFAULT '',
    first_seen TEXT    NOT NULL,
    last_seen  TEXT    NOT NULL,
    UNIQUE (site_id, anon_id)
);
CREATE INDEX devices_person ON devices (person_id);

CREATE TABLE events (
    id            INTEGER PRIMARY KEY,
    site_id       INTEGER NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    person_id     INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    device_id     INTEGER REFERENCES devices (id) ON DELETE SET NULL,
    -- 同一個分頁從打開到關掉是同一個 session
    session_id    TEXT    NOT NULL DEFAULT '',
    type          TEXT    NOT NULL CHECK (type IN ('pageview', 'click', 'scroll', 'custom', 'identify')),
    -- 自訂事件的名稱、點到的文字、捲到幾 %、認出來的 email
    name          TEXT    NOT NULL DEFAULT '',
    url           TEXT    NOT NULL DEFAULT '',
    -- 網址去掉網域、參數和結尾的斜線；頁面用它分
    path          TEXT    NOT NULL DEFAULT '',
    title         TEXT    NOT NULL DEFAULT '',
    referrer      TEXT    NOT NULL DEFAULT '',
    -- 從別的網站連進來時，那個網站的網域（小寫、去掉 www.）；站內換頁是空字串
    referrer_host TEXT    NOT NULL DEFAULT '',
    -- JSON 物件
    props         TEXT    NOT NULL DEFAULT '{}',
    created_at    TEXT    NOT NULL
);
CREATE INDEX events_site_time ON events (site_id, created_at);
CREATE INDEX events_person ON events (person_id, id);
CREATE INDEX events_site_path ON events (site_id, type, path);

CREATE TABLE labels (
    id          INTEGER PRIMARY KEY,
    site_id     INTEGER NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    name        TEXT    NOT NULL,
    color       TEXT    NOT NULL DEFAULT 'neutral',
    description TEXT    NOT NULL DEFAULT '',
    -- 符合規則的事件要有幾次，才算有這個標籤
    min_hits    INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT    NOT NULL,
    UNIQUE (site_id, name)
);

-- 標籤的規則：事件符合任何一條就算一次
CREATE TABLE label_rules (
    id       INTEGER PRIMARY KEY,
    label_id INTEGER NOT NULL REFERENCES labels (id) ON DELETE CASCADE,
    kind     TEXT    NOT NULL CHECK (kind IN ('path_prefix', 'url_contains', 'title_contains', 'event')),
    value    TEXT    NOT NULL,
    UNIQUE (label_id, kind, value)
);

-- 每個人符合每個標籤幾次。收事件時累加；規則改了就整個標籤重算
CREATE TABLE person_labels (
    person_id INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    label_id  INTEGER NOT NULL REFERENCES labels (id) ON DELETE CASCADE,
    hits      INTEGER NOT NULL,
    last_at   TEXT    NOT NULL,
    PRIMARY KEY (person_id, label_id)
);
CREATE INDEX person_labels_label ON person_labels (label_id);

-- 存起來的分群條件；名單每次打開時現算
CREATE TABLE segments (
    id          INTEGER PRIMARY KEY,
    site_id     INTEGER NOT NULL REFERENCES sites (id) ON DELETE CASCADE,
    name        TEXT    NOT NULL,
    description TEXT    NOT NULL DEFAULT '',
    -- all：所有人；member：知道 email 的；anonymous：還不知道是誰的
    kind        TEXT    NOT NULL DEFAULT 'all' CHECK (kind IN ('all', 'member', 'anonymous')),
    -- JSON：{"must": [...], "should": [...], "must_not": [...]}
    rules       TEXT    NOT NULL,
    created_by  INTEGER REFERENCES users (id) ON DELETE SET NULL,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL,
    UNIQUE (site_id, name)
);
