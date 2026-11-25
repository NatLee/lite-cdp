import { Check, CircleCheck, CircleDashed, Copy, RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { useNavigate } from "react-router";
import { api } from "../api";
import { PageBody, useSite, useSites } from "../components/Layout";
import { CardHeader } from "../components/people";
import { Dialog, ErrorBanner, PageHeader, Spinner, useToast } from "../components/ui";
import { useApp, useUser } from "../context";
import { parseLines, timeAgo } from "../lib/format";
import { errorText, useAsync } from "../lib/hooks";
import type { Site } from "../types";

function CodeBlock({ label, code }: { label: string; code: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // 瀏覽器不給用剪貼簿（不是 https 之類的）就讓使用者自己選起來複製
    }
  };
  return (
    <div className="relative">
      <pre
        aria-label={label}
        className="overflow-x-auto rounded-lg bg-zinc-900 p-3 pr-20 font-mono text-xs leading-relaxed text-zinc-100 dark:bg-zinc-950 dark:ring-1 dark:ring-zinc-800"
      >
        {code}
      </pre>
      <button
        type="button"
        className="absolute top-2 right-2 inline-flex items-center gap-1 rounded-md bg-white/10 px-2 py-1 text-xs text-zinc-100 transition-colors hover:bg-white/20"
        aria-label={`複製${label}`}
        onClick={() => void copy()}
      >
        {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />} {copied ? "已複製" : "複製"}
      </button>
    </div>
  );
}

function Step({ n, title, children }: { n: number; title: string; children: ReactNode }) {
  return (
    <li className="flex gap-3">
      <span className="mt-0.5 grid size-6 shrink-0 place-items-center rounded-full bg-brand-100 text-xs font-semibold text-brand-700 dark:bg-brand-950 dark:text-brand-300">
        {n}
      </span>
      <div className="min-w-0 flex-1">
        <h3 className="text-sm font-semibold">{title}</h3>
        <div className="mt-2 flex flex-col gap-2 text-sm text-zinc-600 dark:text-zinc-300">{children}</div>
      </div>
    </li>
  );
}

const OPTIONS: { attr: string; text: string }[] = [
  { attr: 'data-respect-dnt="true"', text: "瀏覽器開了「不要追蹤」的訪客就不記錄。" },
  { attr: 'data-auto-click="false"', text: "不要自動記錄點了哪些連結和按鈕。" },
  { attr: 'data-auto-scroll="false"', text: "不要自動記錄頁面捲到多深。" },
  { attr: 'data-endpoint="https://…/collect"', text: "資料改送到別的網址（例如經過自己的反向代理）。" },
];

