import { useEffect, useState } from "react";
import { accessRequest } from "./api";
import type { Consent } from "./types";
import "./access.css";

export function Authorize({ requestId }: { requestId?: string }) {
  const id = requestId ?? new URLSearchParams(window.location.search).get("request_id") ?? "";
  const [consent, setConsent] = useState<Consent | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let current = true;
    if (!id || id.length > 256) { setError("授權請求不存在或已失效，請回到原應用重新開始。"); return; }
    accessRequest<Consent>(`/consents/${encodeURIComponent(id)}`).then(data => { if (current) setConsent(data); }).catch(e => { if (current) setError(e.message); });
    return () => { current = false; };
  }, [id]);
  return <main className="access-workspace access-consent">
    <p className="access-rubric">應用授權 / OAUTH CONSENT</p>
    {error ? <p role="alert">{error}</p> : !consent ? <p role="status">正在核對授權請求…</p> : <>
      <h1>授權 {consent.client_name}</h1>
      <p>以 {consent.user.display_name}（{consent.user.username}）的身分授權此應用。</p>
      <dl><dt>允許的操作範圍</dt><dd>{consent.scopes.join("、")}</dd><dt>限定資源</dt><dd className="access-wrap">{consent.resource}</dd><dt>請求到期時間（UTC）</dt><dd>{consent.expires_at}</dd></dl>
      <p>應用仍受目前帳號與資料權限限制；可在「連線與存取」撤銷授權。</p>
      <form method="post" action={`/oauth/consent/${encodeURIComponent(consent.request_id)}`} className="access-actions">
        <button className="btn btn-primary" name="decision" value="approve">同意授權</button>
        <button className="btn btn-secondary" name="decision" value="deny">拒絕</button>
      </form>
    </>}
    <a className="access-back" href="/connections">返回連線與存取</a>
  </main>;
}
