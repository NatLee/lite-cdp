import { useEffect, useState } from "react";
import { Check, Copy, Radio, ShieldCheck, Terminal, ArrowUpRight } from "lucide-react";
import { api } from "../api";
import { useSites } from "../components/Layout";
import { ErrorBanner, useToast } from "../components/ui";
import { useApp, useUser } from "../context";
import { parseLines, timeAgo } from "../lib/format";
import { errorText } from "../lib/hooks";
import type { Site } from "../types";
import { IdentitySimulator } from "./IdentitySimulator";
import { legacyPolicy, minimalPolicy, type CollectionPolicy, previewUrl } from "./policy";
import { frameworks, frameworkNotes, installationTemplate, lifecycleTemplate, type Framework } from "./templates";

function Code({label,code}:{label:string;code:string}) {
  const [copied,setCopied]=useState(false);
  return <div className="signal-code"><pre aria-label={label}>{code}</pre><button type="button" aria-label={`複製${label}`} onClick={()=>void navigator.clipboard.writeText(code).then(()=>{setCopied(true);window.setTimeout(()=>setCopied(false),1500);}).catch(()=>{})}>{copied?<Check size={16}/>:<Copy size={16}/>} {copied?"已複製":"複製"}</button></div>;
}
function PolicyEditor({site,policy,onChange}:{site:Site;policy:CollectionPolicy;onChange:(policy:CollectionPolicy)=>void}) {
  const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null),[dirty,setDirty]=useState(false);
  const {reloadSites}=useSites();const toast=useToast();
  const change=(next:Partial<CollectionPolicy>)=>{onChange({...policy,...next});setDirty(true);};
  const booleans:[keyof CollectionPolicy,string,string][]=[
    ["consent_required","要求訪客同意","同意前不建立 ID、不累積佇列。CMP 每次開頁通知選擇。"],
    ["respect_dnt","尊重 Do Not Track","啟用時，瀏覽器 DNT 優先於同意。"],
    ["auto_page","自動頁面瀏覽","關閉後只在應用主動呼叫 page() 時記錄。"],
    ["auto_click","自動點擊","只收連結、按鈕和 data-cdp-click。"],
    ["auto_scroll","自動捲動深度","記錄 25 / 50 / 75 / 100%，每頁各一次。"],
    ["click_text","包含點擊文字","文字可能含個人資訊；關閉時使用一般 click 名稱。"],
    ["legacy_email","相容舊版 email 識別","只匹配尚無 user_id 的舊版人物。不同會員 ID 永不因 email 合併。"],
    ["require_signed_identity","要求伺服器簽署會員 ID","啟用前先完成後端簽章；一般瀏覽器 identify 屬於網站宣告身分。"],
  ];
  async function save(){setBusy(true);setError(null);try{for(const selector of policy.ignore_selectors){try{document.querySelector(selector);}catch{throw new Error(`無效的 CSS selector：${selector}`);}}await api.updateSite(site.id,{collection_policy:policy});await reloadSites();setDirty(false);toast("採集政策已儲存");}catch(err){setError(errorText(err));}finally{setBusy(false);}}
  return <section className="signal-panel policy-editor" aria-labelledby="policy-title"><div className="signal-section-heading"><div><p className="signal-kicker">01 / COLLECTION POLICY</p><h2 id="policy-title">每個網站，明確決定收什麼。</h2></div><button className="btn btn-secondary" onClick={()=>{onChange({...minimalPolicy});setDirty(true);}}>使用最少採集設定</button></div>
    <p className="signal-intro">現有站點沿用舊版規則。調整後按儲存；後端立即檢查，新頁面載入最新設定。下方模擬器會先預覽尚未儲存的選擇。</p>
    <div className="policy-grid">{booleans.map(([field,label,hint])=><label className="policy-toggle" key={field}><input type="checkbox" aria-label={label} checked={Boolean(policy[field])} onChange={e=>change({[field]:e.target.checked})}/><span><strong>{label}</strong><small>{hint}</small></span></label>)}</div>
    <div className="policy-fields">
      <label>SPA 換頁模式<select className="input" value={policy.spa_mode} onChange={e=>change({spa_mode:e.target.value as CollectionPolicy["spa_mode"]})}><option value="history">History（React / Next / Vue）</option><option value="hash">Hash（#/route）</option><option value="manual">Manual（自己呼叫 page）</option></select></label>
      <label>網址參數<select className="input" value={policy.query_mode} onChange={e=>change({query_mode:e.target.value as CollectionPolicy["query_mode"]})}><option value="strip">全部移除 query</option><option value="allowlist">只保留清單內參數</option><option value="keep">保留全部（舊版）</option></select></label>
      <label>來源網址<select className="input" value={policy.referrer_scope} onChange={e=>change({referrer_scope:e.target.value as CollectionPolicy["referrer_scope"]})}><option value="none">不收</option><option value="origin">只收 origin</option><option value="full">完整網址（仍套參數設定）</option></select></label>
      <label>允許的 query 名稱<textarea className="input" rows={2} value={policy.query_allowlist.join("\n")} placeholder="campaign" onChange={e=>change({query_allowlist:parseLines(e.target.value)})}/></label>
      <label>略過點擊的 CSS selectors<textarea className="input" rows={2} value={policy.ignore_selectors.join("\n")} placeholder=".private-area" onChange={e=>change({ignore_selectors:parseLines(e.target.value)})}/></label>
      <label>允許的會員 traits<textarea className="input" rows={2} disabled={policy.allowed_traits===null} value={(policy.allowed_traits||[]).join("\n")} placeholder="plan\ncompany_size" onChange={e=>change({allowed_traits:parseLines(e.target.value)})}/><span className="policy-legacy"><input type="checkbox" checked={policy.allowed_traits===null} onChange={e=>change({allowed_traits:e.target.checked?null:[]})}/> 相容舊版任意 scalar traits</span></label>
    </div>
    <div className="policy-preview"><ShieldCheck size={20}/><span>送出前預覽</span><code>{previewUrl("https://shop.example/pricing?email=private&campaign=spring#details",policy)}</code></div>
    {error&&<ErrorBanner message={error}/>}<div className="policy-footer"><button className="btn btn-primary" disabled={busy} onClick={()=>void save()}>{busy?"儲存中…":"儲存採集設定"}</button><span role="status">{dirty?"有尚未儲存的變更":"使用已儲存設定"}</span></div>
    <details className="signal-details"><summary>伺服器簽章、信任範圍與資料界線</summary><p>公開 key 不驗證登入身分。要標記「伺服器驗證」，請由自己的後端以 HMAC-SHA256 簽署本站 key、user_id、到期秒數；最長一小時。簽章只驗證會員 ID，email 和 traits 仍是網站提供的屬性。</p><Code label="伺服器簽章範例" code={`// Node.js server only. CDP_IDENTITY_SECRET 必須留在伺服器。\nimport { createHmac } from "node:crypto";\nconst userId = String(authenticatedUser.id);\nconst expires_at = Math.floor(Date.now() / 1000) + 900;\nconst message = [${JSON.stringify(site.key)}, userId, expires_at].join("\\n");\nconst signature = createHmac("sha256", process.env.CDP_IDENTITY_SECRET)\n  .update(message).digest("hex");\n// 將這個短效 proof 回傳給已登入的瀏覽器，再呼叫 identify：\nconst identity = { user_id: userId, proof: { expires_at, signature } };`}/><p>管理員可透過已登入的 <code>GET /api/sites/{site.id}/identity-secret</code> 取得本站簽章密鑰，放進自己的伺服器環境變數；不要貼進網站或 GTM。不同站點、子網域與裝置預設隔離。未實作跨站匿名串接。</p></details>
  </section>;
}
function Receipt({site,origin,framework}:{site:Site;origin:string;framework:Framework}) {
  const [test,setTest]=useState<{token:string;expires_at:string}|null>(null),[receipt,setReceipt]=useState<Awaited<ReturnType<typeof api.installationReceipt>>|null>(null),[error,setError]=useState<string|null>(null),[busy,setBusy]=useState(false);
  useEffect(()=>{setTest(null);setReceipt(null);setError(null);},[site.id]);
  async function start(){setBusy(true);setError(null);try{setTest(await api.createInstallationTest(site.id));setReceipt(null);}catch(e){setError(errorText(e));}finally{setBusy(false);}}
  async function refresh(){if(!test)return;setBusy(true);setError(null);try{setReceipt(await api.installationReceipt(site.id,test.token));}catch(e){setError(errorText(e));}finally{setBusy(false);}}
  return <section className="signal-panel receipt-panel"><div className="signal-section-heading"><div><p className="signal-kicker">04 / REAL RECEIPT</p><h2>讓真實網站回傳一張收據。</h2></div><Radio size={24}/></div><p className="signal-intro">這組短效測試碼只寫入診斷收據，不會建立人物、事件或標籤。仍檢查允許來源、同意和採集政策。請在你的網站測試頁替換原本 snippet。</p>
    <button className="btn btn-primary" disabled={busy} onClick={()=>void start()}>{test?"建立新的測試碼":"建立 15 分鐘測試碼"}</button>
    {test&&<div className="receipt-body"><p>到期：{new Date(test.expires_at).toLocaleString("zh-TW")}。安裝後依網站 CMP 授予採集同意並瀏覽一頁。</p><Code label="安裝測試碼" code={installationTemplate(framework,origin,site.key,test.token)}/><p>網站已載入 tracker 時，可在瀏覽器 Console 執行以下指令；此頁之後的事件會進測試收據，重新整理回到正式設定。</p><Code label="已載入網站測試" code={`cdp.test(${JSON.stringify(test.token)});\n// 僅在訪客已同意時通知：\ncdp.consent(true);\ncdp.page();\ncdp.flush();`}/><button className="btn btn-secondary" disabled={busy} onClick={()=>void refresh()}>檢查收據</button><div className="receipt-result" role="status">{receipt?.expired?"測試碼已過期，請建立新的測試。":receipt?.receipt?<><strong>已收到測試資料</strong><p>來源：{receipt.receipt.origin||"未提供 Origin（例如同源或伺服器請求）"} · {receipt.receipt.events.length} 筆事件 · 正式分析未寫入</p><pre aria-label="實際安裝收據">{JSON.stringify(receipt.receipt,null,2)}</pre></>:"尚未收到這次測試的資料。"}</div></div>}
    {error&&<ErrorBanner message={error}/>}<details className="signal-details"><summary>沒有收到時，依序檢查</summary><ol><li>在網站 Console 查看 <code>cdp.status()</code>：是否等待同意、DNT、重複 key 或網路失敗。</li><li>確認網站 origin 在允許清單，站點仍啟用；CSP 的 script-src / connect-src 允許 CDP 網址。</li><li>Network 應先有 tracker-config，允許採集後才有 collect。4xx 不重試；429／5xx／網路錯誤最多三次。</li><li>Headless／bot User-Agent 正式資料會略過；測試收據會標記 bot。不要用舊的「最後收到事件」時間判定這次安裝。</li></ol></details>
  </section>;
}
export function IntegrationWorkbench({site}:{site:Site}) {
  const [framework,setFramework]=useState<Framework>("HTML");
  const [policy,setPolicy]=useState<CollectionPolicy>(site.collection_policy||legacyPolicy);
  const {meta}=useApp();const admin=useUser().role==="admin";
  const origin=(meta?.public_url||window.location.origin).replace(/\/$/,"");
  useEffect(()=>setPolicy(site.collection_policy||legacyPolicy),[site]);
  return <div className="signal-workbench">
    <section className="signal-hero"><div><p className="signal-kicker">LITE CDP / SIGNAL CONSOLE</p><h2>一段程式。<br/><em>每個訊號都有來處。</em></h2><p>先決定收什麼，再看清楚它如何成為一個人物的旅程。</p></div><div className="signal-hero-index"><span><b>01</b> 選擇採集政策</span><span><b>02</b> 演練身分匹配</span><span><b>03</b> 安裝單一程式</span><span><b>04</b> 核對真實收據</span></div><div className="signal-status"><span className={site.last_event_at?"status-dot":"status-dot idle"}/>{site.last_event_at?`已收到資料 · 最近一筆 ${timeAgo(site.last_event_at)}`:"尚未收到正式資料"}<small>本站 key：{site.key.slice(0,8)}…</small></div></section>
    {admin?<PolicyEditor site={site} policy={policy} onChange={setPolicy}/>:<div className="signal-policy-summary"><ShieldCheck size={20}/><span>目前政策：{policy.consent_required?"要求同意":"舊版自動開始"} · query {policy.query_mode} · SPA {policy.spa_mode}。採集設定由管理員調整。</span></div>}
    <IdentitySimulator policy={policy}/>
    <section className="signal-panel integration-panel"><div className="signal-section-heading"><div><p className="signal-kicker">03 / ONE LOADER</p><h2>用你的框架，裝一次就好。</h2></div><Terminal size={24}/></div><div role="tablist" aria-label="網站框架" className="framework-tabs">{frameworks.map(name=><button role="tab" aria-selected={framework===name} aria-controls="framework-example" key={name} onClick={()=>setFramework(name)}>{name}</button>)}</div><div id="framework-example" role="tabpanel" aria-label={framework}><Code label="追蹤程式碼" code={installationTemplate(framework,origin,site.key)}/><p className="integration-note">{frameworkNotes[framework]}</p></div><div className="integration-rules"><p><b>SPA / {policy.spa_mode}</b> {policy.spa_mode==="manual"?"請在 router 完成且標題更新後呼叫 cdp.page()；關閉自動頁面瀏覽可避免初始頁重複。":"自動監聽已啟用的 router 模式；不要再加 router hook 呼叫 page()，以免重複。"}</p><p><b>同意與登入</b> 載入 script 可以先讀取公開設定。未同意時不建立追蹤 ID。登入後明確提供 user_id；登出輪替所有已開分頁的匿名識別。</p><p><b>單例</b> 相同 key 再次載入安全略過；不同 key 或 endpoint 會在 Console 與 <code>cdp.status()</code> 顯示衝突。</p></div><details className="signal-details" open><summary>串接 CMP、登入、登出與自訂事件</summary><Code label="生命週期範例" code={lifecycleTemplate}/><p>在 consent grant 後再呼叫 identify。任意 props 是網站主動提供的自訂事件資料；請不要放密碼、卡號或不需要的個人資訊。用 <code>data-cdp-ignore</code> 排除區塊點擊，不能當作自訂 props 遮罩。</p></details><div className="integration-reference"><a href="https://nextjs.org/docs/app/guides/scripts" target="_blank" rel="noreferrer">Next Script <ArrowUpRight size={16}/></a><a href="https://react.dev/reference/react/StrictMode" target="_blank" rel="noreferrer">React StrictMode <ArrowUpRight size={16}/></a><a href="https://nuxt.com/docs/4.x/directory-structure/app/plugins" target="_blank" rel="noreferrer">Nuxt Plugins <ArrowUpRight size={16}/></a><a href="https://support.google.com/tagmanager/answer/10718549" target="_blank" rel="noreferrer">GTM Consent <ArrowUpRight size={16}/></a></div></section>
    {admin&&<Receipt site={site} origin={origin} framework={framework}/>}
    <section className="signal-data-note"><h3>資料界線，是產品的一部分。</h3><p>不採集 IP 資料庫紀錄、canvas／字型指紋、整份 DOM 或表單輸入。User-Agent 用於瀏覽器類型與 bot 過濾。事件時間來自瀏覽器，另存伺服器收到時間；舊版事件只有收到時間。{meta?.retention_days?`事件保留 ${meta.retention_days} 天；人物資料另行管理。`:"目前未設定事件自動刪除；管理員可設定 CDP_RETENTION_DAYS。"}</p></section>
  </div>;
}
