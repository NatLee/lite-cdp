# 存取整合驗證

2026-11-27。隔離 demo 資料，未使用正式 client credentials、未發送通知、未接觸正式資料。先前純前端證據另見 [主題動態驗證](themed-motion-verification.md)。

## 自動驗證

- Rust 完整套件 53 個測試：單元 29、存取 6、既有 HTTP 18。新存取測試先 RED404，再 GREEN；最終 schema byte-equality 測試再通過。
- 真正官方 rmcp3.12.0 client 透過 ephemeral HTTP listener，以 Discover2026-07-28 探索／列工具（用量0）、呼叫工具（用量1）、撤銷（下次拒絕）。MCP-only key 拒絕一般 API。沒有手寫 initialize substitute。
- API key→原既有 handler→站台可見性→一次用量→撤銷；跨站／移除成員權限、停用帳號、偽造 TrustedPrincipal header 皆拒絕。管理員 sites 投影移除金鑰／成員／通知欄位。CDP 另驗 reports scope 不能呼叫個人事件，people scope 可呼叫，但不同站台仍404，預設 overview 不含已識別訪客。
- React/Vitest 36 個測試，production build 通過。包含未設定供應商、一次性原值／焦點回復、明確 audiences、撤銷刷新、真實數字、原生同意表單、safe next、分頁與管理員查看指定帳號。CDP 額外核對 people:read 預設未勾選、明確勾選才送出。
- `cargo fmt --check`、`cargo clippy --all-targets --all-features -- -D warnings` 通過。SDK TRACE payload canary regression 通過；公用舊 route tracer 只記 method／MatchedPath 路由樣板，內部 dispatcher 不含 TraceLayer。
- source style audit：4px 固定 spacing、字重400／600、Lucide16／20／24 stroke1.5、無 glass／glow／heavy shadow。新 access 頁的實際 computed styles、1440／390與light／dark、reduced-motion／overflow／pageerror 由 capture script 核對。

## 實際瀏覽器

`docs/capture-access.py` 使用真實 UI 與持久化資料：未綁定登入拒絕→原帳號密碼登入→GitHub／有簽章 Google mock 綁定→外部登入仍是原 user ID 與 host cookie→建立 key→關閉原值→真實 API／官方 SDK 呼叫→原生 deny/approve→PKCE exchange→OAuth token 真實 domain call→撤銷授權／key→下一次401→解除綁定後原密碼仍可用。

第二版採管理讀取240／分鐘、寫入60／分鐘的分開配額；最終完整多流程直接使用正式限制，不加入人工延遲。腳本仍保留可選的舊版配額等待開關；錄影不含密碼或金鑰原值。詳細結果見 [access-qa.json](access-qa.json)。mock 只證明本機協定與驗證；正式 GitHub／Google client 註冊與真實網域回呼仍由部署者設定。

[實際錄影](images/access-workflow.webm)；[登入](images/access-01-login.png)、[綁定與憑證](images/access-02-credentials.png)、[原生同意](images/access-consent.png)、[使用紀錄](images/access-03-usage.png)。所有影像皆由實際應用擷取；無 raw token/code/profile JSON。

## Docker

正式映像正常 feature 建置，不開 mock endpoints。locked release build 及新 core COPY 邊界通過；container smoke 覆蓋 health、首次建立管理員／HttpOnly cookie、原生SPA、GitHub／Google disabled 與 DCR-off 預設；建立正式映像內的 key→API list_sites＋官方 SDK list_sites→精確2次成功→撤銷後401且用量不增加，並核對 canonical resource metadata challenge。測試 container 已停止移除。

## 來源與復原

核心 source SHA256 `950710417855acd38c245c50c1facb6e456b45df9c6be2c6fb804f1f7a47082a`；schema SHA256 `5a6dfc0ee83c70fce2cb884cbd1541f5ab8e49487cea76ae009a86f10fc26157`。`python crates/access-kit/scripts/snapshot.py --check` 與 host0002＋0003 concat byte equality 通過。核心 OAuth／provider／audit 的完整負面矩陣見 vendored `crates/access-kit/VERIFICATION.md`。

重現時重新建立隔離 demo、啟動 feature-only loopback mock，將本機網址、私密密碼與官方 SDK probe 位置透過環境參數交給 `docs/capture-access.py`。完成後停止當次程序並清理合成資料。流程會撤銷測試 key／grant，解除 GitHub 綁定後核對原密碼仍可登入；先前的來信流程精確還原狀態、updated_at 與 notes。

## 獨立審查修正

- 真實 Router 的 log-capture 測試先 RED、後 GREEN：`/api/sites/{id}` 只記 MatchedPath 樣板，實際站台ID與 query 不進入紀錄，canary 金鑰／query／ID 不出現在 TRACE 輸出。
- 篩選更新先清除分頁狀態；後續頁綁定已套用 query／generation／cursor，過期回應不覆蓋新結果，同ID只顯示一次。三個 deferred-response 回歸與真實瀏覽器攔截測試通過；獨立審查者另外用原 Chrome 腳本重跑兩專案通過。
- README 使用127.0.0.1；Vite 固定5173且 strictPort，canonical URL 與瀏覽器同 origin。
- v2 公開動態註冊預設關閉：metadata 不含 registration_endpoint，直接註冊請求回400 registration_disabled；管理員原生註冊與 OAuth 全流程正常。

本次本機驗證映像識別：`sha256:3b98ed74d8abebb536eed0476746907ca77887398441e03611f3158bc2b7d7fa`。

獨立驗證另以最終正式映像重跑原始 `docs/smoke-test.sh` 完整領域流程，並以官方 Python MCP SDK 加 REST 跨語言驗證精確2次成功、撤銷後401與重啟後狀態保留；兩專案均通過。兩份 feature-motion／access-workflow WebM 亦完成全片解碼。
