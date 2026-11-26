import { Download, Search, Users, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router";
import { api } from "../api";
import { PageBody, useSite } from "../components/Layout";
import { LabelChips, Pagination, PersonLink } from "../components/people";
import { Empty, ErrorBanner, Loading, PageHeader } from "../components/ui";
import { dateTime, num, timeAgo } from "../lib/format";
import { useAsync, useDebounced } from "../lib/hooks";
import type { Activity, PeopleFilter, PeopleSort, PersonKind } from "../types";

const PER_PAGE = 25;

const TABS: { value: PersonKind; label: string }[] = [
  { value: "all", label: "全部" },
  { value: "member", label: "會員" },
  { value: "anonymous", label: "匿名訪客" },
];

const SORTS: { value: PeopleSort; label: string }[] = [
  { value: "last_seen", label: "最近來過的在前" },
  { value: "first_seen", label: "最近第一次來的在前" },
  { value: "pageviews", label: "看最多頁的在前" },
];

export function People() {
  const site = useSite();
  const [params, setParams] = useSearchParams();
  const activity = (params.get("activity") || undefined) as
    | Activity
    | undefined;
  const kind = (params.get("kind") ?? "all") as PersonKind;
  const labelId = Number(params.get("label_id")) || undefined;
  const segmentId = Number(params.get("segment_id")) || undefined;
  const sort = (params.get("sort") ?? "last_seen") as PeopleSort;
  const page = Math.max(1, Number(params.get("page")) || 1);

  // 搜尋框自己記著正在打的字，停一下才寫進網址
  const [search, setSearch] = useState(params.get("q") ?? "");
  const q = useDebounced(search.trim(), 300);

  /** 改篩選條件：寫進網址（重新整理、分享連結都還在），並回到第一頁。 */
  const set = (changes: Record<string, string | undefined>) =>
    setParams(
      (current) => {
        const next = new URLSearchParams(current);
        if (!("page" in changes)) next.delete("page");
        for (const [key, value] of Object.entries(changes)) {
          if (value) next.set(key, value);
          else next.delete(key);
        }
        return next;
      },
      { replace: true },
    );

  useEffect(() => {
    if (q !== (params.get("q") ?? "")) set({ q: q || undefined });
    // 只在打完字的時候同步
  }, [q]);

  const filter: PeopleFilter = {
    activity,
    kind: kind === "all" ? undefined : kind,
    q: params.get("q") ?? undefined,
    label_id: labelId,
    segment_id: segmentId,
    sort: sort === "last_seen" ? undefined : sort,
  };
  const filterKey = JSON.stringify(filter);

  const labels = useAsync(() => api.labels(site.id), [site.id]);
  const segments = useAsync(() => api.segments(site.id), [site.id]);
  const list = useAsync(
    () => api.people(site.id, { ...filter, page, per_page: PER_PAGE }),
    [site.id, filterKey, page],
  );

  const items = list.data?.items ?? [];
  const total = list.data?.total ?? 0;
  const filtered = Boolean(
    activity || kind !== "all" || filter.q || labelId || segmentId,
  );
  const segment = segments.data?.find((s) => s.id === segmentId);

  return (
    <PageBody>
      <PageHeader
        title="名單"
        subtitle={
          segment
            ? `分群「${segment.name}」裡的人。`
            : "來過這個網站的每一個人：知道 email 的會員，和還不知道是誰的匿名訪客。"
        }
        actions={
          <a
            className="btn btn-secondary"
            href={api.exportUrl(site.id, filter)}
            download
          >
            <Download className="size-4" /> 匯出 CSV
          </a>
        }
      />

      <div
        className="mb-3 flex flex-wrap gap-1"
        role="tablist"
        aria-label="身分"
      >
        {TABS.map((tab) => (
          <button
            key={tab.value}
            role="tab"
            aria-selected={kind === tab.value}
            onClick={() =>
              set({ kind: tab.value === "all" ? undefined : tab.value })
            }
            className={`rounded-lg px-3 py-1.5 text-sm transition-colors ${
              kind === tab.value
                ? "bg-brand-600 font-medium text-white dark:bg-brand-500 dark:text-zinc-950"
                : "text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-400 dark:hover:bg-zinc-800"
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      <div className="card mb-4 flex flex-wrap items-center gap-2 p-3">
        <div className="relative min-w-48 flex-1">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-zinc-400" />
          <input
            className="input pl-9"
            type="search"
            aria-label="搜尋"
            placeholder="搜尋 email、名稱或會員帳號"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <select
          className="input w-auto"
          aria-label="標籤"
          value={labelId ?? ""}
          onChange={(e) => set({ label_id: e.target.value || undefined })}
        >
          <option value="">所有標籤</option>
          {labels.data?.map((l) => (
            <option key={l.id} value={l.id}>
              {l.name}
            </option>
          ))}
        </select>
        <select
          className="input w-auto"
          aria-label="分群"
          value={segmentId ?? ""}
          onChange={(e) => set({ segment_id: e.target.value || undefined })}
        >
          <option value="">不限分群</option>
          {segments.data?.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
            </option>
          ))}
        </select>
        <select
          className="input w-auto"
          aria-label="最近活躍"
          value={activity ?? ""}
          onChange={(e) => set({ activity: e.target.value || undefined })}
        >
          <option value="">不限活躍時間</option>
          <option value="recent">7 天內來過</option>
          <option value="cooling">7–30 天未回訪</option>
          <option value="dormant">超過 30 天未回訪</option>
        </select>
        <select
          className="input w-auto"
          aria-label="排序"
          value={sort}
          onChange={(e) =>
            set({
              sort: e.target.value === "last_seen" ? undefined : e.target.value,
            })
          }
        >
          {SORTS.map((s) => (
            <option key={s.value} value={s.value}>
              {s.label}
            </option>
          ))}
        </select>
        {filtered && (
          <button
            className="btn btn-ghost"
            onClick={() => {
              setSearch("");
              setParams({}, { replace: true });
            }}
          >
            <X className="size-4" /> 清除
          </button>
        )}
      </div>

      {list.error && <ErrorBanner message={list.error} />}
      {!list.data && !list.error && <Loading />}
      {list.data && (
        <div className="card overflow-hidden">
          <div className="border-b border-zinc-100 px-4 py-2.5 text-sm text-zinc-500 dark:border-zinc-800 dark:text-zinc-400">
            共 {num(total)} 人
          </div>
          {items.length === 0 ? (
            <Empty
              icon={Users}
              title={filtered ? "沒有符合條件的人" : "還沒有人來過"}
            >
              {filtered
                ? "換個條件試試，或按「清除」看全部。"
                : "追蹤程式碼裝好、有人打開網頁之後，就會出現在這裡。"}
            </Empty>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-zinc-100 dark:border-zinc-800">
                    <th className="th pl-4">誰</th>
                    <th className="th">標籤</th>
                    <th className="th text-right">瀏覽頁數</th>
                    <th className="th">第一次來</th>
                    <th className="th pr-4">最後一次來</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-zinc-100 dark:divide-zinc-800">
                  {items.map((p) => (
                    <tr
                      key={p.id}
                      className="transition-colors hover:bg-zinc-50 dark:hover:bg-zinc-800/50"
                    >
                      <td className="td max-w-72 pl-4">
                        <PersonLink siteId={site.id} person={p} />
                      </td>
                      <td className="td">
                        <LabelChips labels={p.labels} />
                      </td>
                      <td className="td text-right tabular-nums">
                        {num(p.pageviews)}
                      </td>
                      <td
                        className="td whitespace-nowrap text-zinc-600 dark:text-zinc-300"
                        title={dateTime(p.first_seen)}
                      >
                        {timeAgo(p.first_seen)}
                      </td>
                      <td
                        className="td pr-4 whitespace-nowrap text-zinc-600 dark:text-zinc-300"
                        title={dateTime(p.last_seen)}
                      >
                        {timeAgo(p.last_seen)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          <Pagination
            page={page}
            pages={Math.ceil(total / PER_PAGE)}
            onChange={(p) => set({ page: String(p) })}
          />
        </div>
      )}
    </PageBody>
  );
}
