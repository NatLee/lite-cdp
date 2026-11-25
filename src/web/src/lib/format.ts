import type { BadgeTone } from "../components/ui";
import type { Condition, ConditionField, EventType, LabelChip, PersonKind, Role, RuleGroup, RuleKind } from "../types";

export function dateTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("zh-TW", { dateStyle: "medium", timeStyle: "short", hour12: false });
}

/** 「3 天前」這種寫法；超過一個月就給日期。 */
export function timeAgo(iso: string, now = Date.now()): string {
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return iso;
  const minutes = Math.floor((now - t) / 60_000);
  if (minutes < 1) return "剛剛";
  if (minutes < 60) return `${minutes} 分鐘前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小時前`;
  const days = Math.floor(hours / 24);
  if (days < 31) return `${days} 天前`;
  return new Date(iso).toLocaleDateString("zh-TW", { dateStyle: "medium" });
}

export const num = (n: number) => n.toLocaleString("en-US");

export const ROLE_LABEL: Record<Role, string> = { admin: "管理員", member: "成員" };

export const KIND_LABEL: Record<PersonKind, string> = { all: "所有人", member: "會員", anonymous: "匿名訪客" };

/** 知道 email 的人顯示名字或 email；匿名訪客只有編號。 */
export function personName(p: { id: number; email: string; name: string }): string {
  return p.name || p.email || `匿名訪客 #${p.id}`;
}

export const COLORS: BadgeTone[] = ["neutral", "brand", "green", "amber", "red", "sky", "violet", "pink"];

export const COLOR_LABEL: Record<BadgeTone, string> = {
  neutral: "灰",
  brand: "靛",
  green: "綠",
  amber: "黃",
  red: "紅",
  sky: "藍",
  violet: "紫",
  pink: "粉",
};

export const RULE_KINDS: RuleKind[] = ["path_prefix", "url_contains", "title_contains", "event"];

export const RULE_LABEL: Record<RuleKind, string> = {
  path_prefix: "看過這個路徑底下的頁面",
  url_contains: "看過網址含有",
  title_contains: "看過標題含有",
  event: "觸發過事件",
};

export const RULE_PLACEHOLDER: Record<RuleKind, string> = {
  path_prefix: "/pricing",
  url_contains: "utm_campaign=spring",
  title_contains: "企業方案",
  event: "add_to_cart",
};

export const EVENT_LABEL: Record<EventType, string> = {
  pageview: "瀏覽頁面",
  click: "點擊",
  scroll: "捲動",
  custom: "自訂事件",
  identify: "認出身分",
};

export const GROUPS: RuleGroup[] = ["must", "should", "must_not"];

export const GROUP_LABEL: Record<RuleGroup, string> = {
  must: "每一條都要符合",
  should: "至少符合其中一條",
  must_not: "一條都不能符合",
};

export const FIELDS: ConditionField[] = ["label", "referrer", "email", "seen_within", "first_seen_within", "min_pageviews"];

export const FIELD_LABEL: Record<ConditionField, string> = {
  label: "有這個標籤",
  referrer: "來源網域含有",
  email: "Email 含有",
  seen_within: "最近幾天內來過",
  first_seen_within: "最近幾天內第一次來",
  min_pageviews: "至少看過幾頁",
};

/** 把一個條件寫成一句話；`labels` 用來把標籤編號換成名字。 */
export function describeCondition(c: Condition, labels: LabelChip[]): string {
  switch (c.field) {
    case "label":
      return `有「${labels.find((l) => l.id === c.value)?.name ?? `標籤 #${c.value}`}」`;
    case "referrer":
      return `從 ${c.value} 來過`;
    case "email":
      return `Email 含 ${c.value}`;
    case "seen_within":
      return `${c.value} 天內來過`;
    case "first_seen_within":
      return `${c.value} 天內第一次來`;
    case "min_pageviews":
      return `看過 ${c.value} 頁以上`;
  }
}

/** 把一行一個（或用逗號隔開）的輸入拆成清單。 */
export function parseLines(input: string): string[] {
  const items: string[] = [];
  for (const raw of input.split(/[\n,，]+/)) {
    const item = raw.trim();
    if (item && !items.includes(item)) items.push(item);
  }
  return items;
}
