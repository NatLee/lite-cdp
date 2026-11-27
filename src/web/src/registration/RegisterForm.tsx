import { useEffect, useRef, useState } from "react";
import { Link, Navigate, useLocation } from "react-router";
import { api } from "../api";
import { ErrorBanner, Spinner } from "../components/ui";
import { useApp } from "../context";
import { continuation, registrationRequest, safeRegistrationNext, type Policy } from "./api";
import "./registration.css";
const providerName: Record<string, string> = { google: "Google", github: "GitHub" };
export function RegistrationLoginLink() {
  const location = useLocation();
  return <p className="registration-login-link"><Link to={`/register${continuation(location.search)}`}>註冊帳號</Link><span>・忘記密碼請聯絡管理員</span></p>;
}
export function RegistrationNotice() {
  const location = useLocation();
  return new URLSearchParams(location.search).get("registration") === "pending" ? <p role="status" className="registration-notice">申請已送出，等待管理員審核。核准後才能登入工作區。</p> : null;
}
export function RegisterForm() {
  const { setUser } = useApp();
  const location = useLocation();
  const [policy, setPolicy] = useState<Policy | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const locked = useRef(false);
  const [pending, setPending] = useState(false);
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  useEffect(() => {
    let current = true;
    registrationRequest<Policy>("registration-policy").then(value => { if (current) setPolicy(value); }).catch(e => { if (current) setError((e as Error).message); });
    return () => { current = false; };
  }, []);
  async function submit() {
    if (locked.current) return;
    setError("");
    if (!/^[a-zA-Z0-9._-]{3,64}$/.test(username.trim())) { setError("帳號需為 3 到 64 個英數字或 . _ -"); return; }
    if ([...password].length < 12 || [...password].length > 256) { setError("密碼需為 12 到 256 個字元"); return; }
    if ([...displayName.trim()].length > 100) { setError("顯示名稱最多 100 個字元"); return; }
    if (password !== confirmation) { setError("兩次輸入的密碼不一致"); return; }
    locked.current = true;
    setBusy(true);
    try {
      const result = await registrationRequest<{ status: "pending" | "active"; user_id: number }>("register", "POST", { username: username.trim(), password, ...(displayName.trim() ? { display_name: displayName.trim() } : {}) });
      setPassword(""); setConfirmation("");
      if (result.status === "pending") setPending(true);
      else {
        const state = await api.authState();
        if (!state.user) throw new Error("帳號已建立，請返回登入。");
        setUser(state.user);
      }
    } catch (e) { setError((e as Error).message); }
    finally { locked.current = false; setBusy(false); }
  }
  if (policy?.setup_required) return <Navigate to="/first-login" replace />;
  const next = safeRegistrationNext(location.search);
  return <form className="login-form registration-form w-full max-w-md" aria-busy={busy} onSubmit={e => { e.preventDefault(); void submit(); }}>
    <p className="registration-rubric">MEMBERSHIP / 成員登記</p>
    <h1>{pending ? "申請已送出，等待審核" : "註冊帳號"}</h1>
    {error && <ErrorBanner message={error} />}
    {!policy && !error && <p role="status">正在確認註冊方式…</p>}
    {pending ? <div role="status" className="registration-notice"><p>管理員核准後，即可使用剛才建立的帳號登入。</p><p>審核完成前，無法使用工作區、API 或 MCP。</p></div> : policy && <>
      {policy.mode === "closed" ? <div className="registration-notice"><h2>目前未開放註冊</h2><p>請聯絡管理員建立帳號，或等候開放後再申請。</p></div> : <>
        <p className="registration-description">{policy.mode === "approval" ? "送出後由管理員審核，核准後即可登入。" : "建立帳號後，即可進入工作區。"}</p>
        {policy.password_enabled && <>
          <label className="label" htmlFor="register-username">帳號</label><input id="register-username" className="input" autoComplete="username" maxLength={64} value={username} onChange={e => setUsername(e.target.value)} required />
          <p className="hint">3 到 64 個英數字或 . _ -</p>
          <label className="label mt-3" htmlFor="register-display-name">顯示名稱（選填）</label><input id="register-display-name" className="input" autoComplete="nickname" maxLength={100} value={displayName} onChange={e => setDisplayName(e.target.value)} />
          <label className="label mt-3" htmlFor="register-password">密碼</label><input id="register-password" className="input" type="password" autoComplete="new-password" value={password} onChange={e => setPassword(e.target.value)} required /><p className="hint">12 到 256 個字元。</p>
          <label className="label mt-3" htmlFor="register-confirmation">確認密碼</label><input id="register-confirmation" className="input" type="password" autoComplete="new-password" value={confirmation} onChange={e => setConfirmation(e.target.value)} required />
          <button className="btn btn-primary mt-5 w-full" disabled={busy || !username.trim() || !password || !confirmation}>{busy && <Spinner />}建立帳號</button>
        </>}
        {!!policy.providers.length && <section className="registration-providers" aria-label="外部帳號註冊"><p>使用第三方帳號註冊</p>{policy.providers.filter(provider => providerName[provider]).map(provider => <a className="btn btn-secondary w-full" key={provider} aria-disabled={busy} tabIndex={busy ? -1 : undefined} onClick={e => { if (busy) e.preventDefault(); }} href={`/oauth/login/${provider}?intent=register${next ? `&next=${encodeURIComponent(next)}` : ""}`}>使用 {providerName[provider]} 註冊</a>)}<p className="hint">註冊為新成員；不會依電子郵件合併既有帳號。</p></section>}
      </>}
    </>}
    <p className="registration-return"><Link to={`/login${continuation(location.search)}`}>已有帳號？返回登入</Link></p>
  </form>;
}
