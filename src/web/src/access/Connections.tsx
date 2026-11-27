import { Check, Copy, KeyRound, Link, ShieldCheck, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { accessRequest } from "./api";
import { providerErrorMessage } from "./navigation";
import type { AuditEvent, Client, Credential, Overview, Provider, Transport, Usage } from "./types";
import "./access.css";

const date = (value: string | null) => value ? value.replace("T", " ").replace("Z", " UTC") : "尚無紀錄";
const status = (item: { revoked_at: string | null; expires_at: string }) => item.revoked_at ? "已撤銷" : Date.parse(item.expires_at) <= Date.now() ? "已到期" : "有效";

function Secret({ token, onClose }: { token: string; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current?.querySelector<HTMLInputElement>("input")?.focus();
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); onClose(); }
      if (event.key === "Tab") {
        const items = ref.current?.querySelectorAll<HTMLElement>("input,button");
        if (!items?.length) return;
        const first = items[0], last = items[items.length - 1];
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
      }
    };
    window.addEventListener("keydown", key);
    return () => { window.removeEventListener("keydown", key); if (previous?.isConnected && previous.tabIndex >= 0 && !previous.matches(":disabled")) previous.focus(); else document.getElementById("access-key-name")?.focus(); };
  }, [onClose]);
  const copy = async () => {
    try { await navigator.clipboard.writeText(token); setCopied(true); }
    catch { setError("無法存取剪貼簿，請選取並複製金鑰原值。"); }
  };
  return <div className="access-secret-overlay"><div ref={ref} className="access-secret" role="dialog" aria-modal="true" aria-label="一次性 API 金鑰">
    <h2>一次性 API 金鑰</h2><p>原值只在這次建立後顯示。關閉後無法取回，請存入安全的位置。</p>
    <label className="label" htmlFor="access-secret">金鑰原值</label><input id="access-secret" className="input" value={token} readOnly autoComplete="off" spellCheck={false}/>
    {error && <p role="alert">{error}</p>}
    <div className="access-actions"><button className="btn btn-secondary" onClick={() => void copy()}>{copied ? <Check size={16}/> : <Copy size={16}/>} {copied ? "已複製" : "複製金鑰"}</button><button className="btn btn-primary" onClick={onClose}><X size={16}/>關閉並清除金鑰原值</button></div>
  </div></div>;
}

