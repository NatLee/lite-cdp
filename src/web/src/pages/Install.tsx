import { IntegrationWorkbench } from "../installation/Workbench";
import {
  RefreshCw,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { api } from "../api";
import { PageBody, useSite, useSites } from "../components/Layout";
import { CardHeader } from "../components/people";
import {
  Dialog,
  ErrorBanner,
  PageHeader,
  Spinner,
  useToast,
} from "../components/ui";
import { useUser } from "../context";
import { parseLines } from "../lib/format";
import { errorText, useAsync } from "../lib/hooks";
import type { Site } from "../types";

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
  const [confirming, setConfirming] = useState<"rotate" | "delete" | null>(
    null,
  );

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

  const members =
    users.data?.filter((u) => u.role !== "admin" && !u.disabled) ?? [];

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
              <input
                id="set-name"
                className="input"
                maxLength={60}
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </div>
            <div>
              <label className="label" htmlFor="set-url">
                網址
              </label>
              <input
                id="set-url"
                className="input"
                type="url"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
              />
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
            <p className="hint">
              一行一個，只收這些網址的網頁送來的資料。留白就是都收（金鑰被抄走的話，別人也能送假資料進來）。
            </p>
          </div>
          <fieldset>
            <legend className="label">誰看得到</legend>
            <p className="hint mt-0 mb-2">
              管理員看得到所有網站；成員要勾了才看得到。
            </p>
            {users.error && <ErrorBanner message={users.error} />}
            {members.length === 0 && !users.error ? (
              <p className="text-sm text-zinc-500 dark:text-zinc-400">
                還沒有成員帳號，可以到「帳號設定」新增。
              </p>
            ) : (
              <div className="flex flex-wrap gap-x-5 gap-y-2">
                {members.map((u) => (
                  <label
                    key={u.id}
                    className="inline-flex items-center gap-2 text-sm"
                  >
                    <input
                      type="checkbox"
                      className="size-4 accent-brand-600"
                      checked={memberIds.includes(u.id)}
                      onChange={(e) =>
                        setMemberIds((ids) =>
                          e.target.checked
                            ? [...ids, u.id]
                            : ids.filter((id) => id !== u.id),
                        )
                      }
                    />
                    {u.display_name}
                    <span className="font-mono text-xs text-zinc-500 dark:text-zinc-400">
                      {u.username}
                    </span>
                  </label>
                ))}
              </div>
            )}
          </fieldset>
          <label className="inline-flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              className="size-4 accent-brand-600"
              checked={active}
              onChange={(e) => setActive(e.target.checked)}
            />
            收資料
            <span className="text-xs text-zinc-500 dark:text-zinc-400">
              取消勾選就先停收，已經收到的都還在
            </span>
          </label>
          {error && !confirming && <ErrorBanner message={error} />}
          <button
            className="btn btn-primary self-start"
            disabled={busy || !name.trim()}
          >
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
            <button
              className="btn btn-secondary"
              onClick={() => setConfirming("rotate")}
            >
              <RefreshCw className="size-4" /> 換金鑰
            </button>
          </div>
          <div className="flex flex-wrap items-center justify-between gap-3 px-4 py-3 sm:px-5">
            <p className="text-sm text-zinc-600 dark:text-zinc-300">
              刪除這個網站，連同所有人、事件、標籤和分群。
            </p>
            <button
              className="btn btn-danger"
              onClick={() => setConfirming("delete")}
            >
              <Trash2 className="size-4" /> 刪除網站
            </button>
          </div>
        </div>
      </section>

      <Dialog
        open={confirming !== null}
        title={
          confirming === "delete" ? `刪除「${site.name}」？` : "換一把金鑰？"
        }
        onClose={() => setConfirming(null)}
        footer={
          <>
            <button
              className="btn btn-secondary"
              onClick={() => setConfirming(null)}
            >
              取消
            </button>
            <button
              className="btn btn-danger"
              disabled={busy}
              onClick={() =>
                void (confirming === "delete" ? remove() : rotate())
              }
            >
              {busy && <Spinner />}{" "}
              {confirming === "delete" ? "刪除" : "換金鑰"}
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
      <PageHeader
        title="安裝與設定"
        subtitle={`把「${site.name}」的追蹤程式碼裝到網站上。`}
      />
      <div className="flex flex-col gap-4">
        <IntegrationWorkbench site={site} />
        {admin && <SiteSettings site={site} />}
      </div>
    </PageBody>
  );
}
