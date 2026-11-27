<p align="center">
  <img src="docs/images/cover.svg" alt="Lite CDP：從一次造訪，到真正認識" width="100%">
</p>

<p align="center">
  <a href="https://github.com/NatLee/lite-cdp/actions/workflows/ci.yml"><img src="https://github.com/NatLee/lite-cdp/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <img src="https://img.shields.io/badge/backend-Rust%20%2B%20axum-b7410e" alt="Rust + axum">
  <img src="https://img.shields.io/badge/frontend-React%20%2B%20Tailwind-124bb6" alt="React + Tailwind">
  <img src="https://img.shields.io/badge/docker-one%20image-2496ed" alt="Docker">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT">
</p>

流量分析工具會告訴你「這個月有幾個人來」，但行銷和業務想知道的通常是另一件事：**哪些人**看了價格頁三次卻沒留資料、上個月買過咖啡豆的人最近有沒有再回來、放進購物車沒結帳的是誰。

Lite CDP 是一個自己架的小型顧客資料平台。在網站上貼一行程式碼，它就開始記每個訪客看了哪些頁、點了什麼；訪客登入或留下 email 的那一刻，之前匿名看過的紀錄會併到這個人身上。接著用規則幫人貼標籤，再用標籤和條件組出名單，匯出成 CSV 拿去寄信或投廣告。

一個 Docker 映像檔就是全部：Rust 後端、React 前端、SQLite 資料庫。資料留在你自己的機器上。

## 三步讀懂一次來訪

以下三張圖來自隔離的內建 demo，連續追蹤同一位訪客、同一個已記錄的 `session_id` 與同一筆事件。封面是獨立的航線插畫，操作圖則取自真實介面。

### 1. 選訪客，再選一次來訪

在總覽左側選一位已辨識的訪客，再用來訪選單選一組事件。圖中路徑依資料庫中的 session 分組，節點等距表示事件順序；沒有 session ID 的事件不會被串成一次來訪。

![步驟一：選定同一位訪客與一次已記錄的來訪（總覽航圖局部）](docs/images/workflow-01.png)

### 2. 選事件，讀取內容

以鍵盤選取自訂事件節點，EVENT FOCUS 隨之顯示名稱、頁面路徑、時間、來源與已記錄屬性。這是選取事件的原始資訊，不代表推測出的轉換率。

![步驟二：同一次來訪中的自訂事件與 EVENT FOCUS（總覽航圖局部）](docs/images/workflow-02.png)

### 3. 開啟同一人的完整時間軸

點「開啟個人旅程」，進入同一人的資料頁。來訪選單仍可選回同一組 session 與事件，下方逐筆紀錄包含此人的其他來訪；更多紀錄可按「更早的」載入。重拍腳本會載入所有分頁並核對事件總數，圖中保留頁面頂部與逐筆紀錄開頭。

![步驟三：同一位訪客的個人時間軸，保留相同來訪與事件](docs/images/workflow-03.png)

[觀看同一次來訪的事件游標與詳情切換（WebM）](docs/images/feature-motion.webm)。游標依實際選取事件以 180 ms 移動，詳情同步呈現原始時間與屬性；可用方向鍵、Home、End 選事件。節點等距只表示順序。系統啟用「減少動態」時游標與詳情立即到位，鍵盤焦點仍保留。

前兩張是 1440 px 桌面視窗內的航圖元件裁切；第三張為完整視窗。重拍與資料一致性檢查見 [操作驗證](docs/workflow-verification.md) 與 [capture-workflow.py](docs/capture-workflow.py)。腳本只讀取 demo 資料。

新版來源樣式、桌面／手機檢查與錄影驗證見 [主題動態驗證](docs/themed-motion-verification.md)。

## 功能

