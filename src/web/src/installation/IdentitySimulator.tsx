import { useState } from "react";
import { ArrowRight, Fingerprint, Laptop, RotateCcw, UserRound } from "lucide-react";
import { type CollectionPolicy, previewUrl } from "./policy";

type Device = {anon:string;label:string;user:string|null};
export function IdentitySimulator({policy}:{policy:CollectionPolicy}) {
  const [devices,setDevices] = useState<Device[]>([{anon:"demo-a-001",label:"裝置 A",user:null}]);
  const [current,setCurrent] = useState(0);
  const [allowed,setAllowed] = useState(true);
  const [rawUrl,setRawUrl] = useState("https://shop.example/pricing?email=private@example.com&campaign=spring#details");
  const [log,setLog] = useState(["裝置 A 建立隨機匿名編號，記錄 /pricing。"]);
  const [action,setAction] = useState("pageview");
  const device=devices[current];
  const people=new Set(devices.map(d=>d.user || d.anon)).size;
  const append=(text:string)=>setLog(old=>[...old.slice(-4),text]);
  function another() {
    if (!allowed) {append("尚未同意：新裝置不建立 ID 或送出資料。");return;}
    if (devices.length>=8) return;
    const index=devices.length;
    setDevices(old=>[...old,{anon:`demo-${index+1}-001`,label:`裝置 ${String.fromCharCode(65+index)}`,user:null}]);setCurrent(index);setAction("pageview");append("另一個瀏覽器有不同儲存空間，先建立獨立匿名人物。");
  }
  function identify() {
    if(!allowed){append("尚未同意：identify 被捨棄，不建立 ID 或送出資料。");return;}
    setDevices(old=>old.map((d,i)=>i===current?{...d,user:"member-42"}:d));setAction("identify");append(`${device.label} 明確送出 user_id=member-42；只合併本站相同會員 ID。`);
  }
  function toggleConsent() {
    if (allowed) {setAllowed(false);append("撤回同意：清除目前儲存與佇列，停止採集；歷史人物仍保留。");}
    else {const index=devices.length;setDevices(old=>[...old,{anon:`demo-consent-${index}`,label:"重新同意 · 新匿名",user:null}]);setCurrent(index);setAllowed(true);setAction("pageview");append("重新同意才建立新匿名 ID；不回補拒絕期間的事件，也不自動重新登入。");}
  }
  function reset(reason:string) {
    if(!allowed)return;
    if(devices.length>=8)return;
    const index=devices.length;setDevices(old=>[...old,{anon:`demo-reset-${index}`,label:`${device.label} · 新匿名`,user:null}]);setCurrent(index);setAction("pageview");append(`${reason}：輪替匿名與 session ID；舊人物紀錄保留。`);
  }
  let url="";try{url=previewUrl(rawUrl,policy);}catch{/* invalid input is visible below */}
  const payload=allowed && url?{key:"示範網站",anon_id:device.anon,session_id:`demo-session-${current+1}`,consent:true,...(action==="identify"?{identify:{user_id:"member-42"}}:{events:[{event_id:"demo-event-001",type:"pageview",url}]})}:null;
  return <section className="signal-panel identity-lab" aria-labelledby="identity-lab-title">
    <div className="signal-section-heading"><div><p className="signal-kicker">02 / IDENTITY LAB</p><h2 id="identity-lab-title">看見身分如何接起來。</h2></div><span className="signal-chip">示範資料 · 不會送出</span></div>
    <p className="signal-intro">先讓裝置 A 登入，再新增裝置 B 並登入同一會員。編號來自隨機數；瀏覽器特徵、IP 和 email 相似程度都不會推算身分。</p>
    <div className="sim-actions">
      <button className="btn btn-secondary" onClick={()=>{setAction("pageview");append(allowed?"再次造訪沿用目前匿名編號。":"尚未同意：不記錄這次瀏覽。");}}>再次造訪</button>
      <button className="btn btn-secondary" disabled={devices.length>=8} onClick={another}>另一台裝置</button>
      <button className="btn btn-primary" onClick={identify}>登入同一會員</button>
      <button className="btn btn-secondary" onClick={()=>reset("登出")}>登出</button>
      <button className="btn btn-secondary" onClick={()=>reset("清除儲存")}>清除儲存</button>
      <button className="btn btn-secondary" onClick={toggleConsent}>{allowed?"拒絕同意":"同意採集"}</button>
      <button className="btn btn-secondary" onClick={()=>append("相同 key 重複安裝：重用單例，不多送 pageview。")}>重複安裝</button>
      <button className="btn btn-secondary" onClick={()=>append("不同 key／endpoint：conflicting_installation，保留原本目的地。")}>衝突安裝</button>
      <button className="btn btn-ghost" aria-label="重設模擬" onClick={()=>{setDevices([{anon:"demo-a-001",label:"裝置 A",user:null}]);setCurrent(0);setAllowed(true);setAction("pageview");setLog(["模擬已重設。"]);}}><RotateCcw size={16}/></button>
    </div>
    <div className="identity-pipeline">
      <div className="pipeline-stage"><span className="signal-stage-number">01</span><h3><Laptop size={20}/> 輸入</h3><div className="sim-devices">{devices.map((d,i)=><button key={d.anon} aria-pressed={i===current} onClick={()=>setCurrent(i)}><strong>{d.label}</strong><code>{d.user || d.anon}</code></button>)}</div><label className="label" htmlFor="sim-url">原始網址</label><input id="sim-url" className="input" value={rawUrl} onChange={e=>setRawUrl(e.target.value)}/><small>localStorage：同 origin、同站。sessionStorage：分頁生命週期。</small></div>
      <div className="pipeline-stage"><span className="signal-stage-number">02</span><h3><ArrowRight size={20}/> 送出前</h3><pre aria-label="模擬 payload">{payload?JSON.stringify(payload,null,2):allowed?"請輸入合法的 http(s) 網址。":"同意未授予\n沒有 ID · 沒有事件 · 沒有傳送"}</pre><small>query/hash 依設定清理；會員 traits 僅收允許清單。</small></div>
      <div className="pipeline-stage identity-result"><span className="signal-stage-number">03</span><h3><UserRound size={20}/> 確定式匹配</h3><div className="identity-count"><b aria-label="模擬人物數">{people}</b><span>個歷史人物<br/>{devices.length} 個瀏覽器編號</span></div><svg viewBox="0 0 270 110" role="img" aria-label="匿名紀錄只在會員識別一致時合流"><path d={people===1&&devices.length>1?"M20 25H95L160 55H250 M20 85H95L160 55":"M20 25H250 M20 85H250"} fill="none" stroke="currentColor" strokeWidth="2"/>{[25,85].map(y=><circle key={y} cx="20" cy={y} r="6" fill="currentColor"/>)}<rect x="156" y="39" width="98" height="32" fill="var(--signal-surface)" stroke="currentColor"/><text x="205" y="59" textAnchor="middle" fontSize="11" fill="currentColor">{device.user?"member-42":"anonymous"}</text></svg><p>{device.user?`以 user_id ${device.user} 對應本站人物。不同 user_id 不會因 email 相同被合併。`:"目前未知：維持獨立人物。不同裝置不會自動相認。"}</p><span className="signal-chip"><Fingerprint size={16}/> 不使用瀏覽器指紋</span></div>
    </div>
    <ol className="sim-log" aria-live="polite">{log.map((item,i)=><li key={`${i}-${item}`}><span>{String(i+1).padStart(2,"0")}</span>{item}</li>)}</ol>
  </section>;
}
