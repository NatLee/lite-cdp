# CDP 授權足跡與用量稽核設計

使用者已授權 GitHub＋Google 原帳號綁定／登入與 MCP/API OAuth。這是完整存取階段，保留前一輪主題動態與全部原有資料權限。共用協定由不可分叉的 crates/access-kit 快照提供，服務自己的 adapter 與前端獨立。

## 本庫介面與資料最小化

list_sites/get_site_overview/list_pages/list_segments→reports:read; get_person_events→people:read. Overview excludes recent_identified; site list excludes collection key/members; segments exclude rules/creator metadata. Public collect/tracker stay unchanged.

MCP/API 是領域唯讀操作，不提供通知發送、帳號管理、匯入或金鑰輪替工具。每個操作重讀 active user（disabled=0）並重用原 site membership。不能讓 reports:read 附帶訪客身分或管理秘密。

連線頁用本庫字型／主色／4px spacing、400/600字重與 Lucide16/20/24px，分帳號綁定、金鑰、應用授權、真實用量四區。API金鑰只出現一次，不進 localStorage。Cookie本人管理；管理員另可註冊 client、看全站稽核。Consent 用原生表單同意／拒絕已驗證的 immutable request；安全 next 僅回 /oauth/authorize。正式 provider credentials 未設時解釋停用；mock 需明示本機示範。

## 共用合約

# OAuth／MCP／API 共用合約（2026-11-27）

使用者已回覆：同時支援第三方登入／綁定與MCP/API OAuth授權；登入供應商GitHub＋Google。沿用既有直接實作及PR/merge/刪branch授權。本文件是實作者選擇的具體合約，不把未問過的細節歸為使用者指定。

## 取捨與範圍

採各repo獨立部署／持久化，保留既有Rust/Axum/SQLite（六庫）或Flask（情緒）。ES保留無HTTP相依的library，新增可選server。比較過中央登入服務（改部署過大）、八份獨立手刻協定（難以一致驗證）、共用Rust模組＋各host adapter；採第三項。共用Rust access-kit先在/tmp/access-kit-authoring開發與測試，再以完全相同來源快照vendoring到七個Rust服務，各庫可獨立build，不建立使用者未要求的新GitHub repo。Flask使用相同HTTP與稽核合約，採適合Python的受維護OAuth/OIDC/MCP套件，不用跨語言子程序代理。

第一版MCP／一般API先提供有實際用途的領域讀取、查詢／編譯／分析；既有匯入API、表單／tracker及寫入功能照原規則。任何新增領域寫入需獨立scope並保留原角色；不把帳號管理、秘密金鑰、通知發送列成MCP工具。至少每repo提供以下核心工具；可增加同等權限讀取項目。

| Repo | 工具 | 特別授權 |
|---|---|---|
| ES | compile_query, inspect_query, extract_terms | 新server管理員與API帳號；library不變 |
| Vault | search_keywords, get_keyword, inspect_expression, compare_expressions | 原owner/public/group可見性；public仍是登入使用者可見 |
| Inbox | list_sites, search_feedback, get_feedback, get_feedback_stats | 每次site membership，admin原有全站權限 |
| CDP | list_sites, get_site_overview, list_pages, list_segments, get_person_events | people/events必須people:read；其餘reports:read與site membership |
| Sinyi | news_summary, news_list, volume_trend, trust_summary, reputation_summary | 原member讀取權限；import token仍只匯入 |
| Hotel | get_my_hotels, hotel_overview, hotel_evaluation, compare_hotels, search_hotel_posts | 每次viewable_hotel：主要飯店／競品／admin |
| Sentiment | analyze_sentiment | conversion原文/t2s/tw2s；原payload限制；無模型回503，不能示例冒充 |
| KEYPO | list_topics, volume_trend, sentiment_distribution, search_posts, hot_keywords | 原topic owner/quota/filter/exclusion/sentiment語意 |

## 身分與登入政策

- 保留本機users.id、密碼登入、首次管理員設定及現有admin/member。只允許已建立的帳號主動綁定第三方；不新增開放自註冊，不憑同email合併或自動升admin。
- 已綁定subject可用GitHub/Google登入。未綁定者收到清楚說明「先以原帳號登入並綁定」。管理員仍可依原功能建立member。
- (provider,subject)全站唯一；bind/unbind必須目前登入本人。解除第三方不影響原password登入；不能由URL/user_id替別人綁定。
- GitHub固定官方OAuth/token/user endpoints，以token取得穩定numeric id作subject；Google透過官方OIDC discovery/JWKS與維護的OIDC驗證器核對簽章、iss、aud、exp、nonce。
- state、PKCE、nonce單次且綁browser/session與用途(login/link)，5分鐘過期；所有code exchange在server。不要保留provider access_token／refresh_token，因本需求僅用於身分確認。
- Client ID／Secret由env提供：ACCESS_GITHUB_CLIENT_ID/SECRET、ACCESS_GOOGLE_CLIENT_ID/SECRET、ACCESS_PUBLIC_URL。Public URL必須是預設loopback開發origin或明確配置的https origin，禁止由不可信Host任意組callback。
- Callback URI固定{origin}/oauth/callback/github與/google。未設定provider時按鈕停用並解釋設定；不建立假已連線紀錄。正式provider回呼需人工註冊client後實測，測試以可控local mockprovider驗證完整合約，不能用mock成功冒稱正式成功。