export function Connections({ isAdmin }: { isAdmin: boolean }) {
  const generation = useRef(0);
  const auditGeneration = useRef(0);
  const [overview, setOverview] = useState<Overview | null>(null);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [usage, setUsage] = useState<Usage | null>(null);
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [clients, setClients] = useState<Client[]>([]);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState("");
  const [expires, setExpires] = useState(30);
  const [audiences, setAudiences] = useState<Transport[]>(["api", "mcp"]);
  const [people, setPeople] = useState(false);
  const [secret, setSecret] = useState<string | null>(null);
  const [days, setDays] = useState(7);
  const [transport, setTransport] = useState("");
  const [outcome, setOutcome] = useState("");
  const [auditUser, setAuditUser] = useState("self");
  const [auditMode, setAuditMode] = useState("self");
  const [userDraft, setUserDraft] = useState("");
  const [clientName, setClientName] = useState("");
  const [redirects, setRedirects] = useState("");
  const closeSecret = useCallback(() => setSecret(null), []);
  const query = useCallback((event = false, cursor?: string) => {
    const q = new URLSearchParams(event ? { limit: "25" } : { days: String(days) });
    if (transport) q.set("transport", transport);
    if (event && outcome) q.set("outcome", outcome);
    if (isAdmin && auditUser !== "self") q.set("user_id", auditUser);
    if (cursor) q.set("cursor", cursor);
    return q.toString();
  }, [days, transport, outcome, isAdmin, auditUser]);
  const load = useCallback(async () => {
    const request = ++generation.current;
    setError("");
    const [o, p, c] = await Promise.all([
      accessRequest<Overview>("/overview"), accessRequest<{ items: Provider[] }>("/providers"),
      isAdmin ? accessRequest<{ items: Client[] }>("/clients") : Promise.resolve({ items: [] as Client[] }),
    ]);
    if (request !== generation.current) return;
    setOverview(o); setProviders(p.items); setClients(c.items);
  }, [isAdmin]);
  const loadAudit = useCallback(async () => {
    const request = ++auditGeneration.current;
    setError("");
    const [u, e] = await Promise.all([
      accessRequest<Usage>(`/usage?${query()}`), accessRequest<{ items: AuditEvent[]; next_cursor: string | null }>(`/events?${query(true)}`),
    ]);
    if (request !== auditGeneration.current) return;
    setUsage(u); setEvents(e.items); setNextCursor(e.next_cursor);
  }, [query]);
  useEffect(() => { let current = true; load().catch(e => { if (current) setError(e.message); }); return () => { current = false; generation.current += 1; }; }, [load]);
  useEffect(() => { let current = true; loadAudit().catch(e => { if (current) setError(e.message); }); return () => { current = false; auditGeneration.current += 1; }; }, [loadAudit]);
  const perform = async (work: () => Promise<void>, refresh: false | "overview" | "clients" | "audit" | "all" = "overview") => {
    setError(""); setNotice(""); setBusy(true);
    try {
      await work();
      if (refresh === "overview") setOverview(await accessRequest<Overview>("/overview"));
      if (refresh === "clients") setClients((await accessRequest<{ items: Client[] }>("/clients")).items);
      if (refresh === "audit") await loadAudit();
      if (refresh === "all") await Promise.all([load(), loadAudit()]);
    } catch (e) { setError(e instanceof Error ? e.message : "請求未完成"); }
    finally { setBusy(false); }
  };
  const createKey = () => perform(async () => {
    const created = await accessRequest<{ credential: Credential; token: string }>("/credentials", "POST", { name: name.trim(), scopes: people ? ["reports:read", "people:read"] : ["reports:read"], audiences, expires_in_days: expires });
    setSecret(created.token); setName("");
  });
  const bind = (provider: Provider) => perform(async () => {
    const flow = await accessRequest<{ authorization_url: string }>(`/identities/${provider.provider}/link`, "POST", {});
    window.location.assign(flow.authorization_url);
  }, false);
  const more = () => perform(async () => {
    if (!nextCursor) return;
    const request = auditGeneration.current;
    const page = await accessRequest<{ items: AuditEvent[]; next_cursor: string | null }>(`/events?${query(true, nextCursor)}`);
    if (request !== auditGeneration.current) return;
    setEvents(items => [...items, ...page.items]); setNextCursor(page.next_cursor);
  }, false);
  const register = () => perform(async () => {
    const client = await accessRequest<Client>("/clients", "POST", { client_name: clientName.trim(), redirect_uris: redirects.split("\n").map(v => v.trim()).filter(Boolean), token_endpoint_auth_method: "none", grant_types: ["authorization_code", "refresh_token"], response_types: ["code"] });
    setNotice(`已註冊 ${client.client_name} · ${client.client_id}`); setClientName(""); setRedirects("");
  }, "clients");
  return <div className="access-workspace">
    <header className="access-heading"><p className="access-rubric">IDENTITY / PERMISSION / FOOTPRINTS</p><h1>連線與存取</h1><p>為資料地圖標記授權範圍，沿著真實請求核對工具足跡。</p></header>
    {providerErrorMessage(window.location.search) && <p className="access-error" role="alert">{providerErrorMessage(window.location.search)}</p>}
    {error && <p className="access-error" role="alert">{error}</p>}{notice && <p role="status">{notice}</p>}
    {!overview ? (error ? <button className="btn btn-secondary" onClick={() => void perform(async () => {}, "all")}>重新載入</button> : <p role="status">正在載入連線紀錄…</p>) : <>
      <section className="access-section" aria-labelledby="access-identities"><header><p className="access-rubric">01 / 帳號綁定</p><h2 id="access-identities"><Link size={20}/>保留原帳號，增加登入方式</h2><p>只綁定目前本人。解除後仍可使用原密碼登入，不依相同 email 合併帳號。</p></header><div>
        <div className="access-provider-grid">{providers.map(provider => {
          const identity = overview.identities.find(item => item.provider === provider.provider);
          return <div className="access-provider" key={provider.provider}><h3>{provider.name}</h3>{identity ? <><p>已綁定 · {identity.subject_mask}</p><p className="access-meta">綁定 {date(identity.linked_at)}<br/>最後登入 {date(identity.last_login_at)}</p><button className="btn btn-secondary" disabled={busy} onClick={() => void perform(() => accessRequest<void>(`/identities/${encodeURIComponent(identity.id)}`, "DELETE"))}>解除 {provider.name} 綁定</button></> : <><p>{provider.enabled ? "尚未綁定" : provider.disabled_reason ?? "尚未設定登入供應商"}</p><button className="btn btn-secondary" disabled={busy || !provider.enabled} onClick={() => void bind(provider)}>綁定 {provider.name}</button></>}</div>;
        })}</div>
      </div></section>
      <section className="access-section" aria-labelledby="access-credentials"><header><p className="access-rubric">02 / API 金鑰</p><h2 id="access-credentials"><KeyRound size={20}/>把權限交給指定工具</h2><p>金鑰承接目前帳號的資料權限。停用帳號、移除資料權限或撤銷金鑰後，下一次請求即拒絕。</p></header><div>
        <form className="access-form" onSubmit={e => { e.preventDefault(); void createKey(); }}>
          <label className="label">金鑰名稱<input id="access-key-name" className="input" value={name} onChange={e => setName(e.target.value)} maxLength={80} required placeholder="例如：研究終端"/></label>
          <label className="label">有效天數<input className="input" type="number" min={1} max={90} required value={expires} onChange={e => setExpires(Number(e.target.value))}/></label>
          <fieldset><legend>允許的通道</legend><div className="access-actions">{([ ["api", "一般 API"], ["mcp", "MCP 工具"] ] as const).map(([value, label]) => <label key={value}><input type="checkbox" checked={audiences.includes(value)} onChange={e => setAudiences(items => e.target.checked ? [...items, value] : items.filter(item => item !== value))}/> {label}</label>)}</div></fieldset>
          <p>基本範圍 reports:read：站台、流量、頁面與分群摘要；不含追蹤金鑰、已識別訪客或分群條件。</p><label className="access-sensitive"><input type="checkbox" checked={people} onChange={e => setPeople(e.target.checked)}/>另允許 people:read：讀取個人的完整事件、網址與事件屬性</label>
          <button className="btn btn-primary" disabled={busy || !name.trim() || !audiences.length || expires < 1 || expires > 90}>建立 API 金鑰</button>
        </form>
        {!overview.credentials.length ? <p className="access-empty">尚未建立 API 金鑰。</p> : <ul className="access-ledger">{overview.credentials.map(item => <li key={item.id}><div><h3>{item.name} <span className="access-meta">{status(item)}</span></h3><p className="access-wrap">{item.prefix} · {item.audiences.join(" / ")} · {item.scopes.join("、")}</p><p className="access-meta">到期 {date(item.expires_at)}<br/>最後使用 {date(item.last_used_at)}</p></div><button className="btn btn-secondary" disabled={busy || !!item.revoked_at} aria-label={`撤銷金鑰 ${item.name}`} onClick={() => void perform(() => accessRequest<void>(`/credentials/${encodeURIComponent(item.id)}`, "DELETE"))}>撤銷</button></li>)}</ul>}
      </div></section>
      <section className="access-section" aria-labelledby="access-grants"><header><p className="access-rubric">03 / 應用授權</p><h2 id="access-grants"><ShieldCheck size={20}/>讓應用使用限定資源</h2><p>每份 OAuth 授權限定一個資源；撤銷會一併停用該授權的 access 與 refresh token。</p></header><div>
        {!overview.grants.length ? <p className="access-empty">目前沒有應用授權。</p> : <ul className="access-ledger">{overview.grants.map(item => <li key={item.id}><div><h3>{item.client_name} <span className="access-meta">{status(item)}</span></h3><p>{item.scopes.join("、")}</p><p className="access-wrap">{item.resource}</p><p className="access-meta">到期 {date(item.expires_at)}<br/>最後使用 {date(item.last_used_at)}</p></div><button className="btn btn-secondary" disabled={busy || !!item.revoked_at} aria-label={`撤銷授權 ${item.client_name}`} onClick={() => void perform(() => accessRequest<void>(`/grants/${encodeURIComponent(item.id)}`, "DELETE"))}>撤銷</button></li>)}</ul>}
      </div></section>
      <section className="access-section access-audit" aria-labelledby="access-usage"><header><p className="access-rubric">04 / 用量與稽核</p><h2 id="access-usage">從請求核對使用情況</h2><p>每次一般 API 操作或 MCP 工具執行算一次。協定探索、列工具與原有匯入不混入；日期與分日統計採 UTC。</p></header><div>
        <div className="access-filters"><label>範圍<select className="input" value={days} onChange={e => setDays(Number(e.target.value))}><option value={7}>最近 7 天</option><option value={30}>最近 30 天</option><option value={90}>最近 90 天</option></select></label><label>通道<select className="input" value={transport} onChange={e => setTransport(e.target.value)}><option value="">全部通道</option><option value="api">一般 API</option><option value="mcp">MCP 工具</option></select></label>{isAdmin && <div className="access-filters"><label>查看帳號<select className="input" value={auditMode} onChange={e => { setAuditMode(e.target.value); if (e.target.value !== "user") setAuditUser(e.target.value); }}><option value="self">僅本人</option><option value="all">所有帳號</option><option value="user">指定帳號</option></select></label>{auditMode === "user" && <><label>帳號 ID<input className="input" inputMode="numeric" value={userDraft} onChange={e => setUserDraft(e.target.value)} maxLength={18}/></label><button className="btn btn-secondary" type="button" disabled={!/^[1-9][0-9]{0,17}$/.test(userDraft)} onClick={() => setAuditUser(userDraft)}>查看此帳號</button></>}</div>}</div>
        {usage && <><p className="access-meta">{usage.from.slice(0, 10)} – {usage.to.slice(0, 10)}（不含迄日） · UTC</p><div className="access-totals"><strong>嘗試 {usage.totals.attempts}</strong><span>成功 {usage.totals.successes}</span><span>失敗 {usage.totals.failures}</span><span>待完成 {usage.totals.pending}</span></div>{usage.totals.attempts === 0 && <p className="access-empty">這段期間尚無工具執行，統計為 0。</p>}{!!usage.daily.length && <div className="access-table-scroll" tabIndex={0} role="region" aria-label="UTC 分日用量"><table><thead><tr><th>UTC 日期</th><th>嘗試</th><th>成功</th><th>失敗</th><th>待完成</th></tr></thead><tbody>{usage.daily.map(day => <tr key={day.date}><td>{day.date}</td><td>{day.attempts}</td><td>{day.successes}</td><td>{day.failures}</td><td>{day.pending}</td></tr>)}</tbody></table></div>}</>}
        <div className="access-log-heading"><h3>近期請求紀錄</h3><button className="btn btn-secondary" disabled={busy} onClick={() => void perform(async () => {}, "audit")}>更新稽核紀錄</button><label>結果<select className="input" value={outcome} onChange={e => setOutcome(e.target.value)}><option value="">全部結果</option><option value="pending">待完成</option><option value="success">成功</option><option value="failure">失敗</option><option value="interrupted">中斷</option></select></label></div>
        {!events.length ? <p className="access-empty">沒有符合條件的請求紀錄。</p> : <div className="access-table-scroll" tabIndex={0} role="region" aria-label="請求稽核紀錄"><table><thead><tr><th>時間（UTC）</th><th>通道 / 操作</th><th>結果 / 狀態</th><th>耗時</th><th>追蹤編號</th></tr></thead><tbody>{events.map(event => <tr key={event.id}><td>{date(event.at)}</td><td>{event.transport ?? event.kind}<br/>{event.operation}</td><td>{event.outcome} / {event.status ?? "待完成"}</td><td>{event.duration_ms === null ? "—" : `${event.duration_ms} ms`}</td><td className="access-wrap">{event.request_id}</td></tr>)}</tbody></table></div>}
        {nextCursor && <button className="btn btn-secondary" disabled={busy} onClick={() => void more()}>載入更多紀錄</button>}
      </div></section>
      {isAdmin && <section className="access-section" aria-labelledby="access-clients"><header><p className="access-rubric">ADMIN / 應用註冊</p><h2 id="access-clients">登記 OAuth 公開應用</h2><p>只支援 Authorization Code + PKCE。回呼網址必須精確符合登記值；不能用金鑰管理帳號或應用。</p></header><div><form className="access-form" onSubmit={e => { e.preventDefault(); void register(); }}><label className="label">應用名稱<input className="input" maxLength={80} required value={clientName} onChange={e => setClientName(e.target.value)}/></label><label className="label">回呼網址（每行一個）<textarea className="input" required value={redirects} onChange={e => setRedirects(e.target.value)} rows={3}/></label><button className="btn btn-primary" disabled={busy || !clientName.trim() || !redirects.trim()}>註冊 OAuth 應用</button></form><ul className="access-ledger">{clients.map(client => <li key={client.client_id}><div><h3>{client.client_name}</h3><p className="access-wrap">{client.client_id}</p>{client.redirect_uris.map(uri => <p className="access-wrap access-meta" key={uri}>{uri}</p>)}</div></li>)}</ul></div></section>}
    </>}
    {secret && <Secret token={secret} onClose={closeSecret}/>}
  </div>;
}
