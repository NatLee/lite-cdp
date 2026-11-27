import { ArrowLeft, Plus, Trash2, Users, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { api } from "../api";
import { PageBody, useSite } from "../components/Layout";
import { CardHeader, LabelChips, PersonLink } from "../components/people";
import { Dialog, ErrorBanner, Loading, PageHeader, Spinner, useToast } from "../components/ui";
import { FIELDS, FIELD_LABEL, GROUPS, GROUP_LABEL, KIND_LABEL, num, timeAgo } from "../lib/format";
import { errorText, useAsync, useDebounced } from "../lib/hooks";
import type { Condition, ConditionField, Label, PersonKind, RuleGroup, SegmentPreview, SegmentRules } from "../types";

const KINDS: PersonKind[] = ["all", "member", "anonymous"];
const EMPTY: SegmentRules = { must: [], should: [], must_not: [] };
const TEXT_PLACEHOLDER: Partial<Record<ConditionField, string>> = { referrer: "google.com", email: "@example.com" };

/** 換欄位時給一個合理的預設值。 */
function blank(field: ConditionField, labels: Label[]): Condition {
  switch (field) {
    case "label":
      return { field, value: labels[0]?.id ?? 0 };
    case "referrer":
    case "email":
      return { field, value: "" };
    case "seen_within":
    case "first_seen_within":
      return { field, value: 30 };
    case "min_pageviews":
      return { field, value: 5 };
  }
}

/** 還沒填完的條件（沒選標籤、沒打字）先不送出去。 */
function complete(c: Condition): boolean {
  return typeof c.value === "number" ? c.value > 0 : c.value.trim() !== "";
}

function clean(rules: SegmentRules): SegmentRules {
  return { must: rules.must.filter(complete), should: rules.should.filter(complete), must_not: rules.must_not.filter(complete) };
}

function ConditionRow({
  label,
  condition,
  labels,
  onChange,
  onRemove,
}: {
  label: string;
  condition: Condition;
  labels: Label[];
  onChange: (c: Condition) => void;
  onRemove: () => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-2 sm:flex-nowrap">
      <select
        className="input w-full sm:w-52"
        aria-label={`${label}的欄位`}
        value={condition.field}
        onChange={(e) => onChange(blank(e.target.value as ConditionField, labels))}
      >
        {FIELDS.map((f) => (
          <option key={f} value={f} disabled={f === "label" && labels.length === 0}>
            {FIELD_LABEL[f]}
          </option>
        ))}
      </select>
      {condition.field === "label" ? (
        <select
          className="input min-w-0 flex-1"
          aria-label={`${label}的標籤`}
          value={condition.value}
          onChange={(e) => onChange({ field: "label", value: Number(e.target.value) })}
        >
          {labels.map((l) => (
            <option key={l.id} value={l.id}>
              {l.name}
            </option>
          ))}
        </select>
      ) : condition.field === "referrer" || condition.field === "email" ? (
        <input
          className="input min-w-0 flex-1"
          aria-label={`${label}的內容`}
          placeholder={TEXT_PLACEHOLDER[condition.field]}
          value={condition.value}
          onChange={(e) => onChange({ field: condition.field, value: e.target.value })}
        />
      ) : (
        <div className="flex min-w-0 flex-1 items-center gap-2">
          <input
            className="input w-28"
            type="number"
            min={1}
            aria-label={`${label}的數字`}
            value={condition.value}
            onChange={(e) => onChange({ field: condition.field, value: Math.max(1, Number(e.target.value) || 1) } as Condition)}
          />
          <span className="text-sm text-zinc-500 dark:text-zinc-400">{condition.field === "min_pageviews" ? "頁" : "天"}</span>
        </div>
      )}
      <button type="button" className="btn btn-ghost px-2" aria-label={`拿掉${label}`} onClick={onRemove}>
        <X className="size-4" />
      </button>
    </div>
  );
}

function Preview({ siteId, preview, error }: { siteId: number; preview?: SegmentPreview; error: string | null }) {
  return (
    <section className="card lg:sticky lg:top-6">
      <CardHeader title="現在符合的人" hint="改條件會馬上重算" />
      {error && (
        <div className="p-4">
          <ErrorBanner message={error} />
        </div>
      )}
      {!preview && !error && (
        <div className="px-4 sm:px-5">
          <Loading />
        </div>
      )}
      {preview && (
        <>
          <div className="px-4 py-4 sm:px-5">
            <p className="flex items-baseline gap-2" aria-live="polite">
              <Users className="size-4 self-center text-zinc-400" />
              <span className="text-3xl font-semibold tracking-tight tabular-nums" data-testid="preview-count">
                {num(preview.count)}
              </span>
              <span className="text-sm text-zinc-500 dark:text-zinc-400">
                人，全部 {num(preview.total)} 人的 {preview.total ? Math.round((preview.count / preview.total) * 100) : 0}%
              </span>
            </p>
            <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-zinc-100 dark:bg-zinc-800">
              <div
                className="h-full rounded-full bg-brand-500 transition-[width]"
                style={{ width: `${preview.total ? (preview.count / preview.total) * 100 : 0}%` }}
              />
            </div>
          </div>
          {preview.sample.length > 0 && (
            <ul className="divide-y divide-zinc-100 border-t border-zinc-100 dark:divide-zinc-800 dark:border-zinc-800">
              {preview.sample.map((p) => (
                <li key={p.id} className="flex items-center justify-between gap-3 px-4 py-3 sm:px-5">
                  <div className="min-w-0">
                    <PersonLink siteId={siteId} person={p} />
                    <div className="mt-1 pl-10">
                      <LabelChips labels={p.labels} max={3} />
                    </div>
                  </div>
                  <span className="shrink-0 text-xs text-zinc-500 dark:text-zinc-400">{timeAgo(p.last_seen)}</span>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </section>
  );
}

export function SegmentEditor() {
  const site = useSite();
  const base = `/s/${site.id}`;
  const param = useParams().segmentId;
  const segmentId = param === "new" ? null : Number(param);
  const navigate = useNavigate();
  const toast = useToast();

  const labels = useAsync(() => api.labels(site.id), [site.id]);
  const existing = useAsync(() => (segmentId ? api.segment(site.id, segmentId) : Promise.resolve(null)), [site.id, segmentId]);

  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [kind, setKind] = useState<PersonKind>("all");
  const [rules, setRules] = useState<SegmentRules>(EMPTY);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    const segment = existing.data;
    if (!segment) return;
    setName(segment.name);
    setDescription(segment.description);
    setKind(segment.kind);
    setRules(segment.rules);
  }, [existing.data]);

  // 停一下才重算，免得每打一個字就問一次
  const draft = useDebounced(JSON.stringify({ kind, rules: clean(rules) }), 300);
  const preview = useAsync(() => {
    const { kind, rules } = JSON.parse(draft) as { kind: PersonKind; rules: SegmentRules };
    return api.previewSegment(site.id, kind, rules);
  }, [site.id, draft]);

  const change = (group: RuleGroup, next: Condition[]) => setRules((current) => ({ ...current, [group]: next }));

  const save = async () => {
    setBusy(true);
    setError(null);
    const input = { name: name.trim(), description: description.trim(), kind, rules: clean(rules) };
    try {
      if (segmentId) {
        await api.updateSegment(site.id, segmentId, input);
        toast("分群存好了");
      } else {
        const created = await api.createSegment(site.id, input);
        toast(`建立了分群「${created.name}」`);
        navigate(`${base}/segments/${created.id}`, { replace: true });
      }
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!segmentId) return;
    setBusy(true);
    try {
      await api.deleteSegment(site.id, segmentId);
      toast("分群刪掉了");
      navigate(`${base}/segments`);
    } catch (e) {
      toast(errorText(e), "error");
      setBusy(false);
    }
  };

  const ready = segmentId === null || existing.data;

  return (
    <PageBody>
      <Link to={`${base}/segments`} className="mb-4 inline-flex items-center gap-1 text-sm text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100">
        <ArrowLeft className="size-4" /> 分群
      </Link>
      <PageHeader
        title={segmentId ? (existing.data?.name ?? "分群") : "新增分群"}
        subtitle="三組條件一起算：上面每一條都要符合，中間至少一條，下面的一條都不行。"
        actions={
          segmentId !== null && (
            <>
              <Link className="btn btn-secondary" to={`${base}/people?segment_id=${segmentId}`}>
                <Users className="size-4" /> 看名單
              </Link>
              <button className="btn btn-secondary" aria-label="刪除分群" onClick={() => setConfirming(true)}>
                <Trash2 className="size-4" />
              </button>
            </>
          )
        }
      />
      {existing.error && <ErrorBanner message={existing.error} />}
      {!ready && !existing.error && <Loading />}
      {ready && (
        <div className="grid items-start gap-4 lg:grid-cols-5">
          <div className="flex flex-col gap-4 lg:col-span-3">
            <section className="card flex flex-col gap-3 p-4 sm:p-5">
              <div>
                <label className="label" htmlFor="segment-name">
                  名稱
                </label>
                <input
                  id="segment-name"
                  className="input"
                  maxLength={60}
                  placeholder="例如：看過定價還沒註冊"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                />
              </div>
              <div>
                <label className="label" htmlFor="segment-description">
                  說明
                </label>
                <input
                  id="segment-description"
                  className="input"
                  maxLength={200}
                  placeholder="這群人要拿來做什麼，可以不填"
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                />
              </div>
              <div>
                <span className="label">對象</span>
                <div className="flex flex-wrap gap-1" role="radiogroup" aria-label="對象">
                  {KINDS.map((k) => (
                    <button
                      key={k}
                      type="button"
                      role="radio"
                      aria-checked={kind === k}
                      onClick={() => setKind(k)}
                      className={`rounded-lg border px-3 py-2 text-sm transition-colors ${
                        kind === k
                          ? "border-brand-600 bg-brand-600 font-semibold text-white dark:border-brand-500 dark:bg-brand-500 dark:text-zinc-950"
                          : "border-zinc-300 text-zinc-600 hover:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800"
                      }`}
                    >
                      {KIND_LABEL[k]}
                    </button>
                  ))}
                </div>
              </div>
            </section>

            {GROUPS.map((group) => (
              <section key={group} className="card" aria-label={GROUP_LABEL[group]}>
                <CardHeader
                  title={GROUP_LABEL[group]}
                  hint={group === "should" ? "沒有條件就不看這一組" : undefined}
                  actions={
                    <button
                      type="button"
                      className="btn btn-ghost px-2 py-1 text-xs"
                      disabled={rules[group].length >= 10}
                      onClick={() => change(group, [...rules[group], blank(labels.data?.length ? "label" : "seen_within", labels.data ?? [])])}
                    >
                      <Plus className="size-4" /> 加條件
                    </button>
                  }
                />
                {rules[group].length === 0 ? (
                  <p className="px-4 py-5 text-center text-sm text-zinc-500 sm:px-5 dark:text-zinc-400">沒有條件。</p>
                ) : (
                  <div className="flex flex-col gap-2 p-4 sm:p-5">
                    {rules[group].map((condition, index) => (
                      <ConditionRow
                        key={index}
                        label={`「${GROUP_LABEL[group]}」第 ${index + 1} 個條件`}
                        condition={condition}
                        labels={labels.data ?? []}
                        onChange={(c) => change(group, rules[group].map((old, i) => (i === index ? c : old)))}
                        onRemove={() => change(group, rules[group].filter((_, i) => i !== index))}
                      />
                    ))}
                  </div>
                )}
              </section>
            ))}

            {error && <ErrorBanner message={error} />}
            <div className="flex gap-2">
              <button className="btn btn-primary" disabled={busy || !name.trim()} onClick={() => void save()}>
                {busy && <Spinner />} {segmentId ? "儲存" : "建立分群"}
              </button>
              <Link className="btn btn-secondary" to={`${base}/segments`}>
                取消
              </Link>
            </div>
          </div>
          <div className="lg:col-span-2">
            <Preview siteId={site.id} preview={preview.data} error={preview.error} />
          </div>
        </div>
      )}
      <Dialog
        open={confirming}
        title="刪除這個分群？"
        onClose={() => setConfirming(false)}
        footer={
          <>
            <button className="btn btn-secondary" onClick={() => setConfirming(false)}>
              取消
            </button>
            <button className="btn btn-danger" disabled={busy} onClick={() => void remove()}>
              {busy && <Spinner />} 刪除
            </button>
          </>
        }
      >
        <p className="text-sm text-zinc-600 dark:text-zinc-300">只會刪掉這組條件，裡面的人和他們的紀錄都還在。</p>
      </Dialog>
    </PageBody>
  );
}
