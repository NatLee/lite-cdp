import type { ReactNode } from "react";
import { RegistrationLoginLink, RegistrationNotice } from "../registration/RegisterForm";
import { ProviderButtons } from "../access/ProviderButtons";
import { LogIn, UserPlus } from "lucide-react";
import { useRef, useState } from "react";
import { ErrorBanner, Spinner } from "../components/ui";
import { useApp } from "../context";
import { errorText } from "../lib/hooks";

/** Shared themed account surface; lifecycle is owned by the route pages. */
export function AuthScreen({ setupRequired, children }: { setupRequired: boolean; children?: ReactNode }) {
  const { login, setup, meta } = useApp();
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const submitting = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    if (submitting.current) return;
    setError(null);
    if (setupRequired) {
      if (!/^[a-zA-Z0-9._-]{3,32}$/.test(username.trim())) { setError("帳號要 3 到 32 個字元，只能用英數字和 _ . -"); return; }
      if ([...password].length < 8 || [...password].length > 200) { setError("密碼需為 8 到 200 個字元"); return; }
      if ([...displayName.trim()].length > 40) { setError("顯示名稱最多 40 個字元"); return; }
      if (password !== confirmation) { setError("兩次輸入的密碼不一致"); return; }
    }
    submitting.current = true;
    setBusy(true);
    try {
      if (setupRequired) {
        await setup({
          username: username.trim(),
          display_name: displayName.trim(),
          password,
        });
      } else {
        await login(username.trim(), password);
      }
      setPassword("");
      setConfirmation("");
    } catch (e) {
      setError(errorText(e));
      if (!setupRequired) setPassword("");
    } finally {
      submitting.current = false;
      setBusy(false);
    }
  };

  return (
    <div className="atlas-login">
      <section className="atlas-login-story">
        <span className="eyebrow">LITE CDP / SIGNAL CONSOLE</span>
        <h2>
          從一次造訪，
          <br />
          到真正認識。
        </h2>
        <p>
          把散落的瀏覽、事件與身份，
          <br />
          連成清楚的顧客旅程。
        </p>
        <div className="login-flightplan" aria-hidden="true">
          <div className="flightplan-top">
            IDENTITY PROTOCOL <span>01—03</span>
          </div>
          <div className="flightplan-route">
            <i />
            <i />
            <i />
          </div>
          <div className="flightplan-labels">
            <span>
              PAGEVIEW<small>每一頁瀏覽</small>
            </span>
            <span>
              INTERACTION<small>每一次互動</small>
            </span>
            <span>
              IDENTIFY<small>每一位訪客</small>
            </span>
          </div>
          <div className="flightplan-caption">
            TRACE THE VISIT.
            <br />
            <b>Understand the person.</b>
          </div>
        </div>
      </section>
      <div className="atlas-login-form">
        {children ?? <form
          className="card w-full max-w-sm p-6"
          aria-busy={busy}
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
        >
          <div className="mb-6 flex items-center gap-3">
            <img src="/favicon.svg" alt="" className="size-10" />
            <div>
              <h1 className="font-semibold tracking-tight">{setupRequired ? "建立第一位管理員" : "Lite CDP"}</h1>
              <p className="text-xs text-zinc-500 dark:text-zinc-400">
                {setupRequired
                  ? "首次啟用 / FIRST LOGIN"
                  : "認識你的網站訪客"}
                {meta ? `・v${meta.version}` : ""}
              </p>
            </div>
          </div>
          {setupRequired && <div className="first-login-note">
            <p className="eyebrow">01 建立帳號 / 02 邀請團隊</p>
            <p>由你保管工作空間的第一把鑰匙。建立後即可登入，再由管理員設定成員註冊與審核。</p>
          </div>}
          <label className="label" htmlFor="login-username">
            帳號
          </label>
          <input
            id="login-username"
            className="input"
            autoComplete="username"
            autoFocus
            value={username}
            onChange={(e) => setUsername(e.target.value)}
          />
          {setupRequired && (
            <>
              <p className="hint">3 到 32 個字元：英文字母、數字、. _ -</p>
              <label className="label mt-3" htmlFor="login-display-name">
                顯示名稱
              </label>
              <input
                id="login-display-name"
                className="input"
                placeholder="其他人看到的名字，可以不填"
                value={displayName}
                onChange={(e) => setDisplayName(e.target.value)}
              />
            </>
          )}
          <label className="label mt-3" htmlFor="login-password">
            密碼
          </label>
          <input
            id="login-password"
            className="input"
            type="password"
            autoComplete={setupRequired ? "new-password" : "current-password"}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
          {setupRequired && <>
            <p className="hint">8 到 200 個字元。</p>
            <label className="label mt-3" htmlFor="login-confirmation">確認密碼</label>
            <input id="login-confirmation" className="input" type="password" autoComplete="new-password" value={confirmation} onChange={e=>setConfirmation(e.target.value)} />
          </>}
          {error && (
            <div className="mt-4">
              <ErrorBanner message={error} />
            </div>
          )}
          <button
            className="btn btn-primary mt-5 w-full py-2"
            type="submit"
            disabled={busy || !username.trim() || !password || (setupRequired && !confirmation)}
          >
            {busy ? (
              <Spinner />
            ) : setupRequired ? (
              <UserPlus className="size-4" />
            ) : (
              <LogIn className="size-4" />
            )}
            {setupRequired ? "建立管理員" : "登入"}
          </button>
          <p className="mt-4 text-xs text-zinc-500 dark:text-zinc-400">
            {setupRequired
              ? "這個帳號可以新增網站和其他帳號。之後這個畫面就不會再出現。"
              : "沒有帳號或忘記密碼，請找管理員。"}
          </p>
          {!setupRequired && <><RegistrationNotice /><RegistrationLoginLink /><ProviderButtons/></>}
        </form>}
        <p className="login-footer">YOUR DATA. YOUR CUSTOMER STORIES.</p>
      </div>
    </div>
  );
}
