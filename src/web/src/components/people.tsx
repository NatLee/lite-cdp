import {
  ChevronLeft,
  ChevronRight,
  TrendingDown,
  TrendingUp,
  type LucideIcon,
} from "lucide-react";
import type { ReactNode } from "react";
import { Link } from "react-router";
import { num, personName } from "../lib/format";
import type { LabelChip } from "../types";
import { Badge } from "./ui";

export function LabelChips({
  labels,
  max = 4,
}: {
  labels: LabelChip[];
  max?: number;
}) {
  if (labels.length === 0) return null;
  const shown = labels.slice(0, max);
  const rest = labels.slice(max);
  return (
    <span className="flex flex-wrap gap-1">
      {shown.map((l) => (
        <Badge key={l.id} tone={l.color}>
          {l.name}
        </Badge>
      ))}
      {rest.length > 0 && (
        <Badge title={rest.map((l) => l.name).join("、")}>+{rest.length}</Badge>
      )}
    </span>
  );
}

/** 圓形的頭像：會員用名字的第一個字，匿名訪客是問號。 */
export function Avatar({
  person,
  large,
}: {
  person: { email: string; name: string };
  large?: boolean;
}) {
  const known = person.email !== "";
  const letter = known
    ? (Array.from(person.name || person.email)[0] ?? "?").toUpperCase()
    : "?";
  return (
    <span
      aria-hidden
      className={`grid shrink-0 place-items-center rounded-full font-semibold ${large ? "size-12 text-lg" : "size-8 text-xs"} ${
        known
          ? "bg-brand-100 text-brand-700 dark:bg-brand-950 dark:text-brand-300"
          : "bg-zinc-100 text-zinc-400 dark:bg-zinc-800 dark:text-zinc-500"
      }`}
    >
      {letter}
    </span>
  );
}

/** 名單裡的一個人：頭像、名字，底下是 email（匿名訪客沒有）。 */
export function PersonLink({
  siteId,
  person,
}: {
  siteId: number;
  person: { id: number; email: string; name: string };
}) {
  return (
    <Link
      to={`/s/${siteId}/people/${person.id}`}
      className="group flex min-w-0 items-center gap-3"
    >
      <Avatar person={person} />
      <span className="min-w-0">
        <span className="block truncate text-sm font-semibold group-hover:text-brand-700 dark:group-hover:text-brand-300">
          {personName(person)}
        </span>
        {person.name && person.email && (
          <span className="block truncate text-xs text-zinc-500 dark:text-zinc-400">
            {person.email}
          </span>
        )}
      </span>
    </Link>
  );
}

export function StatCard({
  label,
  value,
  icon: Icon,
  to,
  children,
}: {
  label: string;
  value: number;
  icon: LucideIcon;
  to?: string;
  children?: ReactNode;
}) {
  const body = (
    <>
      <div className="flex items-center justify-between text-zinc-500 dark:text-zinc-400">
        <span className="text-xs font-semibold">{label}</span>
        <Icon className="size-4" />
      </div>
      <p className="mt-2 text-2xl font-semibold tracking-tight tabular-nums">
        {num(value)}
      </p>
      <p className="mt-1 min-h-4 text-xs text-zinc-500 dark:text-zinc-400">
        {children}
      </p>
    </>
  );
  return to ? (
    <Link
      to={to}
      className="atlas-stat card block p-4 transition-colors hover:border-brand-300 dark:hover:border-brand-700"
    >
      {body}
    </Link>
  ) : (
    <div className="atlas-stat card p-4">{body}</div>
  );
}

/** 這 7 天和前 7 天比。 */
export function Trend({
  current,
  previous,
}: {
  current: number;
  previous: number;
}) {
  if (previous === 0)
    return <>{current === 0 ? "前 7 天也是 0" : "前 7 天是 0"}</>;
  const change = Math.round(((current - previous) / previous) * 100);
  if (change === 0) return <>和前 7 天一樣</>;
  const Icon = change > 0 ? TrendingUp : TrendingDown;
  return (
    <span className="inline-flex items-center gap-1">
      <Icon className="size-4" />
      比前 7 天{change > 0 ? "多" : "少"} {Math.abs(change)}%
    </span>
  );
}

export function Pagination({
  page,
  pages,
  onChange,
}: {
  page: number;
  pages: number;
  onChange: (page: number) => void;
}) {
  if (pages <= 1) return null;
  return (
    <div className="flex items-center justify-between border-t border-zinc-100 px-3 py-2 text-sm dark:border-zinc-800">
      <span className="text-zinc-500 tabular-nums dark:text-zinc-400">
        第 {page} / {pages} 頁
      </span>
      <span className="flex gap-1">
        <button
          className="btn btn-secondary px-2"
          disabled={page <= 1}
          onClick={() => onChange(page - 1)}
        >
          <ChevronLeft className="size-4" /> 上一頁
        </button>
        <button
          className="btn btn-secondary px-2"
          disabled={page >= pages}
          onClick={() => onChange(page + 1)}
        >
          下一頁 <ChevronRight className="size-4" />
        </button>
      </span>
    </div>
  );
}

/** 卡片上方的標題列。 */
export function CardHeader({
  title,
  hint,
  actions,
}: {
  title: ReactNode;
  hint?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-2 border-b border-zinc-100 px-4 py-3 sm:px-5 dark:border-zinc-800">
      <div className="min-w-0">
        <h2 className="text-sm font-semibold">{title}</h2>
        {hint && (
          <p className="text-xs text-zinc-500 dark:text-zinc-400">{hint}</p>
        )}
      </div>
      {actions}
    </div>
  );
}

/** 橫條排行：每一列後面的長條照最大值的比例畫。 */
export function BarList({
  rows,
  unit,
  empty,
}: {
  rows: { key: string; label: ReactNode; hint?: string; value: number }[];
  unit: string;
  empty: string;
}) {
  if (rows.length === 0)
    return (
      <p className="px-4 py-8 text-center text-sm text-zinc-500 sm:px-5 dark:text-zinc-400">
        {empty}
      </p>
    );
  const max = Math.max(1, ...rows.map((r) => r.value));
  return (
    <ul className="flex flex-col gap-1 p-2 sm:p-3">
      {rows.map((r) => (
        <li
          key={r.key}
          className="relative overflow-hidden rounded-lg px-3 py-2"
        >
          <span
            aria-hidden
            className="absolute inset-y-0 left-0 rounded-lg bg-brand-50 dark:bg-brand-950/60"
            style={{ width: `${(r.value / max) * 100}%` }}
          />
          <span className="relative flex items-center justify-between gap-3 text-sm">
            <span className="min-w-0 truncate" title={r.hint}>
              {r.label}
            </span>
            <span className="shrink-0 text-xs text-zinc-600 tabular-nums dark:text-zinc-300">
              {num(r.value)} {unit}
            </span>
          </span>
        </li>
      ))}
    </ul>
  );
}
