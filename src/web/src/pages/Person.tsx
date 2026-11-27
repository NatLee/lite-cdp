import {
  ArrowDownToLine,
  ArrowLeft,
  Eye,
  MousePointerClick,
  Trash2,
  UserCheck,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { api } from "../api";
import { JourneyMap } from "../components/JourneyExplorer";
import { PageBody, useSite } from "../components/Layout";
import { Avatar, BarList, CardHeader } from "../components/people";
import {
  Badge,
  Dialog,
  Empty,
  ErrorBanner,
  Loading,
  Spinner,
  useToast,
} from "../components/ui";
import { useUser } from "../context";
import { EVENT_LABEL, dateTime, num, personName, timeAgo } from "../lib/format";
import { errorText, useAsync } from "../lib/hooks";
import type { EventType, PersonDetail, TimelineEvent } from "../types";

const EVENT_ICON: Record<EventType, LucideIcon> = {
  pageview: Eye,
  click: MousePointerClick,
  scroll: ArrowDownToLine,
  custom: Zap,
  identify: UserCheck,
};

const EVENT_TYPES: EventType[] = [
  "pageview",
  "click",
  "scroll",
  "custom",
  "identify",
];

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-4 py-2 text-sm">
      <dt className="shrink-0 text-zinc-500 dark:text-zinc-400">{label}</dt>
      <dd className="min-w-0 text-right break-words">{children}</dd>
    </div>
  );
}

function Profile({ person }: { person: PersonDetail }) {
  const traits = Object.entries(person.traits);
  return (
    <section className="card">
      <CardHeader title="基本資料" />
      <dl className="divide-y divide-zinc-100 px-4 py-2 sm:px-5 dark:divide-zinc-800">
        {person.user_id && <Fact label="會員 ID">{person.user_id}</Fact>}
        {(person.user_id || person.email) && <Fact label="識別來源">{person.identity_verified ? "伺服器簽署會員 ID" : "網站宣告（未驗證登入）"}</Fact>}
        {person.email && <Fact label="Email">{person.email}</Fact>}
        {person.name && <Fact label="名稱">{person.name}</Fact>}
        {person.account && <Fact label="會員帳號">{person.account}</Fact>}
        <Fact label="第一次來">{dateTime(person.first_seen)}</Fact>
        <Fact label="最後一次來">{dateTime(person.last_seen)}</Fact>
        {person.identified_at && (
          <Fact label="認出來的時間">{dateTime(person.identified_at)}</Fact>
        )}
        <Fact label="瀏覽頁數">{num(person.pageviews)}</Fact>
        <Fact label="來訪次數">{num(person.sessions)}</Fact>
        {traits.map(([key, value]) => (
          <Fact key={key} label={key}>
            <span className="font-mono text-xs">{String(value)}</span>
          </Fact>
        ))}
      </dl>
    </section>
  );
}

function EventRow({ event }: { event: TimelineEvent }) {
  const Icon = EVENT_ICON[event.type];
  const props = Object.entries(event.props);
  let title: ReactNode;
  let detail: string;
  switch (event.type) {
    case "pageview":
      title = event.title || event.path;
      detail = event.referrer_host
        ? `${event.path}・從 ${event.referrer_host} 來`
        : event.path;
      break;
    case "identify":
      title = `認出來是 ${event.name}`;
      detail = "";
      break;
    case "scroll":
      title = `捲到 ${event.name}`;
      detail = event.path;
      break;
    default:
      title = event.name;
      detail = event.path;
  }
  return (
    <li className="flex gap-3 px-4 py-3 sm:px-5">
      <span
        className={`mt-1 grid size-7 shrink-0 place-items-center rounded-full ${
          event.type === "identify"
            ? "bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300"
            : event.type === "custom"
              ? "bg-amber-100 text-amber-700 dark:bg-amber-950 dark:text-amber-300"
              : "bg-zinc-100 text-zinc-500 dark:bg-zinc-800 dark:text-zinc-400"
        }`}
        title={EVENT_LABEL[event.type]}
      >
        <Icon className="size-4" />
      </span>
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm">
          {event.type !== "pageview" && event.type !== "identify" && (
            <span className="mr-2 text-xs text-zinc-500 dark:text-zinc-400">
              {EVENT_LABEL[event.type]}
            </span>
          )}
          <span className={event.type === "pageview" ? "" : "font-semibold"}>
            {title}
          </span>
        </p>
        {detail && (
          <p className="truncate font-mono text-xs text-zinc-500 dark:text-zinc-400">
            {detail}
          </p>
        )}
        {props.length > 0 && (
          <p className="mt-1 flex flex-wrap gap-1">
            {props.map(([key, value]) => (
              <span
                key={key}
                className="rounded bg-zinc-100 px-2 py-1 font-mono text-[11px] text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300"
              >
                {key}:{" "}
                {typeof value === "object"
                  ? JSON.stringify(value)
                  : String(value)}
              </span>
            ))}
          </p>
        )}
      </div>
      <time
        className="shrink-0 text-xs whitespace-nowrap text-zinc-500 tabular-nums dark:text-zinc-400"
        title={dateTime(event.created_at)}
      >
        {new Date(event.created_at).toLocaleTimeString("zh-TW", {
          hour: "2-digit",
          minute: "2-digit",
          hour12: false,
        })}
      </time>
    </li>
  );
}

