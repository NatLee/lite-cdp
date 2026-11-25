import { Globe, Plus, Users } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router";
import { api } from "../api";
import { PageBody, useSites } from "../components/Layout";
import { Badge, Dialog, Empty, ErrorBanner, PageHeader, Spinner } from "../components/ui";
import { useUser } from "../context";
import { num, timeAgo } from "../lib/format";
import { errorText } from "../lib/hooks";

function NewSiteDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const navigate = useNavigate();
  const { reloadSites } = useSites();
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setName("");
    setUrl("");
    setError(null);
  }, [open]);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const site = await api.createSite({ name: name.trim(), url: url.trim() });
      await reloadSites();
      // 建好就去安裝頁：那裡有要貼進網頁的追蹤程式碼
      navigate(`/s/${site.id}/install`);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      title="新增網站"
      onClose={onClose}
      footer={
        <>
          <button className="btn btn-secondary" onClick={onClose}>
            取消
          </button>
          <button className="btn btn-primary" disabled={busy || !name.trim()} onClick={() => void save()}>
            {busy && <Spinner />} 建立
          </button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <div>
          <label className="label" htmlFor="site-name">
            名稱
          </label>
          <input id="site-name" className="input" maxLength={60} value={name} onChange={(e) => setName(e.target.value)} />
        </div>
        <div>
          <label className="label" htmlFor="site-url">
            網址
          </label>
          <input
            id="site-url"
            className="input"
            type="url"
            placeholder="https://www.example.com"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
          />
          <p className="hint">有填的話，只收這個網址送來的資料；之後可以在「安裝與設定」再加其他來源。</p>
        </div>
        {error && <ErrorBanner message={error} />}
      </div>
    </Dialog>
  );
}

export function Sites() {
  const admin = useUser().role === "admin";
  const { sites } = useSites();
  const [creating, setCreating] = useState(false);

  return (
    <PageBody>
      <PageHeader
        title="所有網站"
        subtitle={admin ? "每個網站有自己的追蹤程式碼、名單、標籤和分群。" : "你看得到的網站。"}
        actions={
          admin && (
            <button className="btn btn-primary" onClick={() => setCreating(true)}>
              <Plus className="size-4" /> 新增網站
            </button>
          )
        }
      />
      {sites.length === 0 ? (
        <div className="card">
          <Empty icon={Globe} title={admin ? "還沒有網站" : "你還沒有可以看的網站"}>
            {admin ? "新增一個網站，把追蹤程式碼貼進網頁，就會開始記錄訪客。" : "請管理員把你加進網站的成員。"}
          </Empty>
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {sites.map((site) => (
            <Link
              key={site.id}
              to={`/s/${site.id}/overview`}
              className="card flex flex-col gap-3 p-4 transition-colors hover:border-brand-300 dark:hover:border-brand-700"
            >
              <div className="flex items-start justify-between gap-2">
                <div className="min-w-0">
                  <h2 className="truncate font-semibold">{site.name}</h2>
                  <p className="truncate text-xs text-zinc-500 dark:text-zinc-400">{site.url || "沒有填網址"}</p>
                </div>
                {site.active ? <Badge tone="green">收資料中</Badge> : <Badge>已停收</Badge>}
              </div>
              <div className="flex items-baseline gap-4">
                <p>
                  <span className="text-2xl font-semibold tracking-tight tabular-nums">{num(site.people)}</span>
                  <span className="ml-1 text-xs text-zinc-500 dark:text-zinc-400">人</span>
                </p>
                <p className="text-xs text-zinc-500 dark:text-zinc-400">其中會員 {num(site.identified)}</p>
              </div>
              <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1 border-t border-zinc-100 pt-3 text-xs text-zinc-500 dark:border-zinc-800 dark:text-zinc-400">
                <span className="inline-flex min-w-0 items-center gap-1">
                  <Users className="size-3.5 shrink-0" />
                  <span className="truncate">
                    {site.members.length === 0 ? "只有管理員看得到" : site.members.map((m) => m.display_name).join("、")}
                  </span>
                </span>
                <span>{site.last_event_at ? `${timeAgo(site.last_event_at)}有資料` : "還沒收到資料"}</span>
              </div>
            </Link>
          ))}
        </div>
      )}
      <NewSiteDialog open={creating} onClose={() => setCreating(false)} />
    </PageBody>
  );
}