function Guide({ site }: { site: Site }) {
  const { meta } = useApp();
  const origin = (meta?.public_url || window.location.origin).replace(/\/$/, "");
  const snippet = `<script async src="${origin}/t.js" data-key="${site.key}"></script>`;
  const identify = `<script>
  window.cdpq = window.cdpq || [];
  cdpq.push(["identify", { email: "amy@example.com", name: "Amy", plan: "pro" }]);
</script>`;
  const track = `cdp.track("add_to_cart", { sku: "A-1001", price: 1280 });`;

  return (
    <section className="card">
      <CardHeader
        title="追蹤程式碼"
        actions={
          site.last_event_at ? (
            <span className="inline-flex items-center gap-1 text-xs text-emerald-700 dark:text-emerald-400">
              <CircleCheck className="size-4" /> 已收到資料・最近一筆是{timeAgo(site.last_event_at)}
            </span>
          ) : (
            <span className="inline-flex items-center gap-1 text-xs text-zinc-500 dark:text-zinc-400">
              <CircleDashed className="size-4" /> 還沒收到資料
            </span>
          )
        }
      />
      <ol className="flex flex-col gap-6 p-4 sm:p-5">
        <Step n={1} title="貼進每一頁的 <head>">
          <CodeBlock label="追蹤程式碼" code={snippet} />
          <p>
            裝好之後會自動記錄每一頁的瀏覽、點擊和捲動深度。單頁應用（SPA）換網址時也會算一次瀏覽。
            {!meta?.public_url && " 伺服器沒有設定 CDP_PUBLIC_URL，上面用的是你現在開的這個網址。"}
          </p>
        </Step>
        <Step n={2} title="訪客登入或留下 email 時，告訴我們是誰">
          <CodeBlock label="identify 範例" code={identify} />
          <p>
            這個瀏覽器之前匿名看過的紀錄會併到這個人身上；同一個 email 在別的裝置登入也會併在一起。
            <code className="font-mono text-xs">email</code>、<code className="font-mono text-xs">name</code>、
            <code className="font-mono text-xs">account</code> 以外的欄位會存成自訂屬性。登出時呼叫{" "}
            <code className="font-mono text-xs">cdp.reset()</code>。
          </p>
        </Step>
        <Step n={3} title="想記下特定的動作，送自訂事件">
          <CodeBlock label="track 範例" code={track} />
          <p>
            不想寫程式的話，在按鈕上加 <code className="font-mono text-xs">data-cdp-click="add_to_cart"</code>{" "}
            也可以；事件名稱可以拿來建標籤。不想被記錄的區塊加 <code className="font-mono text-xs">data-cdp-ignore</code>。
          </p>
        </Step>
      </ol>
      <div className="border-t border-zinc-100 px-4 py-4 sm:px-5 dark:border-zinc-800">
        <h3 className="text-sm font-semibold">可以加在 script 標籤上的選項</h3>
        <dl className="mt-2 flex flex-col gap-1.5 text-sm">
          {OPTIONS.map((o) => (
            <div key={o.attr} className="flex flex-wrap items-baseline gap-x-3">
              <dt className="font-mono text-xs text-zinc-800 dark:text-zinc-200">{o.attr}</dt>
              <dd className="text-zinc-600 dark:text-zinc-300">{o.text}</dd>
            </div>
          ))}
        </dl>
        <p className="hint mt-3">
          不會存訪客的 IP。
          {meta && (meta.retention_days > 0 ? `事件保留 ${meta.retention_days} 天，過期自動刪除。` : "事件不會自動刪除（CDP_RETENTION_DAYS 可以設定保留天數）。")}
        </p>
      </div>
    </section>
  );
}