/** 時間軸：新的在上面，一次拿 50 筆，按「更早的」再往前拿。 */
function Timeline({ siteId, personId }: { siteId: number; personId: number }) {
  const [type, setType] = useState<EventType | "">("");
  const [events, setEvents] = useState<TimelineEvent[] | null>(null);
  const [next, setNext] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = async (before?: number) => {
    setBusy(true);
    setError(null);
    try {
      const page = await api.events(siteId, personId, {
        before,
        type: type || undefined,
      });
      setEvents((current) =>
        before && current ? [...current, ...page.items] : page.items,
      );
      setNext(page.next_before);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    setEvents(null);
    void load();
    // 換了人或換了種類就從頭拿
  }, [siteId, personId, type]);

  // 同一天的排在一起
  const days: { day: string; items: TimelineEvent[] }[] = [];
  for (const event of events ?? []) {
    const day = new Date(event.created_at).toLocaleDateString("zh-TW", {
      dateStyle: "full",
    });
    if (days.at(-1)?.day === day) days.at(-1)!.items.push(event);
    else days.push({ day, items: [event] });
  }

  return (
    <section className="person-event-ledger card">
      <CardHeader
        title="做過的事"
        actions={
          <select
            className="input w-auto py-1 text-xs"
            aria-label="事件種類"
            value={type}
            onChange={(e) => setType(e.target.value as EventType | "")}
          >
            <option value="">所有事件</option>
            {EVENT_TYPES.map((t) => (
              <option key={t} value={t}>
                {EVENT_LABEL[t]}
              </option>
            ))}
          </select>
        }
      />
      {error && (
        <div className="p-4">
          <ErrorBanner message={error} />
        </div>
      )}
      {!events && !error && (
        <div className="px-4 sm:px-5">
          <Loading />
        </div>
      )}
      {events && events.length === 0 && (
        <Empty icon={Eye} title={type ? "沒有這種事件" : "還沒有紀錄"} />
      )}
      {events && events.length > 0 && (
        <JourneyMap key={`${siteId}:${personId}:${type}`} events={events} />
      )}
      {events && events.length > 0 && <h2 className="ledger-section-heading">04 / 完整時間軸 · 逐筆紀錄</h2>}
      {days.map(({ day, items }) => (
        <div key={day}>
          <h3 className="border-y border-zinc-100 bg-zinc-50 px-4 py-2 text-xs font-semibold text-zinc-500 first:border-t-0 sm:px-5 dark:border-zinc-800 dark:bg-zinc-800/40 dark:text-zinc-400">
            {day}
          </h3>
          <ul className="divide-y divide-zinc-100 dark:divide-zinc-800">
            {items.map((event) => (
              <EventRow key={event.id} event={event} />
            ))}
          </ul>
        </div>
      ))}
      {next !== null && (
        <div className="border-t border-zinc-100 p-3 text-center dark:border-zinc-800">
          <button
            className="btn btn-secondary"
            disabled={busy}
            onClick={() => void load(next)}
          >
            {busy && <Spinner />} 更早的
          </button>
        </div>
      )}
    </section>
  );
}