- **一行程式碼就能裝**：自動記錄瀏覽、點擊和捲動深度，單頁應用（SPA）換網址也算。追蹤程式碼壓縮前約 7 KB，不用 cookie，不載入任何第三方的東西。
- **把匿名訪客認出來**：呼叫 `identify` 之後，這個瀏覽器之前的匿名紀錄會併到這個人身上；同一個 email 在別的裝置登入也會併在一起。
- **顧客行為航圖**：從最近出現的訪客選一人，再選一次來訪。瀏覽、互動、辨識分列成事件路徑，點節點查看時間、頁面、來源與屬性。
- **每個人的完整時間軸**：看了哪些頁、從哪裡來、用什麼裝置、觸發過哪些事件，一次工作階段一段。
- **標籤**：用規則描述「做過什麼事的人」（看過某個路徑底下的頁面、網址或標題含某些字、觸發過某個事件），可以設定要符合幾次才算。規則改了，舊資料會跟著重算。
- **分群**：用標籤、來源、email、最近有沒有來、看過幾頁組出一份名單，分成「全部都要符合／至少符合一個／不能符合」三組。一邊調條件一邊看符合幾個人，存起來之後每次打開都是當下的結果。
- **回訪地圖**：把已辨識會員／匿名訪客，按最後造訪時間分成近期活躍、回訪降溫、久未回訪。點任一座標就能查看那一群人。
- **每日訊號探索**：切換瀏覽頁數／每日訪客，點圖表或拖動日期軸查看當天數據；鍵盤也能操作。
- **CSV 匯出**：名單和分群都可以匯出，包含身分、活躍時間、搜尋、標籤與分群條件。
- **多個網站、多個帳號**：每個網站有自己的追蹤金鑰和允許的來源。成員只看得到指定給他的網站。
- **隱私**：不存訪客的 IP；可以設定事件只留幾天；可以整個刪掉某個人；可以尊重瀏覽器的「不要追蹤」。
- **帳號**：第一個打開網頁的人建立管理員，沒有預設帳號，也沒有預設密碼。密碼用 Argon2 雜湊，登入是 HttpOnly cookie。
- 淺色和深色模式。

## 畫面與工作流程

以墨藍與同色系淺藍構成「Journey Observatory / 顧客行為航圖」：網站切換和功能導覽位於上方，保留完整寬度給名單、身份與行為時間軸。所有數字來自 API；截圖使用內建虛構 demo 資料。

### 從一次來訪開始

總覽先呈現最近出現的六位訪客。選擇一位後，讀取最近 50 筆事件，以已記錄的 `session_id` 分組；沒有來訪識別的事件各自呈現。每次來訪按時間與事件編號排列，節點等距表示先後順序，不是時間比例。資料可能跨多頁，因此畫面明示單次來訪可能尚未完整。

### 從足跡到身份

航圖下方以六個座標呈現目前身分 × 最後造訪：7 天內、7–30 天、超過 30 天。每人只在一格，圓面積代表人數、百分比以網站全部訪客為分母。點格子進入同樣條件的名單，也能直接匯出 CSV。這是目前的分布，不是轉換率或歷史遷移。

每日曲線可切換瀏覽頁數與訪客，點選或用日期滑桿查看一天的兩項數值。每日訪客按當天去重，跨日數字不當成全期獨立人數加總。

### 名單與個人旅程

搜尋、標籤、分群與最近活躍篩選共同整理名單；個人頁的寬幅航圖可切換來訪與事件種類，往下保留逐筆紀錄、載入更早事件、身份、標籤、裝置和來源。

### 規則與分群

標籤按事件規則自動計算。分群編輯器保留三組條件，以及隨條件更新的人數與名單預覽。

<details>
<summary>完整總覽、標籤、安裝、其他功能、登入、深色與手機畫面</summary>

![淺色介面的航圖局部](docs/images/screenshot-journey-light.png)
![深色介面的航圖局部](docs/images/screenshot-journey-dark.png)

![訪客名單](docs/images/screenshot-people-light.png)
![個人旅程](docs/images/screenshot-person-light.png)
![分群編輯](docs/images/screenshot-segment-light.png)

![完整總覽：行為航圖、每日流量與目前身分分布](docs/images/screenshot-overview-light.png)

![標籤](docs/images/screenshot-labels-light.png)
![安裝](docs/images/screenshot-install-light.png)
![登入](docs/images/screenshot-login-light.png)
![深色總覽](docs/images/screenshot-overview-dark.png)
<img src="docs/images/screenshot-mobile.png" alt="手機顧客總覽" width="390">

</details>

## 快速開始

```bash
git clone https://github.com/NatLee/lite-cdp.git
cd lite-cdp
docker compose up -d --build
```

打開 <http://127.0.0.1:8585>，第一次會請你建立管理員帳號。接著：

1. 到「所有網站」新增一個網站。
2. 「安裝與設定」頁有一行追蹤程式碼，貼進網站每一頁的 `<head>`：

   ```html
   <script async src="http://127.0.0.1:8585/t.js" data-key="你的追蹤金鑰"></script>
   ```

3. 打開你的網站逛幾頁，回到「名單」就會看到一個匿名訪客。

想先看有資料的樣子，可以在**空的**資料庫裡建立範例帳號、網站和訪客（密碼會印在畫面上）：

```bash
docker compose run --rm lite-cdp lite-cdp demo
```

忘記密碼的時候：

```bash
docker compose exec lite-cdp lite-cdp reset-password <帳號>
```

## 追蹤程式碼

裝好之後會自動記錄每一頁的瀏覽、點了哪些連結和按鈕、頁面捲到多深。除此之外有兩件事要自己告訴它。

