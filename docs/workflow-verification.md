# 封面與三步操作驗證

2026-11-27，Chrome／Playwright，隔離的內建 demo，1440 × 1050、Asia/Taipei、淺色。前端沿用現有元件，沒有產品行為或權限變更。

## 封面

保留 `docs/images/cover.svg`。以實際 SVG 在 1440 px 與 README 常見的 800 px 寬度檢查：標題、主題圖形及文字可讀，沒有裁切。Inbox 是信紙與信封，CDP 是頁面／互動／身分三條事件航線；封面是品牌插畫，不是產品操作或即時資料，也沒有設定 GitHub Social Preview。

## 重拍

先以專案的 `demo` 指令初始化獨立的本機資料目錄，啟動 API 與前端；不要指向正式或多人共用資料庫。需要 `pip install playwright` 與 Chrome。帳密只由環境變數傳入，不寫進腳本或圖片。

```bash
export DEMO_URL=http://127.0.0.1:15375
export DEMO_USERNAME=demo
# 以自己的隔離 demo 設定 DEMO_PASSWORD。
python docs/capture-workflow.py
python -m py_compile docs/capture-workflow.py
git diff --check
```

此腳本只讀取應用資料。為讓圖內文字清楚，它從最近六位真實 demo 訪客中，選出一位具名會員及含自訂事件的 3–6 筆事件 session；不新增或改寫人物、事件或 session。重建 demo 時時間隨 seed 時刻改變，腳本依實際資料挑選，沒有固定假路徑或假計數。

## 實際操作與結果

1. 選訪客蔡欣怡（`person_id=63`）與 `session:demo1v00062s1`，航圖顯示這次來訪的 5 筆已載入事件。核對個人連結、session 選項與事件數，沒有 session ID 的事件各自分組。
2. 以鍵盤選中 `signup` 事件（`id=1837`），EVENT FOCUS 顯示 `/signup`。session 值保持不變；畫面沒有把其他人的事件或其他來訪混在這條路徑。
3. 點真實「開啟個人旅程」連結，URL 仍是同一人，保留「所有事件」條件；腳本讀取所有 API 分頁、載入 UI 的「更早的」直到結束，逐筆列表的 46 筆與 API 完全一致，再選回同一 session、同一事件。截圖為頁首與逐筆紀錄開頭，並非宣稱整條時間軸在一個視窗內。

實跑輸出：`PASS: visitor #63, session:demo1v00062s1, event #1837 stay consistent across 3 frames; 46 profile events verified; keyboard/mobile no overflow or page errors; read-only.`

前兩張 PNG 為 1440 px 桌面視窗內 `.journey-observatory` 的元件裁切（1360 × 772）；第三張為 1440 × 1050 完整視窗。實際逐張檢查：同一人、同一來訪日期與 5 筆事件一致，選中 signup 後的名稱及路徑清楚。390 × 844 下再操作相同訪客／session／事件，驗證鍵盤選取與焦點內容，總覽和個人頁沒有文件水平溢出。航圖內部可水平捲動是既有功能，沒有擴大整個文件。

## 靜態檢查

- 兩次真實流程重跑通過，沒有瀏覽器 page error。
- `python -m py_compile docs/capture-workflow.py` 通過。
- `git diff --check` 通過。
- Pillow `Image.verify()` 成功解碼三張 PNG；README 的本機 Markdown／HTML 圖片與文件連結逐一確認存在；SVG XML 解析與 Chrome 兩種寬度渲染成功。
- 沒有產品原始碼改動，因此未為文件或圖片新增鏡像測試；產品測試與 build 交由既有 PR CI 執行。流程本身由腳本的真實 UI 與 API 斷言驗證。

## 維護決策

沿用合格封面及既有專用元件，沒有為本輪重做品牌。舊的功能截圖移入 README 可收合圖庫，主區改為連續操作故事。實作與本文件由作者先自查，再交由主代理獨立審查後合併。
