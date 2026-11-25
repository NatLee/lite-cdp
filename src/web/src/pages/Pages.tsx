import { FileText, Search, Tag } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router";
import { api } from "../api";
import { PageBody, useSite } from "../components/Layout";
import { LabelChips } from "../components/people";
import { Empty, ErrorBanner, Loading, PageHeader } from "../components/ui";
import { num, timeAgo } from "../lib/format";
import { useAsync, useDebounced } from "../lib/hooks";

const RANGES = [7, 30, 90];

export function Pages() {
  const site = useSite();
  const [days, setDays] = useState(30);
  const [search, setSearch] = useState("");
  const q = useDebounced(search.trim(), 300);
  const { data, error } = useAsync(() => api.pages(site.id, { days, q: q || undefined }), [site.id, days, q]);

  return (
    <PageBody>
      <PageHeader title="頁面" subtitle="哪些頁面有人看、多少人看；也能看出每一頁會讓訪客得到哪些標籤。" />
      <div className="card mb-4 flex flex-wrap items-center gap-2 p-3">
        <div className="relative min-w-48 flex-1">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-zinc-400" />
          <input
            className="input pl-9"
            type="search"
            aria-label="搜尋"
            placeholder="搜尋路徑或標題"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <select className="input w-auto" aria-label="期間" value={days} onChange={(e) => setDays(Number(e.target.value))}>
          {RANGES.map((d) => (
            <option key={d} value={d}>
              最近 {d} 天
            </option>
          ))}
        </select>
      </div>
      {error && <ErrorBanner message={error} />}
      {!data && !error && <Loading />}
      {data && (
        <div className="card overflow-hidden">
          {data.length === 0 ? (
            <Empty icon={FileText} title={q ? "沒有符合的頁面" : "這段期間沒有人看過任何頁面"} />
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-zinc-100 dark:border-zinc-800">
                    <th className="th pl-4">頁面</th>
                    <th className="th">會得到的標籤</th>
                    <th className="th text-right">訪客</th>
                    <th className="th text-right">瀏覽</th>
                    <th className="th">最近一次</th>
                    <th className="th pr-4" />
                  </tr>
                </thead>
                <tbody className="divide-y divide-zinc-100 dark:divide-zinc-800">
                  {data.map((p) => (
                    <tr key={p.path} className="transition-colors hover:bg-zinc-50 dark:hover:bg-zinc-800/50">
                      <td className="td max-w-80 pl-4">
                        <p className="truncate font-medium">{p.title || p.path}</p>
                        <a
                          className="block truncate font-mono text-xs text-zinc-500 hover:underline dark:text-zinc-400"
                          href={p.url}
                          target="_blank"
                          rel="noreferrer"
                        >
                          {p.path}
                        </a>
                      </td>
                      <td className="td">
                        <LabelChips labels={p.labels} />
                      </td>
                      <td className="td text-right tabular-nums">{num(p.visitors)}</td>
                      <td className="td text-right tabular-nums">{num(p.pageviews)}</td>
                      <td className="td whitespace-nowrap text-zinc-600 dark:text-zinc-300">{timeAgo(p.last_at)}</td>
                      <td className="td pr-4 text-right whitespace-nowrap">
                        <Link
                          className="btn btn-ghost px-2 py-1 text-xs"
                          to={`/s/${site.id}/labels?path=${encodeURIComponent(p.path)}`}
                          title="用這個路徑建一個標籤"
                        >
                          <Tag className="size-3.5" /> 建標籤
                        </Link>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}
    </PageBody>
  );
}