## HTTP合約（共同外觀，不干擾現有/api）

- GET /api/access/providers：公開；兩個provider的name/enabled/login_start_url。
- POST /api/access/identities/{provider}/link：cookie本人；建立綁定流程並回{authorization_url}。
- GET /oauth/login/{provider}：第三方登入起點。
- GET /oauth/callback/{provider}：驗state/PKCE/nonce/subject，建立或確認綁定，再發host原session cookie或回設定頁。
- GET /api/access/overview：cookie本人；provider bindings、credentials、grants與最近7天summary。
- GET /api/access/identities；DELETE /api/access/identities/{id}：本人；admin不代替他人link。
- GET/POST /api/access/credentials：本人；建立{name,scopes,expires_in_days}（1..90，預設30），回原值一次。DELETE /api/access/credentials/{id}撤銷，owner或admin限定。
- GET /api/access/grants；DELETE /api/access/grants/{id}：本人或admin；即時撤銷該grant全部access/refresh tokens。
- GET/POST /api/access/clients：admin管理預註冊OAuthclients。公開DCR另列；不得讓一般APIkey取得管理權限。
- GET /api/access/usage?days=7|30|90&transport=api|mcp：本人；admin可選user_id或all。回時間範圍、UTC分日attempt/success/failure與總數，公開收集不混入。
- GET /api/access/events?cursor=...&limit=...&transport=...&outcome=...：同上，最多100每頁。
- POST /api/v1/{operation}：一般bearer API，JSON參數與MCP同schema。固定operation registry，不能任意代理URL或後台路由。
- GET /api/v1/openapi.json：公開schema與操作說明，不含私有資料。
- /mcp：官方SDK的Streamable HTTP，bearer必須在進入SDK前驗證；不宣稱只處理自製tools/call就是完整MCP。

頁面：「連線與存取」包含帳號綁定／API金鑰／應用授權／用量稽核四區；admin另有client註冊。各repo沿用自身主題，不能共用玻璃卡片模板。Login頁新增已配置provider按鈕；OAuth授權頁顯示應用名、requested scopes、目前帳號與同意／拒絕。APIkey原值僅建立後顯示且不進localStorage；有複製及關閉。用量取真正server資料，無資料顯示0與範圍，不假造圖。

## OAuth授權伺服器

- Authorization Code + PKCE S256，public clients；refresh_token旋轉與reuse detection；不支援implicit/password grant。
- /.well-known/oauth-protected-resource/mcp（及標準根discovery映射）廣告canonical resource{origin}/mcp與authorization_servers；API resource為{origin}/api/v1。/.well-known/oauth-authorization-server回issuer/auth/token/revoke/register/scopes/PKCEmetadata。
- GET /oauth/authorize：核對client_id、exact redirect_uri、response_type=code、state、scope、resource、code_challenge S256。未登入導向自身login，next只允許本機authorize路徑；無效client/redirect不能向未驗證URI回錯誤。
- 由server建立短效pending request ID；consent GET顯示immutable requested scope與client，POST同源cookie同意／拒絕後才redirect。不能從POST任意改user/client/redirect/scopes。
- POST /oauth/token（form-urlencoded）：驗code單次/5min期限/client_id/redirect_uri/resource/verifier，access token1h；refresh30d，旋轉時原token標記used，reuse撤銷family。response Cache-Control:no-store。
- POST /oauth/revoke：RFC7009，以token對應grant撤銷；不能讓一個client撤銷別client token。
- POST /oauth/register：受rate/body/數量限制的public client registration，名稱為不可信文字；redirect僅https或loopback http，不接受fragment/userinfo/wildcard。支持預註冊，DCR屬相容性選項；不抓任意client metadata URL，避免引入SSRF。
- token為隨機256bit opaque值；資料庫只存hash，scope、resource/audience、expiry、revocation、user/client/grant綁定全部server驗證。token不可在URL/query接收。
- bearer驗證每次重新核對user.active與目前角色/資源權限。cookie管理端點不接受API token，禁止scope升級。

## host adapter與共用模組邊界

共用Rust模組處理上述protocol、provider、credential/grant、usage與audit schema，不認識私有domain資料。Host提供：
1. CurrentPrincipal{id,username,display_name,role}之cookie解析與active-user查詢。
2. 建立host原session及cookie；不能讓core另造第二套不互通帳號。
3. 固定DomainOperation{name,description,input_schema,required_scope,read_only} registry。
4. execute(principal,operation,args)執行既有領域handler/services、回JSON或受控status。可用進程內Router oneshot重用原handler，但只允許registry映射，不允許外部給URL；不經網路、不另計一次API用量。TrustedPrincipal只由server extension注入，原extractor再次讀active user；header不能偽造此extension。
5. SQLx pool供core新增access_前綴tables；不要改0001。Host numbered migration呼叫／包含相同schema。

