# API

對外 `POST /api/v1/{operation}`、`/mcp` 與 OAuth 的 scopes、Bearer 和真實用量語義見 [存取指南](access.md)。下列既有管理 API 使用原 HttpOnly cookie；Bearer 不可用來管理帳號或綁定。

網址分工：

- `/collect` 和 `/t.js`：收追蹤資料的公開網址和追蹤程式碼，不用登入，別的網站的瀏覽器會直接連過來。
- `/api/...`：管理介面用的 API，要先登入（HttpOnly cookie `cdp_session`）。

錯誤一律是 `{"error": "給人看的訊息"}` 加上對應的 HTTP 狀態碼。時間都是 UTC 的 RFC 3339（`2026-01-05T08:30:00Z`）。

## 收資料

### `GET /t.js`

追蹤程式碼。用法見 [README](../README.md#追蹤程式碼)。

### `POST /collect`

追蹤程式碼會自己送，一般不用直接呼叫；要從伺服器或 App 送資料時才需要。

內容是 JSON，最多 64 KB。不看 `Content-Type`（追蹤程式碼用 `text/plain` 送，才不會觸發 CORS 預檢）。

```json
{
  "key": "追蹤金鑰",
  "anon_id": "這個瀏覽器的匿名編號",
  "session_id": "這次工作階段的編號",
  "events": [
    {
      "type": "pageview",
      "url": "https://shop.example.com/products/coffee-beans",
      "title": "咖啡豆",
      "referrer": "https://www.example.org/"
    },
    { "type": "custom", "name": "add_to_cart", "url": "https://shop.example.com/cart", "props": { "sku": "A-1001" } }
  ],
  "identify": { "email": "amy@example.com", "name": "Amy", "account": "amy01", "traits": { "plan": "pro" } }
}
```

| 欄位 | 說明 |
|---|---|
| `key` | 網站的追蹤金鑰，必填 |
| `anon_id` | 必填。同一個瀏覽器（或裝置）要一直用同一個，自己產生一個隨機字串就好：8 到 64 個字元，只能用英數字、`-` 和 `_` |
| `session_id` | 可以不給。同一次造訪的事件用同一個，時間軸會照它分段。最多 64 個字元，可以用的字元同上 |
| `events` | 一次最多 50 個，可以是空的（只送 `identify`）|
| `events[].type` | `pageview`、`click`、`scroll`、`custom` |
| `events[].name` | `custom` 是事件名稱；`click` 是按鈕或連結的文字（或 `data-cdp-click` 的值）|
| `events[].url` | 事件發生在哪一頁，最多 2000 個字元 |
| `events[].title`、`events[].referrer` | 頁面標題、從哪裡連進來 |
| `events[].props` | 任意的 JSON 物件，最多 2 KB |
| `identify` | 可以不給。有 `email` 時，這個 `anon_id` 之前的紀錄會併到這個 email 的人身上 |
| `identify.traits` | 自訂屬性，最多 20 個 |

成功回 `204`，沒有內容。

| 狀態碼 | 什麼時候 |
|---|---|
| `400` | 不是合法的 JSON、少了必填欄位、不認得的事件種類、一次超過 50 個事件 |
| `403` | 網站設了「允許的來源」而 `Origin` 不在裡面；或這個網站已經停止收資料 |
| `404` | 沒有這個追蹤金鑰（換過金鑰之後，舊的也是 404）|
| `413` | 內容超過 64 KB |
| `429` | 同一個 IP 送得太頻繁（`CDP_RATE_LIMIT`，預設一分鐘 600 次）|

回應一律帶 `Access-Control-Allow-Origin: *`；要不要收是看網站的「允許的來源」。沒有 `Origin` 標頭的請求（伺服器之間、命令列）不受來源限制。`User-Agent` 看起來是爬蟲的，照樣回 `204` 但不存。IP 只用來限制流量，不會存下來。

```bash
curl -i https://cdp.example.com/collect -d '{
  "key": "<追蹤金鑰>",
  "anon_id": "server-7f3a9c1e",
  "events": [{ "type": "custom", "name": "order_paid", "url": "https://shop.example.com/checkout/done" }],
  "identify": { "email": "amy@example.com" }
}'
```

## 管理 API

除了 `health`、`meta`、`auth/state`、`auth/setup`、`auth/login` 之外都要登入，沒登入回 `401`。會改資料的請求如果帶了 `Origin` 而且不是本站，回 `403`。

「管理員」欄打勾的只有 `admin` 能用。成員（`member`）只看得到指定給他的網站；看不到的網站回 `404`。

### 基本

| 方法 | 路徑 | 說明 |
|---|---|---|
| `GET` | `/api/health` | 回 `ok` |
| `GET` | `/api/meta` | `{version, public_url, retention_days}` |

### 登入和帳號

| 方法 | 路徑 | 管理員 | 說明 |
|---|---|---|---|
| `GET` | `/api/auth/state` | | `{setup_required, user}`；`setup_required` 是還沒有任何帳號 |
| `POST` | `/api/auth/setup` | | 建立第一個管理員並登入：`{username, password, display_name?}`。已經有帳號之後回 `409` |
| `POST` | `/api/auth/login` | | `{username, password}`，回使用者並設定 cookie。同一個帳號連續失敗 5 次會暫時鎖住 |
| `POST` | `/api/auth/logout` | | |
| `PATCH` | `/api/auth/me` | | `{display_name?}` |
| `POST` | `/api/auth/password` | | `{current_password, new_password}`，其他裝置上的登入會一起登出 |
| `GET` | `/api/users` | ✓ | 所有帳號，含 `disabled`、`site_count` |
| `POST` | `/api/users` | ✓ | `{username, password, role?, display_name?}`，`role` 是 `admin` 或 `member`（預設）|
| `PATCH` | `/api/users/<id>` | ✓ | `{display_name?, role?, disabled?, password?}`；不能改自己的角色或停用自己 |

密碼至少 8 個字元，用 Argon2 雜湊。

### 網站

| 方法 | 路徑 | 管理員 | 說明 |
|---|---|---|---|
| `GET` | `/api/sites` | | 看得到的網站，含 `key`、`people`、`identified`、`last_event_at`、`members` |
| `POST` | `/api/sites` | ✓ | 至少要 `name`，其他欄位同下 |
| `GET` | `/api/sites/<id>` | | |
| `PATCH` | `/api/sites/<id>` | ✓ | 只改有給的欄位 |
| `DELETE` | `/api/sites/<id>` | ✓ | 連同底下所有的人、事件、標籤和分群一起刪 |
| `POST` | `/api/sites/<id>/rotate-key` | ✓ | 換一把追蹤金鑰，舊的立刻失效 |

可以改的欄位：

| 欄位 | 說明 |
|---|---|
| `name` | 1 到 60 個字元，不能重複 |
| `url` | 網站的網址，可以空著 |
| `allowed_origins` | 只收這些來源送來的資料，例如 `["https://shop.example.com"]`；空的就不限制。最多 20 個 |
| `active` | `false` 就停收，已經收到的都還在 |
| `member_ids` | 看得到這個網站的成員 |

以下的路徑都在 `/api/sites/<id>` 底下。

### 總覽

`GET /overview?tz_offset=480` 回總覽頁用的數字。`tz_offset` 是瀏覽器的時區比 UTC 快幾分鐘（台北是 `480`），每天的數字照這個時區切。

| 欄位 | 說明 |
|---|---|
| `engagement` | 三筆 `{activity, member, anonymous}`，依 `recent`、`cooling`、`dormant` 排列，包含零值；按目前 `email` 與 `last_seen` 分組 |
| `totals` | `{people, identified, anonymous}` |
| `last_7_days`、`previous_7_days` | `{visitors, pageviews, new_people, identified}` |
| `daily` | 最近 30 天每天的 `{date, pageviews, visitors}`，舊的在前 |
| `top_pages` | `{path, title, pageviews, visitors}` |
| `top_referrers` | `{host, visitors}` |
| `labels` | 每個標籤現在有幾個人 |
| `recent_identified` | 最近被認出來的人 |

每日 `visitors` 在同一天去重，不能跨日加總為整段期間的獨立訪客。`engagement` 是目前資料狀態，不是歷史轉換率；資料保留政策會影響可見資料。

### 人

| 方法 | 路徑 | 管理員 | 說明 |
|---|---|---|---|
| `GET` | `/people` | | 清單，回 `{items, total, page, per_page}` |
| `GET` | `/people/export` | | 同樣的篩選條件，匯出 CSV（最多 50,000 人）|
| `GET` | `/people/<pid>` | | 一個人：基本資料加上 `traits`、`events`、`sessions`、`labels`（含 `hits`）、`devices`、`referrers`、`top_pages` |
| `GET` | `/people/<pid>/events` | | 時間軸，新的在前，回 `{items, next_before}` |
| `DELETE` | `/people/<pid>` | ✓ | 刪掉這個人和他所有的事件。他的瀏覽器下次再來會被當成新的匿名訪客 |

清單的查詢參數：

| 參數 | 說明 |
|---|---|
| `kind` | `member`（知道 email 的）、`anonymous`（還不知道是誰的）；沒給是全部 |
| `activity` | `recent`：最後造訪距今 ≤ 7 天；`cooling`：> 7 天且 ≤ 30 天；`dormant`：> 30 天。三組互斥，以 UTC 連續時間計算，清單與 CSV 皆支援 |
| `q` | email、名稱或帳號含有這段文字 |
| `label_id` | 有這個標籤的人 |
| `segment_id` | 符合這個分群的人 |
| `sort` | `last_seen`（預設）、`first_seen`、`pageviews` |
| `page`、`per_page` | `per_page` 預設 25，最多 100 |

時間軸的查詢參數：`limit`（預設 50，最多 200）、`type`（只看某一種事件）、`before`（上一次回的 `next_before`，用來往更早翻；`null` 就是沒有了）。

### 頁面

`GET /pages?days=30&q=` 回最近幾天被看過的頁面：`{path, title, url, pageviews, visitors, last_at, labels}`。`labels` 是看這一頁會被貼上的標籤。

### 標籤

管理員和成員都可以改。

| 方法 | 路徑 | 說明 |
|---|---|---|
| `GET` | `/labels` | 所有標籤，含 `rules` 和現在有幾個人（`people`）|
| `POST` | `/labels` | 至少要 `name` 和一條規則 |
| `PATCH` | `/labels/<lid>` | 只改有給的欄位。規則或門檻改了，會用現有的事件重算 |
| `DELETE` | `/labels/<lid>` | 有分群用到這個標籤時回 `409` |

| 欄位 | 說明 |
|---|---|
| `name` | 1 到 40 個字元，同一個網站裡不能重複 |
| `color` | `neutral`、`brand`、`green`、`amber`、`red`、`sky`、`violet`、`pink` |
| `description` | 說明，可以空著，最多 200 個字元 |
| `min_hits` | 符合幾次才算有這個標籤，1 到 1000，預設 1 |
| `rules` | `[{kind, value}]`，1 到 20 條，符合其中任何一條就算一次 |

規則的種類（英文不分大小寫）：

| `kind` | 什麼時候符合 |
|---|---|
| `path_prefix` | 看了這個路徑或它底下的頁面。`/blog` 會符合 `/blog` 和 `/blog/2026/hello`，不會符合 `/blogger` |
| `url_contains` | 看的頁面網址含有這段文字（含查詢參數，例如 `utm_source=newsletter`）|
| `title_contains` | 看的頁面標題含有這段文字 |
| `event` | 觸發了這個名稱的自訂事件，或點了 `data-cdp-click` 是這個值的元素 |

### 分群

管理員和成員都可以改。分群存的是條件，不是名單：每次查都是當下符合的人。

| 方法 | 路徑 | 說明 |
|---|---|---|
| `GET` | `/segments` | 所有分群，含現在符合的人數（`count`）|
| `POST` | `/segments` | `{name, description?, kind?, rules}`；名稱 1 到 60 個字元、不能重複，至少要有一個條件 |
| `POST` | `/segments/preview` | `{kind?, rules}`，不存，回 `{count, total, sample}`；`sample` 是前 5 個人 |
| `GET` | `/segments/<sid>` | |
| `PATCH` | `/segments/<sid>` | 只改有給的欄位 |
| `DELETE` | `/segments/<sid>` | |

`kind` 是 `all`（預設）、`member` 或 `anonymous`。`rules` 分三組，每一組最多 10 個條件：

```json
{
  "must": [{ "field": "label", "value": 3 }, { "field": "seen_within", "value": 30 }],
  "should": [{ "field": "referrer", "value": "google" }, { "field": "referrer", "value": "facebook" }],
  "must_not": [{ "field": "email", "value": "@example.com" }]
}
```

`must` 每一條都要符合，`should` 至少符合一條（空的就不看），`must_not` 一條都不能符合。

| `field` | `value` | 意思 |
|---|---|---|
| `label` | 標籤的編號 | 有這個標籤 |
| `referrer` | 文字 | 從網域含有這段文字的網站連進來過 |
| `email` | 文字 | email 含有這段文字 |
| `seen_within` | 天數 | 最近幾天內來過 |
| `first_seen_within` | 天數 | 最近幾天內第一次來 |
| `min_pageviews` | 次數 | 至少看過幾頁 |

要拿到分群裡的人，用 `GET /people?segment_id=<sid>` 或 `GET /people/export?segment_id=<sid>`。
