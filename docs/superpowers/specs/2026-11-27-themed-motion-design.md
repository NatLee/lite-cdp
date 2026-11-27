# 八專案：主題工作台、動態與連線稽核設計

狀態：2026-11-27 前端方案依使用者持續的直接實作授權推進。使用者已確認 GitHub＋Google 帳號登入／綁定，以及 MCP/API OAuth 委託授權。本提交僅完成前端；存取功能由 root 的共用合約另行實作。

## 目的與已知要求

八個專案都要有主題相符的 SVG／獨立元件、能表達功能的動畫、清楚可辨的 section、真實操作截圖與 README cover。保留現有元件拆分、可讀性、資料真義與帳號權限。使用者指定先列問題清單，已在對話列出來源與修正方向。

嚴格約束：每個專案只有一個主要強調色系；暖／冷中性色；禁止紫藍行銷漸層、光斑與玻璃；400/600 兩種字重；padding/margin/gap 僅採 4px 倍數的固定 token（0、auto 可用於正常排版，視窗寬度用 grid/width 而非百分比 padding/gap）；普通表面採 0/4/8px 圓角與細框；不使用常駐重陰影。單一 Lucide 圖示語言，16/20/24px 與 stroke-width 1.5；圖表／領域插圖的幾何座標不屬於 UI spacing 或功能 icon。語意資料色保留必要分類，使用文字或紋理輔助辨識。文字大小不限於 4px 倍數，但小型裝飾字須整理成可讀資訊層級；大標一般32–48px，主工作區24–32px。

## Approaches

1. 只清 token：一致性提升，功能動態與區段節奏不會明顯改善。
2. 以展示 landing page 主導：封面鮮明，但把實際工作內容往下推，不符合資訊密度與功能展示需求。
3. **主題工作台（採用，實作者選擇）**：既有領域元件進一步連動，加入選取／展開／切換／狀態完成的短動畫，章節以標籤、底色、邊線、比例與密度區分；實際工作畫面就是展示。

保留八套已存在的不同構圖，不套一個通用 dashboard。主色提案：ES cobalt、Vault ochre、Inbox vermilion、CDP ink-blue、信義 forest-green、Hotel walnut、情緒 plum、KEYPO olive。沿用既有字型或其可用字重，確保實際字型有400/600，不以全站 CSS 強制覆蓋掩蓋散落的舊值。

## 問題證據

| Repo／位置 | 實際問題 | 修改 |
|---|---|---|
| ES docs/atlas.css:166,274,1095 | 800字重、青色光暈、最高98px標題 | 400/600、平面狀態、編譯工作台比例 |
| Vault src/web/src/index.css:177,179,191,298 | 38/17/42px間距、500字重 | spacing tokens與兩級字重 |
| Inbox index.css:51,1217 / Layout.tsx:51 | 6/10px spacing、登入重陰影 | 4px節奏、紙面與細線層次 |
| CDP index.css:1080,1084,1089,1099 | 大投影、零碎間距、放射光斑、500字重 | 足跡路徑、平面節點與可讀紀錄 |
| Sinyi visual-insights.css:48,82,306 / Layout.tsx:305,329 | 零碎spacing、玻璃覆層、drawer重影 | 不透明區段表面與明確新聞檔案 |
| Hotel visual-insights.css:17,76,78 / index.css:135 | 玻璃、選取投影、23/30px spacing | 剖面／簡報／評論三段連動 |
| Sentiment style.css:12,355,676 | 字型權重宣告不準、工作區投影、量管glow | 正確400/600、平面儀器與資料過渡 |
| KEYPO newsroom.css:28,95,142,157 / Layout.tsx:407,424 | 650–900字重、25px spacing、mobile glass | 報刊式情報工作台、精確高峰定位 |

共享規則需覆蓋登入、設定、表格、tooltip、toast、空資料、錯誤與手機頁，不能只修首頁。既有純色圖表淡填色／網格線不能一概稱為紫藍漸層；本次可用純色或SVG pattern取代裝飾性效果。

## 各專案操作與動態

| 專案 | 清楚的章節 | 必須實作並可展示的互動動態 |
|---|---|---|
| ES | 輸入／編譯階段／子句／輸出成品 | 選語法或子句時，對應階段與節點以120–200ms更新；由實際Rust產生的結構驅動，不假裝支援靜態頁任意編譯 |
| Vault | 主題索引／記錄架／語法拆解／編輯與版本 | 選擇或展開記錄時，索引標記與語法詳情同步；儲存成功才出現短回饋 |
| Inbox | 到件帳／狀態分類／來信閱讀／處理歷史 | 選信時定位標記與閱讀內容銜接；狀態與備註寫入成功後才更新標記，不改來信時間 |
| CDP | 訪客索引／單次來訪足跡／事件詳情／個人時間軸 | 同一session內選事件時足跡游標定位、對應事件詳情過渡；保持真實時間與事件類別，若路徑等距須明示非時間距離 |
| Sinyi | 居住街區／品牌構面比較／新聞證據 | 建築選取短位移與對應比較更新，閱讀入口定位新聞；不創造房價、成交數或真實地圖 |
| Hotel | 旅宿剖面／服務簡報／評論閱讀／追蹤事件 | 選空間後短線段或定位標記連到對應服務簡報；評論與事件保留同一文章識別，不發送通知 |
| Sentiment | 文字輸入／機率地圖與通道／複核原文／匯出收據 | 選樣本時定位標記／通道同步160ms更新；數字即時顯示，機率語意不變，人工資料持續標明來源 |
| KEYPO | 研究條件／主題高峰／頻道足跡／原文證據 | 主題或時段切換以定位線／交叉淡入顯示選取；沿用既有篩選、空頻道與子維度交集，不暗示傳播因果 |

