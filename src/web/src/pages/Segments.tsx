import { Filter, Plus } from "lucide-react";
import { Link } from "react-router";
import { api } from "../api";
import { PageBody, useSite } from "../components/Layout";
import { Badge, Empty, ErrorBanner, Loading, PageHeader } from "../components/ui";
import { GROUPS, KIND_LABEL, describeCondition, num, timeAgo } from "../lib/format";
import { useAsync } from "../lib/hooks";
import type { RuleGroup } from "../types";

const GROUP_SHORT: Record<RuleGroup, string> = { must: "而且", should: "任一", must_not: "排除" };

export function Segments() {
  const site = useSite();
  const base = `/s/${site.id}`;
  const { data, error } = useAsync(() => api.segments(site.id), [site.id]);
  const labels = useAsync(() => api.labels(site.id), [site.id]);

  return (
    <PageBody>
      <PageHeader
        title="分群"
        subtitle="把條件存起來，隨時看現在有哪些人符合，匯出名單去寄信或投廣告。"
        actions={
          <Link className="btn btn-primary" to={`${base}/segments/new`}>
            <Plus className="size-4" /> 新增分群
          </Link>
        }
      />
      {error && <ErrorBanner message={error} />}
      {!data && !error && <Loading />}
      {data && data.length === 0 && (
        <div className="card">
          <Empty icon={Filter} title="還沒有分群">
            例如「看過定價、最近 14 天來過、還不是會員」。
          </Empty>
        </div>
      )}
      {data && data.length > 0 && (
        <div className="grid gap-4 lg:grid-cols-2">
          {data.map((segment) => (
            <article key={segment.id} className="card flex flex-col gap-3 p-4">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <h2 className="truncate font-semibold">
                    <Link className="hover:underline" to={`${base}/segments/${segment.id}`}>
                      {segment.name}
                    </Link>
                  </h2>
                  {segment.description && <p className="mt-0.5 text-sm text-zinc-500 dark:text-zinc-400">{segment.description}</p>}
                </div>
                <Link className="shrink-0 text-right" to={`${base}/people?segment_id=${segment.id}`} title="看這些人">
                  <span className="text-2xl font-semibold tracking-tight tabular-nums">{num(segment.count)}</span>
                  <span className="ml-1 text-xs text-zinc-500 dark:text-zinc-400">人</span>
                </Link>
              </div>
              <ul className="flex flex-wrap gap-1.5">
                <li>
                  <Badge tone="brand">{KIND_LABEL[segment.kind]}</Badge>
                </li>
                {GROUPS.flatMap((group) =>
                  segment.rules[group].map((condition, i) => (
                    <li key={`${group}-${i}`}>
                      <Badge tone={group === "must_not" ? "red" : "neutral"}>
                        {GROUP_SHORT[group]}・{describeCondition(condition, labels.data ?? [])}
                      </Badge>
                    </li>
                  )),
                )}
              </ul>
              <p className="mt-auto border-t border-zinc-100 pt-3 text-xs text-zinc-500 dark:border-zinc-800 dark:text-zinc-400">
                {segment.created_by_name || "已刪除的帳號"} 建立・{timeAgo(segment.updated_at)}更新
              </p>
            </article>
          ))}
        </div>
      )}
    </PageBody>
  );
}
