# API

對外 `POST /api/v1/{operation}`、`/mcp` 與 OAuth 的 scopes、Bearer 和真實用量語義見 [存取指南](access.md)。下列既有管理 API 使用原 HttpOnly cookie；Bearer 不可用來管理帳號或綁定。

瀏覽器安裝、同意生命週期、跨分頁身分順序、簽署範例與實際操作圖見 [追蹤與身分指南](tracking.md)。本頁列出目前 HTTP 欄位；舊版省略身分順序資訊的呼叫仍相容，但不具備新版的完整重試保護。

網址分工：

- `/collect`、`/t.js` 和 `/tracker-config/{key}`：公開 collector、追蹤程式與採集設定，不用管理台登入。
- `/api/...`：管理介面用的 API，要先登入（HttpOnly cookie `cdp_session`）。

應用程式產生的錯誤是 `{"error": "給人看的訊息"}` 加上對應的 HTTP 狀態碼。管理 API 的 JSON 請求若無法通過 Axum 解析，會回英文純文字：JSON 語法錯誤為 `400`、欄位或型別不符為 `422`、缺少 JSON Content-Type 為 `415`。除另有標示外，時間都是 UTC 的 RFC 3339（`2026-01-05T08:30:00Z`）。

## 收資料

### `GET /t.js`