所有動態遵循 prefers-reduced-motion；停用時立即顯示最終狀態，不能隱藏資料。不得自動無限掃描、跳數或偽造即時資料；保留鍵盤焦點。可使用動畫讓狀態更清楚，不為每個按鈕添加不同效果。

## 連線與稽核：已盤點、另依共用存取合約實作

六個Rust網站是Axum/SQLite的admin/member帳號與cookie session。Sinyi/Hotel/KEYPO有匯入專用的單一bearer secret，沒有一般scope key。KEYPO已有登入／操作日誌，但不是完整API計量。Inbox表單與CDP收集器是公開寫入通道，不能誤改成個人API憑證。ES是Rust函式庫／靜態文件；Sentiment是無帳號層的Flask推論。

已確認：GitHub＋Google 外部帳號登入／明確綁定，加上 MCP/API OAuth 委託授權。保留本地帳號與密碼，不憑 email 自動合併；MCP 先採唯讀 scope。具體合約由 root 協調，本前端階段不新增授權接口。

可先確定的資料與頁面提案：
- 帳號綁定：provider/issuer、subject的遮罩識別、綁定時間、最後登入與解除；保留現有users.id，不憑email自動合併。
- 應用授權：client名稱、授權範圍、授權／到期／最後使用時間、撤銷。
- API金鑰：名稱、前綴、scope、資源限制、建立／到期／撤銷／最後使用；原值僅建立時顯示，資料庫存hash。
- 用量／紀錄：日期範圍、時區、API與MCP分開的嘗試／成功／失敗，分日圖與可篩選表格。MCP工具一次執行算一次，不把內部服務重用再算成REST；初始化與list-tools不算領域工具執行。公開收集量另列。
- 權限是目前帳號狀態、角色／成員／擁有者、scope與憑證資源限制的交集；每次請求重新套用，撤銷／移除成員後不可繼續存取。
- 稽核只存安全metadata：UTC時間、request-id、user/client/credential-id、通道、操作、資源識別、狀態、耗時；不記錄token、cookie、授權碼、評論內文、客戶事件payload、推論文字。
- 新增numbered migrations，不更改既有0001；各repo獨立部署。ES建議新增可選server，不把HTTP/auth依賴灌入預設library；Sentiment需要獨立持久化存取層。

後續具體 OAuth/MCP 合約與授權測試需另完成設計與實作，不能以可點的空頁或假用量宣稱完成。

## 驗證與圖片交付

每repo都要實際跑桌面1440與手機390的核心流程、鍵盤、無水平溢位、reduced-motion；真實資料或明示demo，資料識別／篩選貫穿截圖。至少三張功能操作PNG，另以短WebM或GIF展示核心動畫（工具允許時；不可用靜態截圖宣稱已展示動態）。cover隨新色系／字重／構圖更新，保留可編輯來源。新稽核頁需真實請求產生日誌後再拍，不能用硬編數字。

只對行為變更新增有意義測試；風格改動以瀏覽器、樣式規則掃描、build與適當既有回歸確認。樣式掃描應報來源，不用巨大important override掩蓋問題。獨立review後沿用使用者授權的PR／merge／刪工作分支流程；未完成OAuth時不宣稱全案完成。

## 參考資料

- Carbon spacing：借鏡token與層次方法；使用者的4px限制比其含2px的原尺度更嚴格，優先依使用者要求。https://www.carbondesignsystem.com/building-blocks/foundations/spacing/overview
- W3C interaction animation：尊重使用者停用非必要動態的偏好。https://www.w3.org/WAI/WCAG22/Understanding/animation-from-interactions.html
- MCP authorization（本次查閱latest導向2026-07-28）：保護的MCP資源、授權伺服器發現與OAuth角色應分清楚。https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization
- OAuth security BCP：https://www.rfc-editor.org/rfc/rfc9700

## 本 repo 執行範圍

顧客足跡航圖：訪客索引／session足跡／事件詳情／完整時間軸；選事件定位游標。本輪先實作前端；OAuth/MCP/API仍屬整體任務，依root另訂的具體合約推進，不能宣稱已完成。
