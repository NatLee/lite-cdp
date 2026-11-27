import { useEffect, useState } from "react";
import { accessRequest } from "./api";
import { safeAuthorizeNext, providerErrorMessage } from "./navigation";
import type { Provider } from "./types";
import "./access.css";

export function ProviderButtons() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [error, setError] = useState("");
  useEffect(() => {
    let current = true;
    accessRequest<{ items: Provider[] }>("/providers").then(data => { if (current) setProviders(data.items); }).catch(() => { if (current) setError("無法載入外部登入方式，仍可使用其他已設定的方式登入。"); });
    return () => { current = false; };
  }, []);
  const start = (provider: Provider) => {
    const next = safeAuthorizeNext(window.location.search);
    window.location.assign(provider.login_start_url + (next ? `?next=${encodeURIComponent(next)}` : ""));
  };
  return <section className="access-provider-login" aria-label="外部帳號登入">
    <p>已綁定的帳號可使用外部登入；新成員請使用註冊入口，既有成員可登入後綁定。</p>
    {providerErrorMessage(window.location.search) && <p role="alert">{providerErrorMessage(window.location.search)}</p>}
    {error && <p>{error}</p>}
    <div>{providers.map(provider => <div key={provider.provider}>
      <button className="btn btn-secondary w-full" type="button" disabled={!provider.enabled} onClick={() => start(provider)}>使用 {provider.name} 登入</button>
      {!provider.enabled && <p className="hint">{provider.disabled_reason ?? "尚未設定登入供應商"}</p>}
    </div>)}</div>
  </section>;
}