/** 管理員才看得到：名稱、允許的來源、成員、停收、換金鑰、刪除。 */
function SiteSettings({ site }: { site: Site }) {
  const { reloadSites } = useSites();
  const navigate = useNavigate();
  const toast = useToast();
  const users = useAsync(() => api.users(), []);
  const [name, setName] = useState(site.name);
  const [url, setUrl] = useState(site.url);
  const [origins, setOrigins] = useState(site.allowed_origins.join("\n"));
  const [active, setActive] = useState(site.active);
  const [memberIds, setMemberIds] = useState(site.members.map((m) => m.id));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<"rotate" | "delete" | null>(null);

  useEffect(() => {
    setName(site.name);
    setUrl(site.url);
    setOrigins(site.allowed_origins.join("\n"));
    setActive(site.active);
    setMemberIds(site.members.map((m) => m.id));
  }, [site]);

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const save = () =>
    run(async () => {
      await api.updateSite(site.id, {
        name: name.trim(),
        url: url.trim(),
        allowed_origins: parseLines(origins),
        active,
        member_ids: memberIds,
      });
      await reloadSites();
      toast("設定存好了");
    });

  const rotate = () =>
    run(async () => {
      await api.rotateKey(site.id);
      await reloadSites();
      setConfirming(null);
      toast("換了新的金鑰，記得更新網站上的追蹤程式碼");
    });

  const remove = () =>
    run(async () => {
      await api.deleteSite(site.id);
      await reloadSites();
      toast(`刪掉了「${site.name}」`);
      navigate("/sites");
    });

  const members = users.data?.filter((u) => u.role !== "admin" && !u.disabled) ?? [];

  return (
    <>
      <section className="card">
        <CardHeader title="網站設定" />
        <form
          className="flex flex-col gap-4 p-4 sm:p-5"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <div className="grid gap-3 sm:grid-cols-2">
            <div>
              <label className="label" htmlFor="set-name">
                名稱
              </label>
              <input id="set-name" className="input" maxLength={60} value={name} onChange={(e) => setName(e.target.value)} />
            </div>
            <div>
              <label className="label" htmlFor="set-url">
                網址
              </label>
              <input id="set-url" className="input" type="url" value={url} onChange={(e) => setUrl(e.target.value)} />
            </div>
          </div>
          <div>
            <label className="label" htmlFor="set-origins">
              允許的來源
            </label>
            <textarea
              id="set-origins"
              className="input font-mono text-xs"
              rows={3}
              placeholder={"https://www.example.com\nhttps://shop.example.com"}
              value={origins}
              onChange={(e) => setOrigins(e.target.value)}
            />
            <p className="hint">一行一個，只收這些網址的網頁送來的資料。留白就是都收（金鑰被抄走的話，別人也能送假資料進來）。</p>
          </div>
          <fieldset>
            <legend className="label">誰看得到</legend>
            <p className="hint mt-0 mb-2">管理員看得到所有網站；成員要勾了才看得到。</p>
            {users.error && <ErrorBanner message={users.error} />}
            {members.length === 0 && !users.error ? (
              <p className="text-sm text-zinc-500 dark:text-zinc-400">還沒有成員帳號，可以到「帳號設定」新增。</p>
            ) : (
              <div className="flex flex-wrap gap-x-5 gap-y-2">
                {members.map((u) => (
                  <label key={u.id} className="inline-flex items-center gap-2 text-sm">
                    <input
                      type="checkbox"
                      className="size-4 accent-brand-600"
                      checked={memberIds.includes(u.id)}
                      onChange={(e) => setMemberIds((ids) => (e.target.checked ? [...ids, u.id] : ids.filter((id) => id !== u.id)))}
                    />
                    {u.display_name}
                    <span className="font-mono text-xs text-zinc-500 dark:text-zinc-400">{u.username}</span>
                  </label>
                ))}
              </div>
            )}
          </fieldset>
          <label className="inline-flex items-center gap-2 text-sm">
            <input type="checkbox" className="size-4 accent-brand-600" checked={active} onChange={(e) => setActive(e.target.checked)} />
            收資料
            <span className="text-xs text-zinc-500 dark:text-zinc-400">取消勾選就先停收，已經收到的都還在</span>
          </label>
          {error && !confirming && <ErrorBanner message={error} />}
          <button className="btn btn-primary self-start" disabled={busy || !name.trim()}>
            {busy && !confirming && <Spinner />} 儲存
          </button>
        </form>
      </section>

      <section className="card border-rose-200 dark:border-rose-900/60">
        <CardHeader title="危險的操作" />
        <div className="flex flex-col divide-y divide-zinc-100 dark:divide-zinc-800">
          <div className="flex flex-wrap items-center justify-between gap-3 px-4 py-3 sm:px-5">
            <p className="text-sm text-zinc-600 dark:text-zinc-300">
              換一把金鑰。舊的馬上失效，網站上的追蹤程式碼沒更新之前收不到資料。
            </p>
            <button className="btn btn-secondary" onClick={() => setConfirming("rotate")}>
              <RefreshCw className="size-4" /> 換金鑰
            </button>
          </div>
          <div className="flex flex-wrap items-center justify-between gap-3 px-4 py-3 sm:px-5">
            <p className="text-sm text-zinc-600 dark:text-zinc-300">刪除這個網站，連同所有人、事件、標籤和分群。</p>
            <button className="btn btn-danger" onClick={() => setConfirming("delete")}>
              <Trash2 className="size-4" /> 刪除網站
            </button>
          </div>
        </div>
      </section>

      <Dialog
        open={confirming !== null}
        title={confirming === "delete" ? `刪除「${site.name}」？` : "換一把金鑰？"}
        onClose={() => setConfirming(null)}
        footer={
          <>
            <button className="btn btn-secondary" onClick={() => setConfirming(null)}>
              取消
            </button>
            <button className="btn btn-danger" disabled={busy} onClick={() => void (confirming === "delete" ? remove() : rotate())}>
              {busy && <Spinner />} {confirming === "delete" ? "刪除" : "換金鑰"}
            </button>
          </>
        }
      >
        <p className="text-sm text-zinc-600 dark:text-zinc-300">
          {confirming === "delete"
            ? "所有人、事件、標籤和分群都會刪掉，沒辦法復原。"
            : "舊的金鑰馬上失效。換完之後要把新的追蹤程式碼貼到網站上。"}
        </p>
        {error && (
          <div className="mt-3">
            <ErrorBanner message={error} />
          </div>
        )}
      </Dialog>
    </>
  );
}

export function Install() {
  const site = useSite();
  const admin = useUser().role === "admin";
  return (
    <PageBody>
      <PageHeader title="安裝與設定" subtitle={`把「${site.name}」的追蹤程式碼裝到網站上。`} />
      <div className="flex flex-col gap-4">
        <Guide site={site} />
        {admin && <SiteSettings site={site} />}
      </div>
    </PageBody>
  );
}
