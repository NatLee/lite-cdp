# 首次登入驗證 — 2026-11-27

## 行為與邊界

`/first-login` 使用專用頁面與原有主題元件；只開放空白安裝建立第一位管理員，之後成員仍由管理員新增。帳號沿用後端 3–32 個 ASCII 英數字與 `. _ -` 規則；選填顯示名稱最多 40 個字元，密碼為 8–200 個 Unicode 字元。確認密碼不會送到 API。登入與初始化成功後清除元件密碼，密碼不寫入 URL、日誌或瀏覽器儲存。

原有後端與 `crates/access-kit` 完全未改動。已檢查 `src/api/src/routes/auth.rs` 的單一 `INSERT ... SELECT ... WHERE NOT EXISTS` 寫入與 `rows_affected` 衝突判斷，保留既有原子初始化、管理員角色及 session cookie 行為。HTTP 409 代表初始化已完成，前端即關閉舊表單並刷新狀態；刷新暫時失敗仍導向登入。

## 自動化

- 新測試先在原始實作執行，9 個案例均因缺少專用頁面、確認密碼與路由行為而失敗；實作後全部通過。
- 額外的「409 後狀態服務暫時失敗仍關閉初始化」案例先失敗，再修正後通過。
- `pnpm test`：47 個測試通過；包括空白首頁／登入／受保護頁路由、密碼不一致不送 API、成功進入工作空間、現有安裝關閉初始化、不安全 next 移除、合法 OAuth next 保留、409 刷新、伺服器錯誤重試、重複送出防護及既有外部登入失敗提示保留。
- `pnpm build`：通過 TypeScript 與 Vite 正式編譯。
- 沒有後端程式碼變更，因此此次不重跑 Rust 測試。

## 真實瀏覽器

在全新、一次性 Docker 資料庫上，以本機 Chrome 的真實頁面及未改動後端執行，前端使用本次 `dist` 掛載。所有驗證使用隨機測試密碼，只保留記憶體，不記錄 request body、密碼或 session cookie。

1. 從 `/settings` 進入空白安裝，網址轉至 `/first-login`。
2. 在 1440px／390px、light／dark 各拍一張空白表單；皆無水平溢出，確認系統深淺色已生效。所有 context 啟用 reduced motion，並實際驗證 Tab／Shift+Tab 在帳號與顯示名稱間移動焦點。
3. 不相同的確認密碼顯示可讀錯誤，觀察到零次 setup request。
4. 密碼相同時只送一次 setup，進入正常工作空間；`/api/auth/state` 確認 session 為 admin 且不再需要初始化。
5. 已登入時直接開 `/first-login` 回到正常首頁。
6. 另一個已開啟的初始表單送出後收到真實 HTTP 409，自動刷新並轉為 `/login`。
7. 按登出後回 `/login`，使用剛建立的帳密再次登入成功。

未模擬畫面或更換 API 回應。所有螢幕截圖都在填入帳密前拍攝。桌面與手機版本保留既有識別、鍵盤焦點樣式、表單標籤及自然捲動。

| 寬度 | 淺色 | 深色 |
| --- | --- | --- |
| 1440 | [桌面淺色](images/first-login-1440-light.png) | [桌面深色](images/first-login-1440-dark.png) |
| 390 | [手機淺色](images/first-login-390-light.png) | [手機深色](images/first-login-390-dark.png) |

測試容器與其獨立匿名資料卷完成後移除；既有容器、映像檔及其他資料卷保留。原有 README 圖庫和圖片保留。
