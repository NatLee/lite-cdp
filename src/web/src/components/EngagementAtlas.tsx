import { ArrowRight, Radio, ScanLine } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router";
import type { Activity, Overview } from "../types";
import { num } from "../lib/format";

export const ACTIVITY_COLUMNS: {
  key: Activity;
  title: string;
  detail: string;
}[] = [
  { key: "recent", title: "近期活躍", detail: "7 天內來過" },
  { key: "cooling", title: "回訪降溫", detail: "7–30 天未回訪" },
  { key: "dormant", title: "久未回訪", detail: "超過 30 天未回訪" },
];
export function share(value: number, total: number) {
  return total > 0 ? `${((value / total) * 100).toFixed(1)}%` : "0.0%";
}

export function EngagementAtlas({
  data,
  base,
}: {
  data: Overview;
  base: string;
}) {
  const max = Math.max(
    1,
    ...data.engagement.flatMap((c) => [c.member, c.anonymous]),
  );
  const total = data.totals.people;
  return (
    <section className="engagement-atlas" aria-label="身分與回訪地圖">
      <div className="engagement-heading">
        <div>
          <span className="eyebrow">AUDIENCE OBSERVATORY / 01</span>
          <h2>把足跡，放回人的座標。</h2>
          <p>依最後一次造訪與目前身分，找到下一群值得了解的人。</p>
        </div>
        <Link to={`${base}/people`} className="audience-total">
          <span>累計訪客</span>
          <strong>{num(total)}</strong>
          <span>
            探索全部名單 <ArrowRight size={16} />
          </span>
        </Link>
      </div>
      <div className="cohort-map">
        <div className="cohort-corner">
          <ScanLine size={20} />
          <span>身分 × 回訪</span>
        </div>
        {ACTIVITY_COLUMNS.map((c) => (
          <div className={`cohort-column column-${c.key}`} key={c.key}>
            <span className="cohort-beacon" />
            <b>{c.title}</b>
            <small>{c.detail}</small>
          </div>
        ))}
        {(["member", "anonymous"] as const).map((kind) => (
          <div className="cohort-row" key={kind}>
            <div className="cohort-row-title">
              <span>{kind === "member" ? "IDENTIFIED" : "ANONYMOUS"}</span>
              <b>{kind === "member" ? "已辨識會員" : "匿名訪客"}</b>
              <small>
                {kind === "member" ? "知道 email 的人" : "尚未提供 email"}
              </small>
            </div>
            {ACTIVITY_COLUMNS.map((c) => {
              const count =
                data.engagement.find((v) => v.activity === c.key)?.[kind] ?? 0;
              return (
                <Link
                  key={c.key}
                  className={`cohort-cell cohort-${kind} cohort-${c.key}`}
                  to={`${base}/people?kind=${kind}&activity=${c.key}`}
                  aria-label={`${c.title}的${kind === "member" ? "會員" : "匿名訪客"}，${count} 人，查看名單`}
                >
                  <svg viewBox="0 0 160 116" aria-hidden="true">
                    <path
                      d="M80 4v108M24 58h112"
                      stroke="currentColor"
                      strokeOpacity=".12"
                      strokeDasharray="3 4"
                    />
                    <circle
                      cx="80"
                      cy="58"
                      r="50"
                      fill="none"
                      stroke="currentColor"
                      strokeOpacity=".1"
                    />
                    <circle
                      className="cohort-bubble"
                      cx="80"
                      cy="58"
                      r={Math.sqrt(count / max) * 49}
                    />
                  </svg>
                  <span className="cohort-count">
                    <strong>{num(count)}</strong>
                    <small>占全部 {share(count, total)}</small>
                  </span>
                  <span className="cohort-open">
                    查看名單 <ArrowRight size={16} />
                  </span>
                </Link>
              );
            })}
          </div>
        ))}
      </div>
      <div className="cohort-notes">
        <span>
          <Radio size={16} />
          圓面積代表人數；每人只在一格。
        </span>
        <span>目前狀態分布，不代表轉換率。時間以連續 24 小時計算。</span>
      </div>
    </section>
  );
}