export function PersonPage() {
  const site = useSite();
  const admin = useUser().role === "admin";
  const personId = Number(useParams().personId);
  const navigate = useNavigate();
  const toast = useToast();
  const { data: person, error } = useAsync(
    () => api.person(site.id, personId),
    [site.id, personId],
  );
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  const remove = async () => {
    setBusy(true);
    try {
      await api.deletePerson(site.id, personId);
      toast("這個人的資料刪掉了");
      navigate(`/s/${site.id}/people`);
    } catch (e) {
      toast(errorText(e), "error");
      setBusy(false);
    }
  };

  return (
    <PageBody>
      <Link
        to={`/s/${site.id}/people`}
        className="mb-4 inline-flex items-center gap-1 text-sm text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100"
      >
        <ArrowLeft className="size-4" /> 名單
      </Link>
      {error && <ErrorBanner message={error} />}
      {!person && !error && <Loading />}
      {person && (
        <>
          <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
            <div className="flex min-w-0 items-center gap-3">
              <Avatar person={person} large />
              <div className="min-w-0">
                <h1 className="flex flex-wrap items-center gap-2 text-xl font-semibold tracking-tight">
                  <span className="truncate">{personName(person)}</span>
                  {(person.email || person.user_id) ? (
                    <Badge tone="green">會員</Badge>
                  ) : (
                    <Badge>匿名訪客</Badge>
                  )}
                </h1>
                <p className="mt-1 text-sm text-zinc-500 dark:text-zinc-400">
                  {person.name && person.email ? `${person.email}・` : ""}
                  最後一次來是{timeAgo(person.last_seen)}
                </p>
              </div>
            </div>
            {admin && (
              <button
                className="btn btn-secondary"
                onClick={() => setConfirming(true)}
              >
                <Trash2 className="size-4" /> 刪除資料
              </button>
            )}
          </div>

          <div className="person-journey-layout grid items-start gap-4 lg:grid-cols-5">
            <div className="flex flex-col gap-4 lg:col-span-2">
              <Profile person={person} />
              <section className="card">
                <CardHeader title="標籤" hint="符合規則的次數到了門檻才算有" />
                {person.labels.length === 0 ? (
                  <p className="px-4 py-6 text-center text-sm text-zinc-500 sm:px-5 dark:text-zinc-400">
                    還沒有符合任何標籤。
                  </p>
                ) : (
                  <ul className="divide-y divide-zinc-100 dark:divide-zinc-800">
                    {person.labels.map((l) => (
                      <li
                        key={l.id}
                        className={`flex items-center justify-between gap-3 px-4 py-3 sm:px-5 ${l.hits < l.min_hits ? "opacity-60" : ""}`}
                      >
                        <Badge tone={l.color}>{l.name}</Badge>
                        <span
                          className="text-xs text-zinc-500 tabular-nums dark:text-zinc-400"
                          title={`最近一次 ${dateTime(l.last_at)}`}
                        >
                          {l.hits < l.min_hits
                            ? `${l.hits} / ${l.min_hits} 次，還沒到門檻`
                            : `${num(l.hits)} 次・${timeAgo(l.last_at)}`}
                        </span>
                      </li>
                    ))}
                  </ul>
                )}
              </section>
              <section className="card">
                <CardHeader title="最常看的頁面" />
                <BarList
                  unit="次"
                  empty="還沒有看過任何頁面。"
                  rows={person.top_pages.map((p) => ({
                    key: p.path,
                    label: p.title || p.path,
                    hint: p.path,
                    value: p.pageviews,
                  }))}
                />
              </section>
              {person.referrers.length > 0 && (
                <section className="card">
                  <CardHeader title="從哪裡來" />
                  <BarList
                    unit="次"
                    empty=""
                    rows={person.referrers.map((r) => ({
                      key: r.host,
                      label: r.host,
                      value: r.visits,
                    }))}
                  />
                </section>
              )}
              <section className="card">
                <CardHeader
                  title="用過的裝置"
                  hint="同一個人在不同瀏覽器登入，會併在一起"
                />
                <ul className="divide-y divide-zinc-100 dark:divide-zinc-800">
                  {person.devices.map((d) => (
                    <li
                      key={d.id}
                      className="flex items-center justify-between gap-3 px-4 py-3 text-sm sm:px-5"
                    >
                      <span>
                        {d.browser || "不明的瀏覽器"}
                        <span className="text-zinc-500 dark:text-zinc-400">
                          ・{d.os || "不明的系統"}
                        </span>
                      </span>
                      <span
                        className="text-xs text-zinc-500 dark:text-zinc-400"
                        title={dateTime(d.last_seen)}
                      >
                        {timeAgo(d.last_seen)}
                      </span>
                    </li>
                  ))}
                </ul>
              </section>
            </div>
            <div className="lg:col-span-3">
              <Timeline siteId={site.id} personId={person.id} />
            </div>
          </div>

          <Dialog
            open={confirming}
            title="刪除這個人的資料？"
            onClose={() => setConfirming(false)}
            footer={
              <>
                <button
                  className="btn btn-secondary"
                  onClick={() => setConfirming(false)}
                >
                  取消
                </button>
                <button
                  className="btn btn-danger"
                  disabled={busy}
                  onClick={() => void remove()}
                >
                  {busy && <Spinner />} 刪除
                </button>
              </>
            }
          >
            <p className="text-sm text-zinc-600 dark:text-zinc-300">
              {personName(person)} 的基本資料、{num(person.events)}{" "}
              筆事件和裝置紀錄都會刪掉，沒辦法復原。
              對方要求刪除個人資料時用這個。之後同一個瀏覽器再來，會被當成新的訪客。
            </p>
          </Dialog>
        </>
      )}
    </PageBody>
  );
}
