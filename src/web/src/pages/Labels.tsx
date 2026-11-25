import { Pencil, Plus, Tag, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useSearchParams } from "react-router";
import { api } from "../api";
import { PageBody, useSite } from "../components/Layout";
import { Badge, Dialog, Empty, ErrorBanner, Loading, PageHeader, Spinner, useToast, type BadgeTone } from "../components/ui";
import { COLORS, COLOR_LABEL, RULE_KINDS, RULE_LABEL, RULE_PLACEHOLDER, num } from "../lib/format";
import { errorText, useAsync } from "../lib/hooks";
import type { Label, Rule, RuleKind } from "../types";

/** 新增標籤，或（`label` 有值時）編輯。`path` 是從「頁面」帶過來、先填好的路徑。 */
function LabelDialog({
  siteId,
  open,
  label,
  path,
  onClose,
  onSaved,
}: {
  siteId: number;
  open: boolean;
  label?: Label;
  path?: string;
  onClose: () => void;
  onSaved: () => void;
}) {
  const toast = useToast();
  const [name, setName] = useState("");
  const [color, setColor] = useState<BadgeTone>("brand");
  const [description, setDescription] = useState("");
  const [minHits, setMinHits] = useState(1);
  const [rules, setRules] = useState<Rule[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setName(label?.name ?? "");
    setColor(label?.color ?? "brand");
    setDescription(label?.description ?? "");
    setMinHits(label?.min_hits ?? 1);
    setRules(label?.rules ?? [{ kind: "path_prefix", value: path ?? "" }]);
    setError(null);
  }, [open, label, path]);

  const setRule = (index: number, change: Partial<Rule>) =>
    setRules((current) => current.map((r, i) => (i === index ? { ...r, ...change } : r)));

  const filled = rules.filter((r) => r.value.trim());

  const save = async () => {
    setBusy(true);
    setError(null);
    const input = { name: name.trim(), color, description: description.trim(), min_hits: minHits, rules: filled };
    try {
      if (label) await api.updateLabel(siteId, label.id, input);
      else await api.createLabel(siteId, input);
      toast(label ? "標籤存好了，已經用現有的紀錄重算" : `建立了標籤「${input.name}」`);
      onSaved();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      wide
      title={label ? `編輯標籤「${label.name}」` : "新增標籤"}
      onClose={onClose}
      footer={
        <>
          <button className="btn btn-secondary" onClick={onClose}>
            取消
          </button>
          <button className="btn btn-primary" disabled={busy || !name.trim() || filled.length === 0} onClick={() => void save()}>
            {busy && <Spinner />} {label ? "儲存" : "建立"}
          </button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        <div className="grid gap-3 sm:grid-cols-[1fr_auto]">
          <div>
            <label className="label" htmlFor="label-name">
              名稱
            </label>
            <input
              id="label-name"
              className="input"
              maxLength={40}
              placeholder="例如：看過定價"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <div>
            <span className="label">顏色</span>
            <div className="flex h-9 items-center gap-1.5" role="radiogroup" aria-label="顏色">
              {COLORS.map((c) => (
                <button
                  key={c}
                  type="button"
                  role="radio"
                  aria-checked={color === c}
                  aria-label={COLOR_LABEL[c]}
                  onClick={() => setColor(c)}
                  className={`rounded-full ring-offset-2 ring-offset-white transition-shadow dark:ring-offset-zinc-900 ${
                    color === c ? "ring-2 ring-brand-500" : ""
                  }`}
                >
                  <Badge tone={c}>{COLOR_LABEL[c]}</Badge>
                </button>
              ))}
            </div>
          </div>
        </div>
        <div>
          <label className="label" htmlFor="label-description">
            說明
          </label>
          <input
            id="label-description"
            className="input"
            maxLength={200}
            placeholder="給同事看的，可以不填"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
        </div>
        <fieldset>
          <legend className="label">規則</legend>
          <p className="hint mt-0 mb-2">符合其中任何一條就算一次。英文不分大小寫。</p>
          <div className="flex flex-col gap-2">
            {rules.map((rule, index) => (
              <div key={index} className="flex flex-wrap items-center gap-2 sm:flex-nowrap">
                <select
                  className="input w-full sm:w-56"
                  aria-label={`規則 ${index + 1} 的種類`}
                  value={rule.kind}
                  onChange={(e) => setRule(index, { kind: e.target.value as RuleKind })}
                >
                  {RULE_KINDS.map((k) => (
                    <option key={k} value={k}>
                      {RULE_LABEL[k]}
                    </option>
                  ))}
                </select>
                <input
                  className="input min-w-0 flex-1 font-mono"
                  aria-label={`規則 ${index + 1} 的內容`}
                  placeholder={RULE_PLACEHOLDER[rule.kind]}
                  value={rule.value}
                  onChange={(e) => setRule(index, { value: e.target.value })}
                />
                <button
                  type="button"
                  className="btn btn-ghost px-2"
                  aria-label={`拿掉規則 ${index + 1}`}
                  disabled={rules.length === 1}
                  onClick={() => setRules((current) => current.filter((_, i) => i !== index))}
                >
                  <X className="size-4" />
                </button>
              </div>
            ))}
          </div>
          <button
            type="button"
            className="btn btn-ghost mt-2 px-2 text-xs"
            disabled={rules.length >= 20}
            onClick={() => setRules((current) => [...current, { kind: "path_prefix", value: "" }])}
          >
            <Plus className="size-3.5" /> 再加一條
          </button>
        </fieldset>
        <div>
          <label className="label" htmlFor="label-min-hits">
            符合幾次才算
          </label>
          <input
            id="label-min-hits"
            className="input w-28"
            type="number"
            min={1}
            max={1000}
            value={minHits}
            onChange={(e) => setMinHits(Math.max(1, Number(e.target.value) || 1))}
          />
          <p className="hint">設成 3，就是看了 3 次以上才有這個標籤；適合用來找「很有興趣」的人。</p>
        </div>
        {error && <ErrorBanner message={error} />}
      </div>
    </Dialog>
  );
}

export function Labels() {
  const site = useSite();
  const toast = useToast();
  const [params, setParams] = useSearchParams();
  const { data, error, reload } = useAsync(() => api.labels(site.id), [site.id]);
  const [dialog, setDialog] = useState<{ label?: Label; path?: string } | null>(null);
  const [removing, setRemoving] = useState<Label | null>(null);
  const [busy, setBusy] = useState(false);

  // 從「頁面」按「建標籤」過來：直接打開填好路徑的視窗
  const path = params.get("path");
  useEffect(() => {
    if (!path) return;
    setDialog({ path });
    setParams({}, { replace: true });
  }, [path]);

  const remove = async () => {
    if (!removing) return;
    setBusy(true);
    try {
      await api.deleteLabel(site.id, removing.id);
      toast(`刪掉了標籤「${removing.name}」`);
      setRemoving(null);
      await reload();
    } catch (e) {
      toast(errorText(e), "error");
    } finally {
      setBusy(false);
    }
  };

  return (
    <PageBody>
      <PageHeader
        title="標籤"
        subtitle="用規則描述「做過什麼事的人」。訪客的行為符合規則時自動貼上，不用改網站的程式。"
        actions={
          <button className="btn btn-primary" onClick={() => setDialog({})}>
            <Plus className="size-4" /> 新增標籤
          </button>
        }
      />
      {error && <ErrorBanner message={error} />}
      {!data && !error && <Loading />}
      {data && data.length === 0 && (
        <div className="card">
          <Empty icon={Tag} title="還沒有標籤">
            例如「看過 /pricing 底下的頁面」或「觸發過 add_to_cart 事件」。
          </Empty>
        </div>
      )}
      {data && data.length > 0 && (
        <div className="card overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b border-zinc-100 dark:border-zinc-800">
                <th className="th pl-4">標籤</th>
                <th className="th">規則</th>
                <th className="th text-right">門檻</th>
                <th className="th text-right">人數</th>
                <th className="th pr-4" />
              </tr>
            </thead>
            <tbody className="divide-y divide-zinc-100 dark:divide-zinc-800">
              {data.map((label) => (
                <tr key={label.id}>
                  <td className="td pl-4 align-top">
                    <Badge tone={label.color}>{label.name}</Badge>
                    {label.description && <p className="mt-1 max-w-64 text-xs text-zinc-500 dark:text-zinc-400">{label.description}</p>}
                  </td>
                  <td className="td align-top">
                    <ul className="flex flex-col gap-1">
                      {label.rules.map((rule, i) => (
                        <li key={i} className="text-xs text-zinc-600 dark:text-zinc-300">
                          {RULE_LABEL[rule.kind]}{" "}
                          <code className="rounded bg-zinc-100 px-1 py-0.5 font-mono dark:bg-zinc-800">{rule.value}</code>
                        </li>
                      ))}
                    </ul>
                  </td>
                  <td className="td text-right align-top whitespace-nowrap tabular-nums">{label.min_hits} 次</td>
                  <td className="td text-right align-top tabular-nums">
                    <Link className="text-brand-700 hover:underline dark:text-brand-300" to={`/s/${site.id}/people?label_id=${label.id}`}>
                      {num(label.people)}
                    </Link>
                  </td>
                  <td className="td pr-4 text-right align-top whitespace-nowrap">
                    <button className="btn btn-ghost px-2 py-1 text-xs" onClick={() => setDialog({ label })}>
                      <Pencil className="size-3.5" /> 編輯
                    </button>
                    <button className="btn btn-ghost px-2 py-1 text-xs" aria-label={`刪除 ${label.name}`} onClick={() => setRemoving(label)}>
                      <Trash2 className="size-3.5" />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <LabelDialog
        siteId={site.id}
        open={dialog !== null}
        label={dialog?.label}
        path={dialog?.path}
        onClose={() => setDialog(null)}
        onSaved={() => {
          setDialog(null);
          void reload();
        }}
      />
      <Dialog
        open={removing !== null}
        title={`刪除標籤「${removing?.name ?? ""}」？`}
        onClose={() => setRemoving(null)}
        footer={
          <>
            <button className="btn btn-secondary" onClick={() => setRemoving(null)}>
              取消
            </button>
            <button className="btn btn-danger" disabled={busy} onClick={() => void remove()}>
              {busy && <Spinner />} 刪除
            </button>
          </>
        }
      >
        <p className="text-sm text-zinc-600 dark:text-zinc-300">
          {num(removing?.people ?? 0)} 個人身上的這個標籤會一起拿掉；訪客的瀏覽紀錄不受影響。還有分群用到它的話要先改掉分群。
        </p>
      </Dialog>
    </PageBody>
  );
}