export function TrafficExplorer({ daily }: { daily: Overview["daily"] }) {
  const [metric, setMetric] = useState<"pageviews" | "visitors">("pageviews");
  const [selected, setSelected] = useState<number | null>(null);
  const index = Math.min(selected ?? daily.length - 1, daily.length - 1);
  const day = daily[index];
  const max = Math.ceil(Math.max(2, ...daily.map((d) => d[metric])) / 2) * 2;
  const x = (i: number) =>
    daily.length > 1 ? 36 + (i / (daily.length - 1)) * 852 : 462;
  const y = (v: number) => 184 - (v / max) * 156;
  const path = daily
    .map((d, i) => `${i === 0 ? "M" : "L"}${x(i)} ${y(d[metric])}`)
    .join(" ");
  const peak = daily.reduce<Overview["daily"][number] | undefined>(
    (best, d) => (!best || d[metric] > best[metric] ? d : best),
    undefined,
  );
  return (
    <section className="traffic-explorer card">
      <div className="traffic-heading">
        <div>
          <span className="eyebrow">TRAFFIC PULSE / 02</span>
          <h3>每一天的來訪訊號</h3>
          <p>最近 30 天 · 依瀏覽器時區分日</p>
        </div>
        <div className="traffic-switch" aria-label="圖表指標">
          {(["pageviews", "visitors"] as const).map((key) => (
            <button
              key={key}
              aria-pressed={metric === key}
              onClick={() => setMetric(key)}
            >
              {key === "pageviews" ? "瀏覽頁數" : "每日訪客"}
            </button>
          ))}
        </div>
      </div>
      {day ? (
        <>
          <div className="traffic-readout" aria-live="polite">
            <span>
              {day.date}{" "}
              <small>{index === daily.length - 1 ? "今天" : "選取日期"}</small>
            </span>
            <b>
              {num(day[metric])}
              <small>{metric === "pageviews" ? "頁瀏覽" : "位訪客"}</small>
            </b>
            <span>
              {metric === "pageviews"
                ? `${num(day.visitors)} 位訪客`
                : `瀏覽 ${num(day.pageviews)} 頁`}
            </span>
          </div>
          <div className="traffic-plot">
            <div className="traffic-y-axis" aria-hidden="true">
              {[0, 0.5, 1].map((r) => (
                <span key={r} style={{ top: `${(y(max * r) / 210) * 100}%` }}>
                  {num(max * r)}
                </span>
              ))}
            </div>
            <svg
              viewBox="0 0 924 210"
              preserveAspectRatio="none"
              aria-hidden="true"
            >
              <defs>
                <linearGradient id="traffic-fill" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0" stopColor="var(--atlas-accent)" stopOpacity=".28" />
                  <stop offset="1" stopColor="var(--atlas-accent)" stopOpacity=".02" />
                </linearGradient>
              </defs>
              {[0, 0.5, 1].map((r) => (
                <g key={r}>
                  <line
                    x1="36"
                    x2="888"
                    y1={y(max * r)}
                    y2={y(max * r)}
                    stroke="currentColor"
                    strokeOpacity=".13"
                    strokeDasharray="3 5"
                  />
                </g>
              ))}
              <path
                d={`${path} L${x(daily.length - 1)} 184 L${x(0)} 184 Z`}
                fill="url(#traffic-fill)"
              />
              <path
                d={path}
                fill="none"
                stroke="var(--atlas-accent)"
                strokeWidth="2.5"
                vectorEffect="non-scaling-stroke"
              />
              <line
                x1={x(index)}
                x2={x(index)}
                y1="16"
                y2="184"
                stroke="var(--atlas-accent)"
                strokeDasharray="4 4"
              />
              <circle
                cx={x(index)}
                cy={y(day[metric])}
                r="5"
                fill="var(--atlas-accent)"
                stroke="var(--atlas-paper)"
                strokeWidth="2"
              />
            </svg>
            <div className="traffic-targets">
              {daily.map((d, i) => (
                <button
                  key={d.date}
                  aria-label={`${d.date}，${num(d.pageviews)} 頁，${num(d.visitors)} 位訪客`}
                  aria-pressed={index === i}
                  onFocus={() => setSelected(i)}
                  onClick={() => setSelected(i)}
                  title={`${d.date} · ${d[metric]}`}
                >
                  <span />
                </button>
              ))}
            </div>
          </div>
          <div className="traffic-axis">
            <span>{daily[0].date.slice(5)}</span>
            <span>拖曳或用左右鍵探索日期</span>
            <span>{daily.at(-1)?.date.slice(5)}</span>
          </div>
          <input
            className="traffic-range"
            type="range"
            aria-label="圖表日期"
            aria-valuetext={day.date}
            min={0}
            max={daily.length - 1}
            value={index}
            onChange={(e) => setSelected(Number(e.target.value))}
          />
          <div className="traffic-note">
            <span>
              最高單日：{peak?.date} · {num(peak?.[metric] ?? 0)}{" "}
              {metric === "pageviews" ? "頁" : "人"}
            </span>
            <span>
              {metric === "visitors"
                ? "同一天每人計一次；跨日人數不可相加為獨立訪客。"
                : "僅計 pageview 事件，不含自訂事件。"}
            </span>
          </div>
        </>
      ) : (
        <p className="traffic-empty">
          還沒有每日資料。收到追蹤事件後，這裡會顯示來訪趨勢。
        </p>
      )}
    </section>
  );
}
