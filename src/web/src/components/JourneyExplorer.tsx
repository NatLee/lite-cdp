import { ArrowRight, Crosshair, Navigation, Radio } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router";
import { api } from "../api";
import { EVENT_LABEL, dateTime, personName, timeAgo } from "../lib/format";
import { useAsync } from "../lib/hooks";
import type { TimelineEvent } from "../types";
import { ErrorBanner } from "./ui";

const clock = (value: string) =>
  new Date(value).toLocaleTimeString("zh-TW", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
const lane = (event: TimelineEvent) =>
  event.type === "pageview" ? 0 : event.type === "identify" ? 2 : 1;

/** Session IDs are recorded by the tracker. Missing IDs never join unrelated events. */
export function sessionGroups(events: TimelineEvent[]) {
  const groups = new Map<string, TimelineEvent[]>();
  for (const event of events) {
    const key = event.session_id
      ? `session:${event.session_id}`
      : `event:${event.id}`;
    groups.set(key, [...(groups.get(key) ?? []), event]);
  }
  return [...groups.entries()]
    .map(([id, items]) => ({
      id,
      items: items.sort(
        (a, b) => a.created_at.localeCompare(b.created_at) || a.id - b.id,
      ),
    }))
    .sort((a, b) =>
      b.items.at(-1)!.created_at.localeCompare(a.items.at(-1)!.created_at),
    );
}

export function JourneyMap({ events }: { events: TimelineEvent[] }) {
  const groups = sessionGroups(events);
  const [session, setSession] = useState("");
  const [focus, setFocus] = useState<number | null>(null);
  const current = groups.find((group) => group.id === session) ?? groups[0];
  const items = current?.items ?? [];
  const selected = items.find((event) => event.id === focus) ?? items[0];
  const width = Math.max(600, items.length * 118 + 30);
  const point = (event: TimelineEvent, i: number) => ({
    x: 72 + i * ((width - 144) / Math.max(1, items.length - 1)),
    y: 42 + lane(event) * 94,
  });
  if (!selected || !current)
    return <div className="journey-empty">這位訪客還沒有事件紀錄。</div>;
  return (
    <div className="journey-map">
      <div className="journey-controls">
        <label>
          <span>SESSION / 選擇一次來訪</span>
          <select
            aria-label="選擇來訪"
            value={current.id}
            onChange={(e) => {
              setSession(e.target.value);
              setFocus(null);
            }}
          >
            {groups.map((group) => (
              <option key={group.id} value={group.id}>
                {dateTime(group.items[0].created_at)} · {group.items.length}{" "}
                筆事件{group.items[0].session_id ? "" : " · 無來訪識別"}
              </option>
            ))}
          </select>
        </label>
        <div className="journey-session-count">
          <strong>{items.length}</strong>
          <span>
            筆已載入事件
            <br />
            <b>{groups.length}</b>{" "}
            {groups.some((group) => !group.items[0].session_id)
              ? " 組事件可選"
              : " 次來訪可選"}
          </span>
        </div>
      </div>
      <div className="journey-instrument">
        <div className="journey-lanes" aria-hidden="true">
          <span>
            PAGE<small>瀏覽</small>
          </span>
          <span>
            ACTION<small>互動</small>
          </span>
          <span>
            IDENTITY<small>辨識</small>
          </span>
        </div>
        <div
          className="journey-scroll"
          tabIndex={0}
          aria-label="事件航圖，可左右捲動"
        >
          <div className="journey-canvas" style={{ width }}>
            <svg viewBox={`0 0 ${width} 292`} aria-hidden="true">
              <defs>
                <linearGradient id="route-stroke">
                  <stop stopColor="#37c8e8" />
                  <stop offset="1" stopColor="#739aff" />
                </linearGradient>
              </defs>
              {[42, 136, 230].map((y) => (
                <line
                  key={y}
                  x1="0"
                  x2={width}
                  y1={y}
                  y2={y}
                  stroke="#29415c"
                  strokeDasharray="3 7"
                />
              ))}
              <polyline
                points={items
                  .map((e, i) => {
                    const p = point(e, i);
                    return `${p.x},${p.y}`;
                  })
                  .join(" ")}
                fill="none"
                stroke="url(#route-stroke)"
                strokeWidth="2"
              />
              {items.map((event, i) => {
                const p = point(event, i);
                return (
                  <circle
                    key={event.id}
                    cx={p.x}
                    cy={p.y}
                    r={selected.id === event.id ? 15 : 9}
                    fill="#0a1d33"
                    stroke={selected.id === event.id ? "#b6f5ff" : "#438eaf"}
                    strokeWidth="1"
                  />
                );
              })}
            </svg>
            {items.map((event, i) => {
              const p = point(event, i);
              return (
                <button
                  key={event.id}
                  className={`journey-point event-${event.type}`}
                  style={{ left: p.x, top: p.y }}
                  aria-pressed={selected.id === event.id}
                  aria-label={`${i + 1}. ${EVENT_LABEL[event.type]} ${event.type === "pageview" ? event.title || event.path : event.name || EVENT_LABEL[event.type]} ${clock(event.created_at)}`}
                  onClick={() => setFocus(event.id)}
                >
                  <span className="point-cross">
                    {String(i + 1).padStart(2, "0")}
                  </span>
                  <span className="point-label">
                    <time>{clock(event.created_at)}</time>
                    <b>
                      {event.type === "pageview"
                        ? event.title || event.path
                        : event.name || EVENT_LABEL[event.type]}
                    </b>
                    <small>{event.path || EVENT_LABEL[event.type]}</small>
                  </span>
                </button>
              );
            })}
          </div>
        </div>
      </div>
      <div className="journey-focus" aria-live="polite">
        <div className="focus-cross">
          <Crosshair size={22} />
          <span>EVENT FOCUS</span>
        </div>
        <div className="focus-description">
          <span>
            {EVENT_LABEL[selected.type]} · {dateTime(selected.created_at)}
          </span>
          <strong>
            {selected.type === "pageview"
              ? selected.title || selected.path
              : selected.name || EVENT_LABEL[selected.type]}
          </strong>
          <code>{selected.path || "未附頁面路徑"}</code>
        </div>
        <dl>
          <div>
            <dt>來源</dt>
            <dd>{selected.referrer_host || "未提供"}</dd>
          </div>
          <div>
            <dt>事件屬性</dt>
            <dd>
              {Object.entries(selected.props).length
                ? Object.entries(selected.props)
                    .map(
                      ([key, value]) =>
                        `${key}: ${typeof value === "object" ? JSON.stringify(value) : String(value)}`,
                    )
                    .join(" · ")
                : "無額外屬性"}
            </dd>
          </div>
        </dl>
      </div>
      <p className="journey-footnote">
        <span>
          <i /> 記錄的事件順序 · 節點等距，非時間比例
        </span>
        <span>僅顯示已載入事件，單次來訪可能尚未完整。</span>
      </p>
    </div>
  );
}

function VisitorJourney({
  siteId,
  personId,
}: {
  siteId: number;
  personId: number;
}) {
  const { data, error, reload } = useAsync(
    () => api.events(siteId, personId, { limit: 50 }),
    [siteId, personId],
  );
  if (error)
    return (
      <div className="journey-empty">
        <ErrorBanner message={error} />
        <button className="btn btn-secondary" onClick={() => void reload()}>
          重新載入旅程
        </button>
      </div>
    );
  if (!data)
    return (
      <div className="journey-empty" role="status">
        正在讀取這位訪客最近 50 筆事件…
      </div>
    );
  return <JourneyMap events={data.items} />;
}

export function JourneyExplorer({ siteId }: { siteId: number }) {
  const people = useAsync(
    () => api.people(siteId, { sort: "last_seen", per_page: 6 }),
    [siteId],
  );
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const selected =
    people.data?.items.find((p) => p.id === selectedId) ??
    people.data?.items[0];
  return (
    <section className="journey-observatory" aria-label="顧客行為航圖">
      <div className="observatory-heading">
        <div>
          <span className="eyebrow">JOURNEY OBSERVATORY / 顧客行為航圖</span>
          <h2>每一次來訪，都有跡可循。</h2>
          <p>選一位訪客，沿著真實事件，讀懂這次來訪。</p>
        </div>
        <span className="observatory-coordinate">
          <Navigation size={25} />
          <span>
            OBSERVE
            <br />
            THE JOURNEY
          </span>
        </span>
      </div>
      <div className="observatory-body">
        <aside className="visitor-selector">
          <div className="visitor-selector-title">
            <Radio size={13} />
            <span>最近出現 · 最多 6 位訪客</span>
          </div>
          {people.error && <ErrorBanner message={people.error} />}{" "}
          {!people.data && !people.error && <p role="status">正在讀取訪客…</p>}
          {people.data?.items.map((person, i) => (
            <button
              key={person.id}
              aria-pressed={selected?.id === person.id}
              onClick={() => setSelectedId(person.id)}
            >
              <span className="visitor-index">
                {String(i + 1).padStart(2, "0")}
              </span>
              <span>
                <b>{personName(person)}</b>
                <small>
                  {person.email ? "已辨識會員" : "匿名訪客"} ·{" "}
                  {timeAgo(person.last_seen)}
                </small>
              </span>
              <ArrowRight size={13} />
            </button>
          ))}
          <Link to={`/s/${siteId}/people`}>
            完整訪客名單 <ArrowRight size={13} />
          </Link>
        </aside>
        <div className="observatory-route">
          {selected ? (
            <>
              <div className="selected-visitor">
                <span>
                  正在觀測 <b>{personName(selected)}</b>
                </span>
                <Link to={`/s/${siteId}/people/${selected.id}`}>
                  開啟個人旅程 <ArrowRight size={13} />
                </Link>
              </div>
              <VisitorJourney
                key={`${siteId}:${selected.id}`}
                siteId={siteId}
                personId={selected.id}
              />
            </>
          ) : (
            <div className="journey-empty">
              收集到事件後，即可從這裡查看來訪路徑。
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
