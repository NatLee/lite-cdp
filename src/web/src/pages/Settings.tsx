import { AccountRegistrationStatus } from "../registration/AccountRegistrationStatus";
import { RegistrationAdmin } from "../registration/RegistrationAdmin";
import { KeyRound, Pencil, UserPlus } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { api } from "../api";
import { PageBody } from "../components/Layout";
import { Badge, Dialog, ErrorBanner, Loading, PageHeader, Spinner, useToast } from "../components/ui";
import { useApp, useUser } from "../context";
import { ROLE_LABEL, dateTime } from "../lib/format";
import { errorText, useAsync } from "../lib/hooks";
import type { Account, Role } from "../types";

function Section({ title, hint, actions, children }: { title: string; hint?: string; actions?: ReactNode; children: ReactNode }) {
  return (
    <section className="card mb-4">
      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-zinc-100 px-4 py-3 sm:px-5 dark:border-zinc-800">
        <div>
          <h2 className="text-sm font-semibold">{title}</h2>
          {hint && <p className="text-xs text-zinc-500 dark:text-zinc-400">{hint}</p>}
        </div>
        {actions}
      </div>
      {children}
    </section>
  );
}

function Profile() {
  const user = useUser();
  const { setUser } = useApp();
  const toast = useToast();
  const [name, setName] = useState(user.display_name);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const changed = name.trim() !== user.display_name;

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      setUser(await api.updateMe({ display_name: name.trim() }));
      toast("個人資料存好了");
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Section title="個人資料" hint={`帳號 ${user.username}・${ROLE_LABEL[user.role]}`}>
      <form
        className="flex max-w-md flex-col gap-3 p-4 sm:p-5"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <div>
          <label className="label" htmlFor="me-name">
            顯示名稱
          </label>
          <input id="me-name" className="input" value={name} maxLength={100} onChange={(e) => setName(e.target.value)} />
          <p className="hint">同事在帳號清單和分群的建立者上看到的名字。</p>
        </div>
        {error && <ErrorBanner message={error} />}
        <button className="btn btn-primary self-start" disabled={busy || !name.trim() || !changed}>
          {busy && <Spinner />} 儲存
        </button>
      </form>
    </Section>
  );
}

function Password() {
  const toast = useToast();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [again, setAgain] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mismatch = again !== "" && next !== again;

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.changePassword(current, next);
      setCurrent("");
      setNext("");
      setAgain("");
      toast("密碼改好了，其他裝置要重新登入");
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Section title="密碼">
      <form
        className="flex max-w-md flex-col gap-3 p-4 sm:p-5"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <div>
          <label className="label" htmlFor="pw-current">
            目前的密碼
          </label>
          <input
            id="pw-current"
            className="input"
            type="password"
            autoComplete="current-password"
            value={current}
            onChange={(e) => setCurrent(e.target.value)}
          />
        </div>
        <div>
          <label className="label" htmlFor="pw-new">
            新密碼
          </label>
          <input
            id="pw-new"
            className="input"
            type="password"
            autoComplete="new-password"
            value={next}
            onChange={(e) => setNext(e.target.value)}
          />
          <p className="hint">至少 8 個字元。</p>
        </div>
        <div>
          <label className="label" htmlFor="pw-again">
            再打一次新密碼
          </label>
          <input
            id="pw-again"
            className="input"
            type="password"
            autoComplete="new-password"
            value={again}
            onChange={(e) => setAgain(e.target.value)}
          />
          {mismatch && <p className="mt-1 text-xs text-rose-600 dark:text-rose-400">兩次打的不一樣</p>}
        </div>
        {error && <ErrorBanner message={error} />}
        <button className="btn btn-primary self-start" disabled={busy || !current || next.length < 8 || next !== again}>
          {busy ? <Spinner /> : <KeyRound className="size-4" />} 改密碼
        </button>
      </form>
    </Section>
  );
}

/** 新增帳號，或（`account` 有值時）改名字、重設密碼。 */
function AccountDialog({
  open,
  account,
  onClose,
  onSaved,
}: {
  open: boolean;
  account?: Account;
  onClose: () => void;
  onSaved: () => void;
}) {
  const toast = useToast();
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [role, setRole] = useState<Role>("member");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setUsername(account?.username ?? "");
    setDisplayName(account?.display_name ?? "");
    setPassword("");
    setRole(account?.role ?? "member");
    setError(null);
  }, [open, account]);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      if (account) {
        await api.updateUser(account.id, {
          display_name: displayName.trim() || undefined,
          password: password || undefined,
        });
        toast(password ? "已儲存，密碼也重設了" : "已儲存");
      } else {
        await api.createUser({ username: username.trim(), display_name: displayName.trim(), password, role });
        toast(`建立了帳號 ${username.trim()}`);
      }
      onSaved();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const ready = account ? password === "" || password.length >= 8 : username.trim().length >= 3 && password.length >= 8;

  return (
    <Dialog
      open={open}
      title={account ? `編輯帳號 ${account.username}` : "新增帳號"}
      onClose={onClose}
      footer={
        <>
          <button className="btn btn-secondary" onClick={onClose}>
            取消
          </button>
          <button className="btn btn-primary" disabled={busy || !ready} onClick={() => void save()}>
            {busy && <Spinner />} {account ? "儲存" : "建立"}
          </button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        {!account && (
          <div>
            <label className="label" htmlFor="acct-username">
              帳號
            </label>
            <input
              id="acct-username"
              className="input"
              autoComplete="off"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
            <p className="hint">3 到 32 個字元：英文字母、數字、. _ -。建立之後不能改。</p>
          </div>
        )}
        <div>
          <label className="label" htmlFor="acct-name">
            顯示名稱
          </label>
          <input
            id="acct-name"
            className="input"
            maxLength={100}
            placeholder={account ? "" : "不填就用帳號"}
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
          />
        </div>
        <div>
          <label className="label" htmlFor="acct-password">
            {account ? "重設密碼" : "密碼"}
          </label>
          <input
            id="acct-password"
            className="input"
            type="password"
            autoComplete="new-password"
            placeholder={account ? "不改就留白" : ""}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
          <p className="hint">至少 8 個字元。請用別的管道把密碼交給對方，並請對方登入後自己改掉。</p>
        </div>
        {!account && (
          <div>
            <label className="label" htmlFor="acct-role">
              角色
            </label>
            <select id="acct-role" className="input" value={role} onChange={(e) => setRole(e.target.value as Role)}>
              <option value="member">成員</option>
              <option value="admin">管理員（看得到所有網站，可以管理網站和帳號）</option>
            </select>
          </div>
        )}
        {error && <ErrorBanner message={error} />}
      </div>
    </Dialog>
  );
}

