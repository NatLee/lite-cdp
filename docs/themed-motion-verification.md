# 顧客足跡航圖：主題動態驗證

2026-11-27。此檔記錄先前已完成的前端階段。後續 GitHub／Google 綁定、OAuth、API／MCP 與真實用量已整合；請另見 [存取驗證](access-verification.md) 與 [存取指南](access.md)。

## 來源與視覺規則

- 全站 source CSS 與各頁 Tailwind spacing 改為固定 4px 倍數；保留 0／auto。字級、邊框與領域圖形座標不作 UI spacing 計算。
- 字重僅 400／600；普通表面圓角 0／4／8px；移除常駐表面陰影與玻璃覆層。
- Lucide 功能圖示 16／20／24px、stroke 1.5。資料圖、品牌插畫與圓形選取標記不當成功能 icon。
- 原始樣式掃描：`python docs/frontend_style_audit.py`。實際頁面 computed style 檢查納入截圖與流程腳本，覆蓋 padding／gap、字重、圖示和 backdrop-filter；auto margin 由來源檢查處理。
- 額外桌面 1440 與手機 390 路由巡查：20 個頁寬組合通過，詳見 [實際 computed-style 結果](themed-motion-style-audit.json)。檢查涵蓋設定與非首頁頁面。

## 執行證據

- `pnpm --dir src/web test`：22/22 通過。
- `pnpm --dir src/web build`：TypeScript 與 Vite production build 通過。
- `python docs/capture-workflow.py`：同一訪客 #63、session demo1v00062s1、自訂事件 #1837：訪客選取 → session 足跡 → 鍵盤選事件 → 個人時間軸。讀完所有分頁並核對 46 筆原始事件；重查 API 回應完全相同，沒有寫入任何資料。
- `python docs/capture-screenshots.py`：桌面、深色、390px 手機與選單流程通過；沒有 pageerror 或水平溢位。
- 實際流程檢查 prefers-reduced-motion：動畫停用時資料立即可見；選取狀態、session、事件／來信識別與鍵盤操作仍保留。
- `python -m py_compile docs/capture-workflow.py docs/capture-screenshots.py docs/frontend_style_audit.py`；PNG decode、WebM decode、README 本地圖片／連結檢查和 `git diff --check` 通過。

新增方向鍵／Home／End 選事件測試：實作前 4/5（新測試失敗），實作後 focused suite 5/5、完整 suite 22/22。事件焦點不跨越選定 session，焦點保持在對應按鈕。

## 動態與圖片

三張 workflow PNG 為隔離 demo 的連續操作。所有 README 舊版介面截圖一併更新。封面仍由可編輯的 [cover.svg](images/cover.svg) 產生，是領域插畫，沒有宣稱真實業務數字。

[實際操作 WebM](images/feature-motion.webm) 由 Playwright 記錄已登入後的工作流程；不含登入畫面。影片以 1440×1050 VP8 編碼，另解碼成逐格接觸表檢查游標／選信與內容切換。靜態 PNG 的動畫停在最終可讀狀態，不用靜態截圖宣稱動態。

重拍時指定 `DEMO_URL`、`DEMO_PASSWORD`，Inbox 另需 `DEMO_DATABASE_PATH`；只可指向隔離本機 demo。帳密不寫入版本庫。可用 `PLAYWRIGHT_CHROMIUM_EXECUTABLE` 選擇既有本機 Chromium。

## 人工覆核與界線

- 已檢視三張流程圖、更新封面、桌面／手機截圖與影片接觸表；文字、資料來源與操作步驟相符。主色仍可由品牌負責人另行確認。
- 截圖與影片使用內建虛構 demo，README alt text 說明實際操作，不將 demo 當真實顧客紀錄。
- 本階段不新增 API 或權限測試；既有 frontend 行為測試保留。OAuth 部署憑證、callback、MCP scopes 與真實用量稽核尚待下一階段實作。
- root 負責全分支獨立 review 與整合；本工作只提交，不 merge。
