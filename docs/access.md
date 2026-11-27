# 連線與存取

既有密碼、管理員／成員與站台成員權限持續生效。GitHub、Google 支援既有帳號登入與明確綁定；管理員開放註冊後，也可從 `/register` 申請成員帳號。不因相同 email 合併帳號。停用帳號或移除站台權限，下一次 API／MCP 操作立即重讀目前權限。

## 啟用外部登入

設定 `ACCESS_PUBLIC_URL=https://your-domain.example`，且 `CDP_PUBLIC_URL` 若同時設定，必須同 origin。未設定時採固定 `http://127.0.0.1:8585`；不信任轉送標頭推導資源網址。瀏覽器開發代理使用前端網址作為 `ACCESS_PUBLIC_URL`。

設定 `ACCESS_GITHUB_CLIENT_ID`、`ACCESS_GITHUB_CLIENT_SECRET`，在 GitHub OAuth app 登記精確回呼 `https://your-domain.example/oauth/callback/github`。Google OAuth client 設定 `ACCESS_GOOGLE_CLIENT_ID`、`ACCESS_GOOGLE_CLIENT_SECRET`，登記 `https://your-domain.example/oauth/callback/google`。正式端點固定於程式；未設定供應商時登入與綁定按鈕會停用並說明原因。HTTPS canonical URL 的外部登入 cookie 帶 Secure。

## 一般 API 與 MCP

在「連線與存取」建立金鑰，選擇 API／MCP 通道與1–90天有效期。原值只顯示一次；關閉即從畫面記憶體清除，列表僅顯示前綴。金鑰不得管理帳號、綁定、其他金鑰或 OAuth client。

`reports:read` 只讀取站台與彙總報表；`people:read` 才能讀取個人事件、完整事件網址與屬性。建立金鑰預設不含個人事件，需另外勾選。 所有工具唯讀：`list_sites`、`get_site_overview`、`list_pages`、`list_segments`、`get_person_events`。站台投遞／追蹤金鑰、通知設定與成員名單均不輸出。CDP 報表另移除已識別訪客、完整頁面網址及分群條件。

一般 API：`POST /api/v1/{operation}`，JSON 參數及 `Authorization: Bearer …`；`GET /api/v1/openapi.json` 提供實際登錄的輸入 schema。MCP：`/mcp`，官方 rmcp3.12.0，僅支援 `2026-07-28` Streamable HTTP 每請求協定。官方 client 必須使用 `ClientLifecycleMode::Discover`；不提供舊 initialize／session 流程。接受測試見 `src/api/tests/access.rs` 與 `src/api/examples/access_probe.rs`。

公開動態註冊預設關閉。部署者可選擇 `ACCESS_DYNAMIC_CLIENT_REGISTRATION=true`；公開待用註冊24小時後回收，與管理員註冊配額分開。

OAuth 公開 client 由管理員在原生頁面登記精確 redirect URI，使用 Authorization Code + S256 PKCE。資源只能精確為 canonical URL 加 `/mcp` 或 `/api/v1`，每份授權只能一個。原生同意表單只送 approve／deny；scope、client、resource 由伺服器保存。access token 最長1小時，且不超過原授權30天期限；refresh token 旋轉，重用會撤銷整個家族。金鑰、授權、綁定都可在原生頁面撤銷。

## 稽核語義

一般 API 操作與 MCP tools/call 各算一次；探索、列工具、原有公開收集與通知不計入。先以獨立交易保留 pending 事件／attempt，再執行既有處理器，最後以另一交易更新 outcome 與 UTC 分日統計。保留失敗拒絕執行；完成記錄失敗回503並保留 pending。超過120秒未完成的事件以有界批次恢復為 interrupted。詳細事件90天，彙總可選7／30／90天；pending 與失敗分開顯示。

成員只查本人；管理員可篩所有帳號或指定正整數帳號ID。憑證與綁定管理始終是本人。事件僅含操作名稱、身分參照、狀態、耗時、request ID；不存參數、內容、原始 token、授權碼或 provider profile。SDK payload logging 在 subscriber layer 硬性抑制；舊 HTTP tracer 只記 method 與 MatchedPath 路由樣板。

## 本機流程證據

[登入與未綁定說明](images/access-01-login.png) · [正式映像未設定供應商](images/access-login-unconfigured.png) · [已綁定與憑證登記](images/access-02-credentials.png) · [原生同意授權](images/access-consent.png) · [真實用量與撤銷](images/access-03-usage.png) · [實際操作錄影](images/access-workflow.webm)。[390px 淺色](images/access-mobile-light.png)／[深色](images/access-mobile-dark.png)。機器核對結果見 [access-qa.json](access-qa.json)。

`docs/capture-access.py --url http://127.0.0.1:PORT --mock-providers` 只允許隔離 loopback fixture；密碼從 `ACCESS_SCREENSHOT_PASSWORD` 讀取，官方 SDK probe 位置從 `ACCESS_CAPTURE_PROBE` 讀取，不印出祕密。mock 建置需 `--features access-kit/test-support` 與 `ACCESS_TEST_MOCK_PROVIDER_URL`，正式 Docker 不開此 feature。固定 mock 可簽署 Google JWT，僅證明本機流程；未宣稱使用正式 GitHub／Google credentials 上線驗證。

## 快照

不可變核心：`crates/access-kit`；來源 SHA256 `950710417855acd38c245c50c1facb6e456b45df9c6be2c6fb804f1f7a47082a`，schema SHA256 `5a6dfc0ee83c70fce2cb884cbd1541f5ab8e49487cea76ae009a86f10fc26157`。`src/api/migrations/0002_access.sql` 加 `0003_access_client_registration.sql` 與核心 schema byte-identical。以 `python crates/access-kit/scripts/snapshot.py --check` 核對。

## 使用者註冊與管理員審核

一般使用者可透過 `/register` 使用帳號密碼或已設定的 Google／GitHub 申請帳號。管理員可選擇關閉註冊（預設）、註冊後待審核、直接開放；待審核及拒絕帳號沒有應用、API 或 MCP 存取權。第一位管理員仍由 `/first-login` 建立，管理員直接新增成員不受公開註冊政策限制。

註冊政策、審核結果與原因保存在管理後台；只有第三方登入的帳號須保留至少一種登入方式。這裡的使用者註冊與 `ACCESS_DYNAMIC_CLIENT_REGISTRATION` 控制的 OAuth 應用程式註冊是兩個獨立設定。新版採附加資料庫遷移，保留既有帳號的啟用狀態；本輪驗證見 [registration-verification.md](registration-verification.md)。