function Accounts({ accounts, error, reload }: { accounts?: Account[]; error: string | null; reload: () => Promise<void> }) {
  const me = useUser();
  const toast = useToast();
  const [dialog, setDialog] = useState<{ account?: Account } | null>(null);

  const change = async (account: Account, input: { role?: Role; disabled?: boolean }) => {
    try {
      await api.updateUser(account.id, input);
      await reload();
    } catch (e) {
      toast(errorText(e), "error");
    }
  };

  return (
    <Section
      title="帳號"
      hint="成員只看得到被加進去的網站；離開的人停用就好。"
      actions={
        <button className="btn btn-secondary" onClick={() => setDialog({})}>
          <UserPlus className="size-4" /> 新增帳號
        </button>
      }
    >
      {error && (
        <div className="p-4">
          <ErrorBanner message={error} />
        </div>
      )}
      {!accounts && !error && <Loading />}
      {accounts && (
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b border-zinc-100 dark:border-zinc-800">
                <th className="th">帳號</th>
                <th className="th">角色</th>
                <th className="th text-right">看得到的網站</th>
                <th className="th">建立時間</th>
                <th className="th" />
              </tr>
            </thead>
            <tbody className="divide-y divide-zinc-100 dark:divide-zinc-800">
              {accounts.map((a) => {
                const self = a.id === me.id;
                return (
                  <tr key={a.id} className={a.disabled ? "text-zinc-400 dark:text-zinc-500" : ""}>
                    <td className="td">
                      <span className="font-semibold">{a.display_name}</span>
                      <span className="ml-2 font-mono text-xs text-zinc-500 dark:text-zinc-400">{a.username}</span>
                      {self && <span className="ml-2 text-xs text-zinc-500 dark:text-zinc-400">（你）</span>}
                      <AccountRegistrationStatus status={a.account_status} />
                      {a.disabled && (
                        <span className="ml-2">
                          <Badge tone="red">已停用</Badge>
                        </span>
                      )}
                    </td>
                    <td className="td">
                      <select
                        className="input w-auto py-1 text-xs"
                        aria-label={`${a.username} 的角色`}
                        value={a.role}
                        disabled={self || (a.account_status !== undefined && a.account_status !== "active")}
                        onChange={(e) => void change(a, { role: e.target.value as Role })}
                      >
                        <option value="member">{ROLE_LABEL.member}</option>
                        <option value="admin">{ROLE_LABEL.admin}</option>
                      </select>
                    </td>
                    <td className="td text-right tabular-nums">{a.site_count}</td>
                    <td className="td whitespace-nowrap">{dateTime(a.created_at)}</td>
                    <td className="td text-right whitespace-nowrap">
                      <button className="btn btn-ghost px-2 py-1 text-xs" onClick={() => setDialog({ account: a })}>
                        <Pencil className="size-4" /> 編輯
                      </button>
                      {!self && (!a.account_status || a.account_status === "active") && (
                        <button
                          className="btn btn-ghost px-2 py-1 text-xs"
                          onClick={() => {
                            if (a.disabled || window.confirm(`停用 ${a.username}？對方會馬上被登出，之後不能登入。`)) {
                              void change(a, { disabled: !a.disabled });
                            }
                          }}
                        >
                          {a.disabled ? "重新啟用" : "停用"}
                        </button>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
      <AccountDialog
        open={dialog !== null}
        account={dialog?.account}
        onClose={() => setDialog(null)}
        onSaved={() => {
          setDialog(null);
          void reload();
        }}
      />
    </Section>
  );
}

export function Settings() {
  const admin = useUser().role === "admin";
  // 只有管理員拿得到帳號清單
  const accounts = useAsync(() => (admin ? api.users() : Promise.resolve(undefined)), [admin]);

  return (
    <PageBody>
      <PageHeader title="帳號設定" subtitle={admin ? "你的帳號，以及團隊的帳號。" : "你的帳號。"} />
      <Profile />
      <Password />
      {admin && <RegistrationAdmin onReviewed={() => void accounts.reload()} />}
      {admin && <Accounts accounts={accounts.data} error={accounts.error} reload={accounts.reload} />}
    </PageBody>
  );
}