**訪客是誰。** 在訪客登入、註冊或留下 email 的時候：

```html
<script>
  window.cdpq = window.cdpq || [];
  cdpq.push(["identify", { email: "amy@example.com", name: "Amy", plan: "pro" }]);
</script>
```

`email`、`name`、`account` 以外的欄位會存成自訂屬性。登出時呼叫 `cdp.reset()`，之後的紀錄就不會再算在這個人身上。

**特別想記的動作。** 例如加入購物車、送出表單：

```js
cdp.track("add_to_cart", { sku: "A-1001", price: 1280 });
```

不想寫程式的話，在按鈕上加 `data-cdp-click="add_to_cart"` 也可以。事件名稱可以拿來建標籤。

`cdpq.push([...])` 的寫法在追蹤程式碼還沒載入時也能用；載入之後也可以直接呼叫 `cdp.identify()`、`cdp.track()`、`cdp.page()`、`cdp.reset()`。

可以加在 `<script>` 上的選項：

| 屬性 | 說明 |
|---|---|
| `data-key` | 追蹤金鑰，必填 |
| `data-respect-dnt="true"` | 瀏覽器開了「不要追蹤」的訪客就不記錄 |
| `data-auto-click="false"` | 不要自動記錄點了哪些連結和按鈕 |
| `data-auto-scroll="false"` | 不要自動記錄頁面捲到多深 |
| `data-endpoint="https://…/collect"` | 資料改送到別的網址（例如經過自己的反向代理）|

不想被記錄的區塊加 `data-cdp-ignore`，裡面的點擊就不會送出來。

### 放到網路上

前面放一個有 HTTPS 的反向代理（Caddy、nginx…），然後設定：

- `CDP_PUBLIC_URL`：外部看到的網址。「安裝與設定」頁的追蹤程式碼用它組，是 https 時登入 cookie 會加 `Secure`。
- `CDP_TRUST_PROXY=true`：流量限制是照 IP 算的，這樣才看得到訪客真正的 IP。
- 每個網站的「允許的來源」填上自己的網址。追蹤金鑰本來就寫在網頁原始碼裡，誰都看得到；填了之後，別的網站拿你的金鑰也送不進資料。

## 隱私

Lite CDP 存的是個人資料，請照你所在地的法規告知訪客並取得同意。它自己做了這幾件事：

- **不存 IP。** IP 只在記憶體裡用來限制流量。
- **不用 cookie。** 訪客的匿名編號放在他瀏覽器的 `localStorage`，只在你的網站上有效，不會跨站。
- **可以設定保留期限。** `CDP_RETENTION_DAYS=365` 會每天刪掉一年以前的事件；預設不刪。
- **可以刪掉一個人。** 管理員在那個人的頁面按刪除，他的資料、事件和標籤會一起刪掉。
- **可以尊重「不要追蹤」。** 加上 `data-respect-dnt="true"`。
- 認得出來的爬蟲不會被記錄。

## 設定

都是環境變數，全部可以不設。用 docker compose 時寫在同一個資料夾的 `.env`（[.env.example](.env.example) 有範例）。

| 變數 | 預設 | 說明 |
|---|---|---|
| `CDP_PORT` | `8585` | docker compose 對外的 port |
| `CDP_PUBLIC_URL` | | 外部看到的網址，例如 `https://cdp.example.com` |
| `CDP_COOKIE_SECURE` | 看 `CDP_PUBLIC_URL` | `true` / `false`，強制指定 cookie 要不要加 `Secure` |
| `CDP_TRUST_PROXY` | `false` | 前面有反向代理時設 `true`，訪客的 IP 改看 `X-Forwarded-For` |
| `CDP_RATE_LIMIT` | `600` | 同一個 IP 一分鐘最多送幾次資料，`0` 是不限制 |
| `CDP_RETENTION_DAYS` | `0` | 事件留幾天，過期自動刪除；`0` 是不刪 |
| `CDP_BIND` | `0.0.0.0:8585` | 伺服器聽的位址 |
| `CDP_DATA_DIR` | `data`（映像檔裡是 `/app/data`） | 放 SQLite 資料庫的資料夾 |
| `CDP_DATABASE_PATH` | `<CDP_DATA_DIR>/lite-cdp.db` | 資料庫檔案 |
| `CDP_STATIC_DIR` | `src/web/dist`（映像檔裡是 `/app/web`） | 前端檔案的位置 |
| `CDP_DEMO_PASSWORD` | 隨機產生 | `demo` 指令建立的範例帳號要用的密碼 |

資料都在 `lite-cdp-data` 這個 volume 的 SQLite 檔案裡，備份那個檔案就好。

## 適合和不適合

適合：一個或幾個網站、想知道「誰」做了什麼、想自己保管資料的團隊。只要一台小機器和一個 SQLite 檔案。

