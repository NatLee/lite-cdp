import { ArrowRight, Code2, Eye, Footprints, UserCheck, Users } from "lucide-react";
import { Link } from "react-router";
import { api } from "../api";
import { PageBody, useSite } from "../components/Layout";
import { BarList, CardHeader, PersonLink, StatCard, Trend } from "../components/people";
import { Badge, Empty, ErrorBanner, Loading, PageHeader } from "../components/ui";
import { num, timeAgo } from "../lib/format";
import { useAsync } from "../lib/hooks";
import type { Overview } from "../types";

/** 最近 30 天每天的瀏覽頁數。 */
function DailyChart({ daily }: { daily: Overview["daily"] }) {
  const max = Math.max(1, ...daily.map((d) => d.pageviews));
  const total = daily.reduce((sum, d) => sum + d.pageviews, 0);
  const label = (date: string) => date.slice(5).replace("-", "/");
  return (
    <section className="card p-4 sm:p-5">
      <div className="flex items-baseline justify-between">
        <h2 className="text-sm font-semibold">最近 30 天</h2>
        <span className="text-xs text-zinc-500 dark:text-zinc-400">共瀏覽 {num(total)} 頁</span>
      </div>
      <div className="mt-4 flex h-36 items-end gap-[3px]" role="img" aria-label={`最近 30 天共瀏覽 ${total} 頁`}>
        {daily.map((d) => (
          <div key={d.date} className="group relative flex h-full flex-1 items-end">
            <div
              className={`w-full rounded-t-sm transition-colors ${
                d.pageviews === 0
                  ? "bg-zinc-200 dark:bg-zinc-800"
                  : "bg-brand-500 group-hover:bg-brand-600 dark:bg-brand-500 dark:group-hover:bg-brand-400"
              }`}
              style={{ height: d.pageviews === 0 ? 2 : `${Math.max(6, (d.pageviews / max) * 100)}%` }}
            />
            <span className="pointer-events-none absolute bottom-full left-1/2 z-10 mb-1 hidden -translate-x-1/2 rounded-md bg-zinc-900 px-1.5 py-0.5 text-[11px] whitespace-nowrap text-white group-hover:block dark:bg-white dark:text-zinc-900">
              {label(d.date)}・{num(d.pageviews)} 頁・{num(d.visitors)} 人
            </span>
          </div>
        ))}
      </div>
      <div className="mt-1.5 flex justify-between text-[11px] text-zinc-500 tabular-nums dark:text-zinc-400">
        <span>{label(daily[0]?.date ?? "")}</span>
        <span>{label(daily[Math.floor(daily.length / 2)]?.date ?? "")}</span>
        <span>今天</span>
      </div>
    </section>
  );
}

function More({ to, children }: { to: string; children: string }) {
  return (
    <Link to={to} className="inline-flex items-center gap-1 text-xs text-brand-700 hover:underline dark:text-brand-300">
      {children} <ArrowRight className="size-3" />
    </Link>
  );
}