ES新增server自己的users/password/session bootstrap，可重用同host primitive。Flask新增SQLite access store與最小admin/bootstrap/password，原離線瀏覽器工作台仍可開啟；server推論在設定管理員後改為已登入或bearer可用，未setup只能本機setup，不能永遠保留繞過auth的公開推論路徑。README清楚說明新服務存取流程。

## 稽核口徑與資源邊界

- 一次API operation或MCP tools/call算一次attempt；回應分類success/failure，保留request_id、UTC時間、transport、operation、user/client/credential非秘密ID、status、duration_ms。MCP initialize/list-tools等是協定活動，不算工具執行。
- domain reuse不重複計數；API與MCP各有獨立通道。公開Inbox表單與CDPcollector維持原行為與原統計，不把「接受但略過的bot」誤稱新增事件。
- 禁止記錄token、cookie、code、完整query/payload、回饋內文、訪客屬性、推論文本。event操作名來自registry，未知值只記unknown。
- 90天詳細紀錄；durable每日aggregate供7/30/90天圖表；頁面明示時間範圍，不宣稱永久all-time。記錄與aggregate更新需交易一致。資料庫記錄失敗不能靜默假裝可稽核，回明確503且不要執行尚未開始的操作。
- 有期限、撤銷、無scope、wrongaudience、disableduser、失去site/hotel/topic權限都必須拒絕。invalidtoken事件只供admin看且不得把原token寫log。
- Body/參數長度限制、credential/user速率限制、clientregistry與pendingflow有界且清理過期資料。外部issuer/token/JWKS請求設定timeout，不追隨任意redirect。

## 必要測試與驗收

core：PKCE wrongverifier/reuse/expiry，redirect exactmatch，scope escalation，wrongresource，state replay/browser mismatch，Google badiss/aud/nonce/signature，identity conflict/noemailmerge，refreshrotation/reusefamily revoke，revocation immediate，disabledaccount，audit counts1only，own/admin visibility，expiry/migrationsrestart。
host：每repo至少一條成功API與同工具MCP實際SDKclient呼叫，一條原權限拒絕；CDP跨site/people scope、Hotel跨hotel、Vaultprivate/group、KEYPO別人topic等必測。保持舊tests與Docker可建置。
UI：真實建立key→呼叫API/MCP→用量增加→撤銷後拒絕；provider未設定／已綁定／解除、consent同意拒絕等可控環境測試。新功能截圖需由真實localhost mockclient/provider產生並明示示範，不能截HTML造假。
正式GitHub/Google client註冊、正式回呼URL、品牌色與alt意義列在需人工確認項目；未有正式credentials時仍完整實作＋mock驗證，不宣稱正式連線已完成。

## 來源與版本

已查MCP latest於2026-11-27導向2026-07-28版本，含resource metadata／authorization server discovery；官方Rust SDK registry查得rmcp3.12.0（需cargo info確認MSRV／API並固定相容版本）。openidconnect4.1.0；oauth2版本由cargo確認。SDK必須選含已修schema-cache問題的版本並在transport前驗token，不能以舊手寫session協定冒稱最新相容。
- https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization
- https://modelcontextprotocol.io/specification/2026-07-28/basic/transports
- https://docs.rs/rmcp/latest/rmcp/
- https://developers.google.com/identity/openid-connect/openid-connect
- https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps
- https://www.rfc-editor.org/rfc/rfc9700

## Builder review decisions — 2026-11-27

These are implementer decisions within the user's direct implementation authorization.
- Required MCP version is 2026-07-28 only. Use official rmcp 3.12.0 and Python mcp 2.3.0; per-request metadata, no protocol sessions. Reject unsupported legacy versions explicitly rather than advertise them. Discovery/list do not increment tool counts; remove initialize from required acceptance.
- Personal API credentials store explicit allowed audiences, a non-empty subset of `api` and `mcp`, default both. Creation UI displays them; every bearer check enforces them. OAuth grants/access/refresh bind a single exact canonical resource and cannot expand through refresh.
- Audit starts with a durable pending attempt in the same transaction as daily attempt increment, then completes by updating outcome and success/failure aggregate atomically. Record stale pending as interrupted/failure after bounded timeout with a conditional update (never race active completion); display pending count separately. Failure to reserve an audit record denies execution. If finalization fails, return503 and retain pending for recovery; never claim an unrecorded success.
- Sentiment public web runs Python3.12 ASGI: Flask mounted with a WSGI adapter plus official MCP SDK, one externally exposed port. Legacy Paddle runtime remains in a separate internal worker on the existing GPU image; modern web uses a fixed configured worker URL, bounded JSON and timeout, with a deployment secret header. The worker accepts only inference, requires its secret, and has no host-published port. Remove compose publication of9292 too. Synthetic/injected predictor tests exercise contracts; no claimed GPU execution. Offline browser workbench remains public; all real inference endpoints require an authenticated user/bearer, including prior endpoint aliases.