不適合：需要即時串接廣告平台、跨 App 和線下資料整合、或每天上億筆事件的情境。那是大型 CDP 的工作。

## 開發

需要 Rust（stable）、Node 24、pnpm。開發固定使用 `http://127.0.0.1:5173`；後端的 `ACCESS_PUBLIC_URL` 必須與前端相同。若另設 `CDP_PUBLIC_URL`，也必須同 origin。

```bash
# 後端：http://127.0.0.1:8585
cd src/api && ACCESS_PUBLIC_URL=http://127.0.0.1:5173 cargo run

# 前端：http://127.0.0.1:5173，/api、/collect 和 /t.js 會轉給後端
cd src/web && pnpm install && pnpm dev --host 127.0.0.1 --port 5173 --strictPort
```

```bash
cd src/api && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd src/web && pnpm test && pnpm build
bash docs/smoke-test.sh http://127.0.0.1:8585   # 對一個空的執行個體跑一輪
```

API 和收資料的格式在 [docs/api.md](docs/api.md)。

## 設計與驗證

[產業介面設計與驗證](docs/industry-design.md) 說明這次的專用元件與真實資料互動；[視覺分析設計與驗證](docs/visual-insights.md) 說明分布定義與驗證；[原版型紀錄](docs/design.md) 說明版型、響應式、後端修正及驗證範圍。封面是可編輯的 [SVG](docs/images/cover.svg)；應用畫面由 [擷取腳本](docs/capture-screenshots.py) 對隔離的 demo 執行個體重拍。

## 授權

[MIT](LICENSE) © Nat Lee

## 帳號綁定、OAuth、API 與 MCP

原生「連線與存取」可綁定 GitHub／Google、建立與撤銷金鑰、管理應用授權及核對真正用量。`reports:read` 只讀取站台與彙總報表；`people:read` 才能讀取個人事件、完整事件網址與屬性。建立金鑰預設不含個人事件，需另外勾選。 設定與協定見 [存取指南](docs/access.md)，本機真實操作見 [錄影](docs/images/access-workflow.webm) 與 [稽核截圖](docs/images/access-03-usage.png)。


## 首次登入與建立管理員

第一次開啟空白安裝時，首頁、登入頁和需要登入的頁面都會自動導向 `/first-login`。輸入 3–32 個英文字母、數字或 `. _ -` 的帳號、選填顯示名稱，以及 8–200 個字元的密碼與確認密碼，按「建立管理員」後即以新管理員身份進入工作空間。密碼不一致時不會送出請求。

這個頁面只用來建立第一位管理員。有帳號的安裝會改顯示 `/login`；其他成員可由管理員新增，或依註冊設定自行申請。若另一位管理員已先完成設定，舊頁面會自動轉成登入。透過 OAuth 登入的合法本站授權流程會在建立帳號後接續進行。

Docker 對外網址請維持 `CDP_PUBLIC_URL` 與 `ACCESS_PUBLIC_URL` 同源。操作與實際瀏覽器驗證見 [首次登入驗證紀錄](docs/first-login-verification.md)。

![首次登入頁](docs/images/first-login-1440-light.png)

## 成員註冊與審核

`/first-login` 只用來建立第一位管理員。後續成員可從登入頁的「註冊帳號」進入 `/register`；新安裝預設關閉註冊。管理員可在「設定 → 註冊與審核」選擇關閉、審核後開通，或開放立即註冊。關閉只停止新申請，仍可處理已送出的申請。

密碼註冊包含確認密碼；帳號為 3–64 個英數字或 `. _ -`，密碼為 12–256 個字元，顯示名稱選填、最多 100 個字元。設定 Google／GitHub 後，註冊頁只顯示可用的供應商。第三方註冊會建立一般成員，不會依電子郵件合併既有帳號；原有第三方「登入」入口仍只登入已綁定的帳號。

待審核或已拒絕的帳號無法進入工作區，也無法使用 API／MCP。管理員可依狀態分頁查看申請、核准或填寫原因後拒絕；下方處理紀錄保留申請、審核及註冊設定變更的時間與操作者。既有帳號的停用／啟用不會取代註冊審核。

[註冊與審核驗證紀錄](docs/registration-verification.md) 提供測試範圍與實際瀏覽器截圖。

實際 Chrome 畫面：[註冊頁・桌面淺色](docs/images/registration-register-desktop-light.png) · [管理員審核・手機深色](docs/images/registration-review-mobile-dark.png)。完整桌面／手機、淺色／深色共八張截圖見驗證紀錄。 可在建置後執行 `python docs/verify-registration.py --binary /path/to/binary` 重現完整流程；需安裝 Python Playwright 與 Google Chrome。