export function OverviewPage() {
  const site = useSite();
  const base = `/s/${site.id}`;
  const { data, error } = useAsync(() => api.overview(site.id), [site.id]);

  return (
    <PageBody>
      <PageHeader
        title={site.name}
        subtitle={site.url || "誰來過、看了什麼、哪些人值得聯絡。"}
        actions={!site.active && <Badge tone="amber">已停止收資料</Badge>}
      />
      {error && <ErrorBanner message={error} />}
      {!data && !error && <Loading />}
      {data && data.totals.people === 0 && (
        <div className="card">
          <Empty icon={Code2} title="還沒有收到資料">
            把追蹤程式碼貼進網站，有人打開網頁之後這裡就會開始有數字。
            <div className="mt-4">
              <Link to={`${base}/install`} className="btn btn-primary">
                <Code2 className="size-4" /> 看追蹤程式碼
              </Link>
            </div>
          </Empty>
        </div>
      )}
      {data && data.totals.people > 0 && (
        <div className="flex flex-col gap-4">
          <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
            <StatCard label="認得的人" value={data.totals.people} icon={Users} to={`${base}/people`}>
              會員 {num(data.totals.identified)}・匿名 {num(data.totals.anonymous)}
            </StatCard>
            <StatCard label="最近 7 天訪客" value={data.last_7_days.visitors} icon={Footprints}>
              <Trend current={data.last_7_days.visitors} previous={data.previous_7_days.visitors} />
            </StatCard>
            <StatCard label="最近 7 天瀏覽頁數" value={data.last_7_days.pageviews} icon={Eye}>
              <Trend current={data.last_7_days.pageviews} previous={data.previous_7_days.pageviews} />
            </StatCard>
            <StatCard label="最近 7 天認出的會員" value={data.last_7_days.identified} icon={UserCheck} to={`${base}/people?kind=member`}>
              <Trend current={data.last_7_days.identified} previous={data.previous_7_days.identified} />
            </StatCard>
          </div>
          <DailyChart daily={data.daily} />
          <div className="grid gap-4 lg:grid-cols-2">
            <section className="card">
              <CardHeader title="最多人看的頁面" hint="最近 30 天" actions={<More to={`${base}/pages`}>所有頁面</More>} />
              <BarList
                unit="人"
                empty="最近 30 天沒有人來。"
                rows={data.top_pages.map((p) => ({ key: p.path, label: p.title || p.path, hint: p.path, value: p.visitors }))}
              />
            </section>
            <section className="card">
              <CardHeader title="從哪裡來" hint="最近 30 天，從別的網站連進來的訪客" />
              <BarList
                unit="人"
                empty="最近 30 天沒有從別的網站連進來的訪客。"
                rows={data.top_referrers.map((r) => ({ key: r.host, label: r.host, value: r.visitors }))}
              />
            </section>
            <section className="card">
              <CardHeader title="標籤" hint="每個標籤現在有幾個人" actions={<More to={`${base}/labels`}>管理標籤</More>} />
              {data.labels.length === 0 ? (
                <p className="px-4 py-8 text-center text-sm text-zinc-500 sm:px-5 dark:text-zinc-400">
                  還沒有標籤。用標籤記下「看過定價頁」「加入購物車」這類行為。
                </p>
              ) : (
                <ul className="divide-y divide-zinc-100 dark:divide-zinc-800">
                  {data.labels.map((l) => (
                    <li key={l.id}>
                      <Link
                        to={`${base}/people?label_id=${l.id}`}
                        className="flex items-center justify-between gap-3 px-4 py-2.5 transition-colors hover:bg-zinc-50 sm:px-5 dark:hover:bg-zinc-800/50"
                      >
                        <Badge tone={l.color}>{l.name}</Badge>
                        <span className="text-sm tabular-nums">
                          {num(l.people)} <span className="text-xs text-zinc-500 dark:text-zinc-400">人</span>
                        </span>
                      </Link>
                    </li>
                  ))}
                </ul>
              )}
            </section>
            <section className="card">
              <CardHeader title="最近認出來的會員" actions={<More to={`${base}/people?kind=member`}>所有會員</More>} />
              {data.recent_identified.length === 0 ? (
                <p className="px-4 py-8 text-center text-sm text-zinc-500 sm:px-5 dark:text-zinc-400">
                  還沒有。訪客登入或留下 email 時呼叫 <code className="font-mono">cdp.identify()</code>，就知道是誰。
                </p>
              ) : (
                <ul className="divide-y divide-zinc-100 dark:divide-zinc-800">
                  {data.recent_identified.map((p) => (
                    <li key={p.id} className="flex items-center justify-between gap-3 px-4 py-2.5 sm:px-5">
                      <PersonLink siteId={site.id} person={p} />
                      <span className="shrink-0 text-xs text-zinc-500 dark:text-zinc-400">{timeAgo(p.identified_at)}</span>
                    </li>
                  ))}
                </ul>
              )}
            </section>
          </div>
        </div>
      )}
    </PageBody>
  );
}
