import {
  ArrowRight,
  Code2,
  Eye,
  Footprints,
  UserCheck,
  Users,
} from "lucide-react";
import { Link } from "react-router";
import { api } from "../api";
import { JourneyExplorer } from "../components/JourneyExplorer";
import { PageBody, useSite } from "../components/Layout";
import {
  BarList,
  CardHeader,
  PersonLink,
  StatCard,
  Trend,
} from "../components/people";
import { Badge, Empty, ErrorBanner, PageHeader } from "../components/ui";
import { num, timeAgo } from "../lib/format";
import { useAsync } from "../lib/hooks";
import {
  EngagementAtlas,
  TrafficExplorer,
} from "../components/EngagementAtlas";

function More({ to, children }: { to: string; children: string }) {
  return (
    <Link
      to={to}
      className="inline-flex items-center gap-1 text-xs text-brand-700 hover:underline dark:text-brand-300"
    >
      {children} <ArrowRight className="size-3" />
    </Link>
  );
}

export function OverviewPage() {
  const site = useSite();
  const base = `/s/${site.id}`;
  const { data, error, reload } = useAsync(
    () => api.overview(site.id),
    [site.id],
  );

  return (
    <PageBody>
      <PageHeader
        title={site.name}
        subtitle={site.url || "誰來過、看了什麼、哪些人值得聯絡。"}
        actions={!site.active && <Badge tone="amber">已停止收資料</Badge>}
      />
      {error && (
        <>
          <ErrorBanner message={error} />
          <button
            className="btn btn-secondary mb-4"
            onClick={() => void reload()}
          >
            重新載入總覽
          </button>
        </>
      )}
      {!data && !error && (
        <div role="status" className="atlas-loading">
          <span>正在整理訪客分布與每日訊號…</span>
          <div />
          <div />
          <div />
        </div>
      )}
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
          <JourneyExplorer key={site.id} siteId={site.id} />
          <div className="atlas-metrics grid grid-cols-2 gap-4 lg:grid-cols-4">
            <StatCard
              label="累計訪客"
              value={data.totals.people}
              icon={Users}
              to={`${base}/people`}
            >
              會員 {num(data.totals.identified)}・匿名{" "}
              {num(data.totals.anonymous)}
            </StatCard>
            <StatCard
              label="最近 7 天訪客"
              value={data.last_7_days.visitors}
              icon={Footprints}
            >
              <Trend
                current={data.last_7_days.visitors}
                previous={data.previous_7_days.visitors}
              />
            </StatCard>
            <StatCard
              label="最近 7 天瀏覽頁數"
              value={data.last_7_days.pageviews}
              icon={Eye}
            >
              <Trend
                current={data.last_7_days.pageviews}
                previous={data.previous_7_days.pageviews}
              />
            </StatCard>
            <StatCard
              label="最近 7 天認出的會員"
              value={data.last_7_days.identified}
              icon={UserCheck}
              to={`${base}/people?kind=member`}
            >
              <Trend
                current={data.last_7_days.identified}
                previous={data.previous_7_days.identified}
              />
            </StatCard>
          </div>
          <div className="observatory-section-title">
            <span>02 / AUDIENCE & SIGNAL</span>
            <h2>從一段旅程，到整體樣貌。</h2>
            <p>目前的訪客分布，以及最近 30 天的流量。</p>
          </div>
          <TrafficExplorer daily={data.daily} />
          <EngagementAtlas data={data} base={base} />
          <div className="atlas-insights grid grid-cols-1 gap-4 lg:grid-cols-2">
            <section className="card">
              <CardHeader
                title="最多人看的頁面"
                hint="最近 30 天"
                actions={<More to={`${base}/pages`}>所有頁面</More>}
              />
              <BarList
                unit="人"
                empty="最近 30 天沒有人來。"
                rows={data.top_pages.map((p) => ({
                  key: p.path,
                  label: p.title || p.path,
                  hint: p.path,
                  value: p.visitors,
                }))}
              />
            </section>
            <section className="card">
              <CardHeader
                title="從哪裡來"
                hint="最近 30 天，從別的網站連進來的訪客"
              />
              <BarList
                unit="人"
                empty="最近 30 天沒有從別的網站連進來的訪客。"
                rows={data.top_referrers.map((r) => ({
                  key: r.host,
                  label: r.host,
                  value: r.visitors,
                }))}
              />
            </section>
            <section className="card">
              <CardHeader
                title="標籤"
                hint="每個標籤現在有幾個人"
                actions={<More to={`${base}/labels`}>管理標籤</More>}
              />
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
                          {num(l.people)}{" "}
                          <span className="text-xs text-zinc-500 dark:text-zinc-400">
                            人
                          </span>
                        </span>
                      </Link>
                    </li>
                  ))}
                </ul>
              )}
            </section>
            <section className="card">
              <CardHeader
                title="最近認出來的會員"
                actions={
                  <More to={`${base}/people?kind=member`}>所有會員</More>
                }
              />
              {data.recent_identified.length === 0 ? (
                <p className="px-4 py-8 text-center text-sm text-zinc-500 sm:px-5 dark:text-zinc-400">
                  還沒有。訪客登入或留下 email 時呼叫{" "}
                  <code className="font-mono">cdp.identify()</code>
                  ，就知道是誰。
                </p>
              ) : (
                <ul className="divide-y divide-zinc-100 dark:divide-zinc-800">
                  {data.recent_identified.map((p) => (
                    <li
                      key={p.id}
                      className="flex items-center justify-between gap-3 px-4 py-2.5 sm:px-5"
                    >
                      <PersonLink siteId={site.id} person={p} />
                      <span className="shrink-0 text-xs text-zinc-500 dark:text-zinc-400">
                        {timeAgo(p.identified_at)}
                      </span>
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