追蹤程式碼。用法見 [README](../README.md#追蹤程式碼)。

### `GET /tracker-config/{key}`

回傳 `{version: 2, site_id, namespace, active, policy}`，其中 `namespace` 是 `site_<id>`、`policy` 是下方的 `collection_policy` 物件。回應帶 `Cache-Control: no-store` 與 `Access-Control-Allow-Origin: *`；不含身分簽署密鑰。找不到追蹤金鑰回 `404`。追蹤器先讀設定，再依同意與 DNT 決定是否建立 ID、採集事件；設定失敗時停止採集。

### `POST /collect`

追蹤程式碼會自己送，一般不用直接呼叫；要從伺服器或 App 送資料時才需要。

內容是 JSON，最多 64 KB。不看 `Content-Type`（追蹤程式碼用 `text/plain` 送，才不會觸發 CORS 預檢）。

```json
{
  "key": "追蹤金鑰",
  "anon_id": "browser-7f3a9c1e",
  "session_id": "session-4d2b6f8a",
  "consent": true,
  "events": [
    {
      "type": "pageview",
      "event_id": "event-page-7f3a9c1e",
      "url": "https://shop.example.com/products/coffee-beans",
      "title": "咖啡豆",
      "referrer": "https://www.example.org/"
    },
    { "type": "custom", "event_id": "event-cart-8b4d2e6f", "name": "add_to_cart", "url": "https://shop.example.com/cart", "props": { "sku": "A-1001" } }
  ],
  "identify": { "user_id": "member-42", "mutation_id": "identity-9c5e3f7a", "revision": 1042, "email": "amy@example.com", "name": "Amy", "account": "amy01", "traits": { "plan": "pro" } }
}
```

以上 ID 與 revision 是格式示意；實際 `anon_id`、`session_id` 要符合下表字元限制。每個新事件使用新的 `event_id`；每次明確登入主張使用新的 mutation ID 與協調後遞增的 revision，只有重試沿用原值。`consent: true` 必須來自網站實際的同意決策。啟用簽署政策時還須提供 `identify.proof`。

| 欄位 | 說明 |
|---|---|
| `key` | 網站的追蹤金鑰，必填 |
| `anon_id` | 必填。同一個瀏覽器（或裝置）要一直用同一個，自己產生一個隨機字串就好：8 到 64 個字元，只能用英數字、`-` 和 `_` |
| `session_id` | 可以不給。同一次造訪的事件用同一個，時間軸會照它分段。最多 64 個字元，可以用的字元同上 |
| `consent` | 網站要求同意時必須為 `true`，否則回 `403`；這是客戶端主張，不是同意稽核紀錄 |
| `events` | 一次最多 50 個，可以是空的（只送 `identify`）|
| `events[].event_id` | 建議提供：8–64 個 ASCII 英數字、`-`、`_`；同站唯一，重試沿用。省略時保留舊版行為，無法去重 |
| `events[].happened_at` | 選填 RFC 3339 時間，允許過去 7 天至未來 5 分鐘；省略時以接收時間為事件時間。伺服器另存 `received_at` |
| `events[].type` | `pageview`、`click`、`scroll`、`custom` |
| `events[].name` | `custom` 是事件名稱；`click` 是按鈕或連結的文字（或 `data-cdp-click` 的值）|
| `events[].url` | HTTP(S) 頁面網址，清理後最多 2000 bytes；無網址的伺服器 custom event 可省略 |
| `events[].title`、`events[].referrer` | 頁面標題、從哪裡連進來 |
| `events[].props` | 任意的 JSON 物件，最多 2 KB |
| `identify.user_id` | 網站內穩定、區分大小寫的會員 ID；去除頭尾空白，最多 128 bytes，不含控制字元 |
| `identify.email` | 選填聯絡 email，轉小寫。未提供 `user_id` 時才使用允許的舊版 email-only 路徑 |
| `identify.name`、`identify.account` | 選填基本欄位，各最多保留 80 字元 |
| `identify.traits` | 自訂平面屬性，最多保留 20 個字串／數字／布林值，另受站台 `allowed_traits` 白名單限制 |
| `identify.mutation_id`、`identify.revision` | 必須成對提供或一起省略。ID 為 8–64 個 ASCII 英數字、`-`、`_`；revision 為 1–9,007,199,254,740,991 的整數，依同站／同匿名裝置協調順序 |
| `identify.proof` | 選填 `{expires_at, signature}`；到期時間為 Unix 秒，須未過期且最多在一小時後。HMAC-SHA256 只驗證 user_id，格式見 [簽署會員 ID](tracking.md#signed-member-ids) |
| `identity_context` | 取得正式 collector 的身分確認後，後續事件可帶 `{mutation_id, revision}`；不能與 `identify` 同時提供，且必須符合該裝置目前已接受的身分版本 |
| `test_token` | 選填安裝測試碼；通過政策篩選與格式檢查後，有效碼只寫測試回執，錯站或過期碼回 `403`。不建立人物、裝置、事件或標籤；全部內容被政策排除的例外見下方 |

同站相同 `user_id` 才會明確連接不同瀏覽器；匿名人物的歷史會接到該會員。已辨識人物切換帳號只改變往後事件的歸屬，不搬走前一位會員的歷史。不同 `user_id` 不會因 email 相同而合併。舊版 email-only 匹配只在 email-only 人物間進行，並可用 `legacy_email=false` 停用；升級規則見 [身分演算法](tracking.md#the-identity-algorithm)。

正常處理回 `204`、沒有 body，但不代表新增了資料：重複事件、舊身分封包、爬蟲或政策排除也可能回 `204`。涉及身分時可讀 `X-CDP-Identity`：`applied` 已套用、`replayed` 已處理過、`current` context 符合目前版本、`stale` 版本過舊／衝突；此 header 已開放跨來源讀取。`stale` 封包內事件也會丟棄，避免歸給後來登入的帳號。

若政策移除封包中的全部事件與身分主張，例如只送已停用的 click／scroll 且沒有 identify，collector 會提前回 `204`。這時不驗證 `test_token`、不寫測試回執，也不寫 analytics；不能以 `204` 判定測試碼有效或安裝測試已完成。

事件以同站 `event_id` 去重，效期僅到該事件資料列被刪除；不保證永久或 exactly-once 投遞。身分 mutation 的高水位獨立保留，舊版本與已啟用順序後的無 metadata 主張不能重新綁定。重試要保留原 mutation 與事件 ID，不得將舊資料改用新 revision 重送。缺少 mutation 與客戶端時間的舊版 identity-only 呼叫仍按抵達順序處理。多個客戶端共用 `anon_id` 時須協調同一個順序；完整規則見 [重播與順序](tracking.md#identity-replay-and-ordering)。

正式 collector 確認身分後，`identity_context` 可免除重送 profile 與過期的 proof；它本身不能建立新身分。安裝測試回執不算正式確認。新簽署主張仍須有效 proof，不能用 context 取代首次簽署。

| 狀態碼 | 什麼時候 |
|---|---|
| `400` | JSON／ID／時間／mutation-context 格式不符、沒有事件或 identify、不認得的事件種類、一次超過 50 個事件 |
| `403` | 來源不允許、網站停收、缺少要求的同意、尊重的 DNT=1、身分簽章缺少／無效；通過政策篩選與格式檢查後的測試碼錯站／過期 |
| `404` | 沒有這個追蹤金鑰（換過金鑰之後，舊的也是 404）|
| `413` | 內容超過 64 KB |
| `429` | 同一個 IP 送得太頻繁（`CDP_RATE_LIMIT`，預設一分鐘 600 次）|

collector 一般回應帶 `Access-Control-Allow-Origin: *`；是否收取仍檢查站台政策。有 `Origin` 時依允許清單判定；無 `Origin` 的伺服器請求不受這項來源限制，因此追蹤金鑰不是身分驗證憑證。要求同意的網站仍需 `consent: true`；`respect_dnt` 啟用且 HTTP `DNT: 1` 時拒收。URL credentials/hash 一律移除，query、referrer、click/scroll 與 traits 依目前政策清理；custom props 內任意文字不會遞迴遮罩。正式 analytics 不收已辨識爬蟲。IP 只用來限制流量，不存入 analytics。

```bash
curl -i https://cdp.example.com/collect -d '{
  "key": "<追蹤金鑰>",
  "anon_id": "server-7f3a9c1e",
  "consent": true,
  "events": [{ "type": "custom", "event_id": "order-event-8b4d2e6f", "name": "order_paid", "url": "https://shop.example.com/checkout/done" }],
  "identify": { "user_id": "member-42", "mutation_id": "identity-9c5e3f7a", "revision": 1042 }
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
| `GET` | `/api/sites/<id>/identity-secret` | ✓ | 回 `{secret, algorithm, message, encoding}`，帶 `no-store`；密鑰只留在受信任的應用伺服器 |
| `POST` | `/api/sites/<id>/installation-tests` | ✓ | 建立 15 分鐘測試碼，回 `{token, expires_at}` |
| `GET` | `/api/sites/<id>/installation-tests/<token>` | | 有該站閱讀權即可讀 `{expires_at, expired, received_at, receipt}`；尚未收到時 receipt 為 null |

可以改的欄位：

| 欄位 | 說明 |
|---|---|
| `name` | 1 到 60 個字元，不能重複 |
| `url` | 網站的網址，可以空著 |
| `allowed_origins` | 只收這些來源送來的資料，例如 `["https://shop.example.com"]`；空的就不限制。最多 20 個 |
| `active` | `false` 就停收，已經收到的都還在 |
| `member_ids` | 看得到這個網站的成員 |
| `collection_policy` | 採集與身分政策，完整欄位如下；一般站台回應包含政策，但不包含簽署密鑰 |

`POST` 省略政策會使用舊版預設；`PATCH` 完全省略政策會保留原設定。若傳入 `collection_policy`，會取代整個政策物件，內部省略欄位使用下表預設，**不是逐欄合併**。建議先讀取完整政策，再修改與送回。

| 政策欄位 | 舊版預設／允許值 |
|---|---|
| `consent_required`、`respect_dnt` | 都是 `false` |
| `auto_page`、`auto_click`、`auto_scroll` | 都是 `true`；auto_page 控制 SDK 自動 pageview，仍可手動送 pageview；collector 也會排除已停用的 click/scroll |
| `spa_mode` | `history`；可為 `history`、`hash`、`manual` |
| `query_mode` | `keep`；可為 `keep`、`strip`、`allowlist` |
| `query_allowlist` | `[]`，allowlist 模式保留的 query key |
| `referrer_scope` | `full`；可為 `full`、`origin`、`none` |
| `click_text` | `true`；停用後 collector 將 click 名稱統一為 `click` |
| `ignore_selectors` | `[]`，SDK 自動點擊排除用 CSS selectors；不遮罩其他事件或 props |
| `allowed_traits` | `null` 代表舊版全部支援欄位；`[]` 代表不收；陣列只保留列出的 key |
| `legacy_email` | `true`，允許 email-only identify；停用後不處理沒有 user_id 的主張 |
| `require_signed_identity` | `false`；啟用後 identify 必須帶有效 user_id proof，匿名事件仍可送入 |

三種清單各最多 30 項，每項非空白、最多 120 bytes、不含控制字元；清單限制或不支援的模式回 `400` JSON 錯誤，未知政策欄位在 JSON 解析階段回 `422` 英文純文字。UI「最少採集設定」需儲存後生效。SDK 在下次文件載入時取得新設定，collector 立即依目前政策檢查。

安裝回執包含清理後的 events、origin、identity 的 user_id/email/verified、consent_checked、bot 與 `analytics_written: false`。讀到過期回執不代表還可送入；到期後不接受新回執，建立新測試時清除已過期資料。全部內容被政策排除的封包回 `204` 且不產生回執。測試模式的使用與排錯見 [實際安裝回執](tracking.md#real-installation-receipt)。

以下的路徑都在 `/api/sites/<id>` 底下。

### 總覽

`GET /overview?tz_offset=480` 回總覽頁用的數字。`tz_offset` 是瀏覽器的時區比 UTC 快幾分鐘（台北是 `480`），每天的數字照這個時區切。

| 欄位 | 說明 |
|---|---|
| `engagement` | 三筆 `{activity, member, anonymous}`，依 `recent`、`cooling`、`dormant` 排列，包含零值；目前有 `user_id` 或 email 即為 member，再按 `last_seen` 分組 |
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
| `kind` | `member`（有 user_id 或 email）、`anonymous`（兩者都沒有）；沒給是全部 |
| `activity` | `recent`：最後造訪距今 ≤ 7 天；`cooling`：> 7 天且 ≤ 30 天；`dormant`：> 30 天。三組互斥，以 UTC 連續時間計算，清單與 CSV 皆支援 |
| `q` | user_id、email、名稱或帳號含有這段文字 |
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
