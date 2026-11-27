import { useCallback, useEffect, useRef, useState } from "react";
import { ApiError } from "../api";
import { ErrorBanner } from "../components/ui";
import { registrationRequest, type Mode, type Page, type Policy, type Registration, type RegistrationEvent, type Status } from "./api";
import "./registration.css";
const modes: Record<Mode, string> = { closed: "關閉", approval: "審核後開通", open: "開放" };
const statuses: Record<Status, string> = { pending: "待審核", active: "已開通", rejected: "已拒絕" };
const utc = (value: string) => `${value.replace("T", " ").replace(/\.\d+Z$/, "Z").replace("Z", "")} UTC`;
function eventText(event: RegistrationEvent) {
  if (event.action === "policy_changed") return `更改註冊方式：${modes[event.mode as Mode] ?? event.mode}`;
  return `${event.action === "registered" ? "提交申請" : event.action === "approved" ? "核准申請" : "拒絕申請"} #${event.registration_id}`;
}
function appendUnique<T extends { id: number }>(current: T[], incoming: T[]) { const ids = new Set(current.map(item => item.id)); return [...current, ...incoming.filter(item => !ids.has(item.id))]; }
export function RegistrationAdmin({ onReviewed }: { onReviewed?: () => void }) {
  const [policy, setPolicy] = useState<Policy | null>(null);
  const [mode, setMode] = useState<Mode>("closed");
  const [status, setStatus] = useState<Status>("pending");
  const [queue, setQueue] = useState<Page<Registration>>({ items: [], next_cursor: null });
  const [audit, setAudit] = useState<Page<RegistrationEvent>>({ items: [], next_cursor: null });
  const [reasons, setReasons] = useState<Record<number, string>>({});
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const version = useRef(0);
  const locked = useRef(false);
  const paging = useRef(false);
  const [loadingPage, setLoadingPage] = useState(false);
  const reload = useCallback(async () => {
    const request = ++version.current;
    setLoading(true); setError(""); setQueue({ items: [], next_cursor: null });
    try {
      const [p, q, a] = await Promise.all([
        registrationRequest<Policy>("registration-policy"),
        registrationRequest<Page<Registration>>(`registrations?status=${status}&limit=50`),
        registrationRequest<Page<RegistrationEvent>>("registration-events?limit=50"),
      ]);
      if (request !== version.current) return;
      setPolicy(p); setMode(p.mode); setQueue(q); setAudit(a);
    } catch (e) { if (request === version.current) setError((e as Error).message); }
    finally { if (request === version.current) setLoading(false); }
  }, [status]);
  useEffect(() => { void reload(); return () => { version.current++; }; }, [reload]);
  async function mutate(work: () => Promise<void>) {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError(""); setNotice("");
    try { await work(); }
    catch (e) {
      if (e instanceof ApiError && e.status === 409) { await reload(); setError("這筆申請已由其他管理員處理，清單已更新。"); }
      else setError((e as Error).message);
    } finally { locked.current = false; setBusy(false); }
  }
  async function review(item: Registration, decision: "approve" | "reject") {
    const reason = reasons[item.id]?.trim();
    if (decision === "reject" && !reason) return;
    await mutate(async () => {
      await registrationRequest<Registration>(`registrations/${item.id}/review`, "POST", { decision, ...(decision === "reject" ? { reason } : {}) });
      await reload(); onReviewed?.();
      setReasons(current => { const next = { ...current }; delete next[item.id]; return next; });
      setNotice(decision === "approve" ? `已核准 ${item.username}，現在可以登入。` : `已拒絕 ${item.username}。`);
    });
  }
  async function more(kind: "queue" | "audit") {
    const cursor = (kind === "queue" ? queue : audit).next_cursor;
    if (!cursor || paging.current) return;
    const request = version.current;
    paging.current = true; setLoadingPage(true); setError("");
    try {
      if (kind === "queue") {
        const page = await registrationRequest<Page<Registration>>(`registrations?status=${status}&limit=50&before=${cursor}`);
        if (request === version.current) setQueue(current => ({ items: appendUnique(current.items, page.items), next_cursor: page.next_cursor }));
      } else {
        const page = await registrationRequest<Page<RegistrationEvent>>(`registration-events?limit=50&before=${cursor}`);
        if (request === version.current) setAudit(current => ({ items: appendUnique(current.items, page.items), next_cursor: page.next_cursor }));
      }
    } catch (e) { if (request === version.current) setError((e as Error).message); }
    finally { paging.current = false; setLoadingPage(false); }
  }
  return <div className="registration-admin card" aria-busy={busy || loading}>
    <header><p className="registration-rubric">MEMBERSHIP DESK / 成員登記處</p><h2>註冊與審核</h2><p className="hint">控制新成員的加入方式，並保留每一次處理紀錄。新申請一律建立為一般成員。</p></header>
    {error && <ErrorBanner message={error} />}{notice && <p role="status" className="registration-notice">{notice}</p>}
    <form className="registration-controls" onSubmit={e => { e.preventDefault(); void mutate(async () => { const saved = await registrationRequest<Policy>("registration-policy", "PATCH", { mode }); setPolicy(saved); await reload(); setNotice("註冊方式已更新。"); }); }}>
      <label htmlFor="registration-mode">註冊方式<select id="registration-mode" aria-label="註冊方式" className="input" disabled={!policy || busy || loading} value={mode} onChange={e => setMode(e.target.value as Mode)}>{Object.entries(modes).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label>
      <button className="btn btn-primary" disabled={!policy || busy || loading || mode === policy.mode}>儲存註冊方式</button>
    </form><p className="hint mt-3">關閉：停止新申請。審核後開通：管理員核准後才能登入。開放：註冊後立即開通。變更不影響既有成員。</p>
    <section aria-labelledby="registration-queue-heading"><h3 id="registration-queue-heading">成員申請</h3><div className="registration-controls"><label htmlFor="registration-status">申請狀態<select id="registration-status" aria-label="申請狀態" className="input" disabled={busy} value={status} onChange={e => { setNotice(""); setStatus(e.target.value as Status); }}>{Object.entries(statuses).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label><button className="btn btn-secondary" disabled={busy || loading} onClick={() => void reload()}>更新申請與紀錄</button></div>
      {loading ? <p role="status" className="registration-meta">正在載入申請…</p> : !queue.items.length ? <p className="registration-meta mt-4">沒有符合條件的申請。</p> : <ul className="registration-ledger">{queue.items.map(item => <li key={item.id}><div className="registration-record-header"><div><h3>{item.display_name}</h3><p>{item.username} <span className="registration-meta">#{item.id}・{item.provider}・{statuses[item.status]}</span></p></div><p className="registration-meta">申請於 {utc(item.created_at)}</p></div>{item.reviewed_at && <p className="registration-meta">處理於 {utc(item.reviewed_at)}・管理員 #{item.reviewed_by}</p>}{item.rejection_reason && <p>拒絕原因：{item.rejection_reason}</p>}{item.status === "pending" && <div className="registration-reason"><label htmlFor={`registration-reason-${item.id}`}>{item.username} 的拒絕原因</label><textarea id={`registration-reason-${item.id}`} className="input" maxLength={500} value={reasons[item.id] ?? ""} placeholder="拒絕時必填，最多 500 個字元" disabled={busy} onChange={e => setReasons(current => ({ ...current, [item.id]: e.target.value }))} /><div className="registration-controls"><button className="btn btn-primary" aria-label={`核准 ${item.username}`} disabled={busy || loading} onClick={() => void review(item, "approve")}>核准</button><button className="btn btn-secondary" aria-label={`拒絕 ${item.username}`} disabled={busy || loading || !reasons[item.id]?.trim()} onClick={() => void review(item, "reject")}>拒絕</button></div></div>}</li>)}</ul>}
      {queue.next_cursor && <button className="btn btn-secondary" disabled={busy || loading || loadingPage} onClick={() => void more("queue")}>載入更多申請</button>}
    </section>
    <section aria-labelledby="registration-audit-heading"><h3 id="registration-audit-heading">註冊處理紀錄</h3>{!audit.items.length && <p className="registration-meta mt-4">尚無註冊處理紀錄。</p>}<ul className="registration-ledger">{audit.items.map(event => <li key={event.id}><p>{eventText(event)}</p><p className="registration-meta">{utc(event.created_at)}・{event.actor_username ?? (event.actor_id ? `帳號 #${event.actor_id}` : "註冊申請者")}</p>{event.reason && <p>{event.reason}</p>}</li>)}</ul>{audit.next_cursor && <button className="btn btn-secondary" disabled={busy || loading || loadingPage} onClick={() => void more("audit")}>載入更多註冊紀錄</button>}</section>
  </div>;
}
