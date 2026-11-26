import type { BadgeTone } from "./components/ui";

export type Role = "admin" | "member";

export interface Meta {
  version: string;
  /** 對外的網址，組追蹤程式碼用；沒設定是空字串 */
  public_url: string;
  /** 事件留幾天；0 是不會自動刪 */
  retention_days: number;
}

export interface User {
  id: number;
  username: string;
  display_name: string;
  role: Role;
}

export interface AuthState {
  /** 還沒有任何帳號：第一個進來的人要先建立管理員 */
  setup_required: boolean;
  user: User | null;
}

export interface Account extends User {
  disabled: boolean;
  created_at: string;
  /** 看得到幾個網站 */
  site_count: number;
}

export interface Member {
  id: number;
  username: string;
  display_name: string;
}

export interface Site {
  id: number;
  name: string;
  url: string;
  /** 追蹤程式碼裡的 data-key */
  key: string;
  allowed_origins: string[];
  active: boolean;
  created_at: string;
  /** 一共認得幾個人（含匿名） */
  people: number;
  /** 其中知道 email 的 */
  identified: number;
  last_event_at: string | null;
  members: Member[];
}

export interface SiteInput {
  name?: string;
  url?: string;
  allowed_origins?: string[];
  active?: boolean;
  member_ids?: number[];
}

export interface LabelChip {
  id: number;
  name: string;
  color: BadgeTone;
}

export type RuleKind = "path_prefix" | "url_contains" | "title_contains" | "event";

export interface Rule {
  kind: RuleKind;
  value: string;
}

export interface Label extends LabelChip {
  description: string;
  /** 符合幾次才算有這個標籤 */
  min_hits: number;
  rules: Rule[];
  people: number;
  created_at: string;
}

export interface LabelInput {
  name?: string;
  color?: BadgeTone;
  description?: string;
  min_hits?: number;
  rules?: Rule[];
}

export type PersonKind = "all" | "member" | "anonymous";

export interface Person {
  id: number;
  /** 匿名訪客是空字串 */
  email: string;
  name: string;
  account: string;
  pageviews: number;
  first_seen: string;
  last_seen: string;
  identified_at: string | null;
  labels: LabelChip[];
}

export type PeopleSort = "last_seen" | "first_seen" | "pageviews";

export type Activity = "recent" | "cooling" | "dormant";

export interface PeopleFilter {
  activity?: Activity;
  kind?: PersonKind;
  q?: string;
  label_id?: number;
  segment_id?: number;
  sort?: PeopleSort;
  page?: number;
  per_page?: number;
}

export interface PeoplePage {
  items: Person[];
  total: number;
  page: number;
  per_page: number;
}

export interface PersonDetail extends Omit<Person, "labels"> {
  traits: Record<string, string | number | boolean | null>;
  events: number;
  sessions: number;
  labels: (LabelChip & { hits: number; min_hits: number; last_at: string })[];
  devices: { id: number; browser: string; os: string; first_seen: string; last_seen: string }[];
  referrers: { host: string; visits: number }[];
  top_pages: { path: string; title: string; pageviews: number }[];
}

export type EventType = "pageview" | "click" | "scroll" | "custom" | "identify";

export interface TimelineEvent {
  id: number;
  type: EventType;
  name: string;
  url: string;
  path: string;
  title: string;
  referrer_host: string;
  props: Record<string, unknown>;
  session_id: string;
  created_at: string;
}

export interface EventsPage {
  items: TimelineEvent[];
  /** 還有更早的話，下一次帶這個當 `before` */
  next_before: number | null;
}

export interface PageRow {
  path: string;
  title: string;
  url: string;
  pageviews: number;
  visitors: number;
  last_at: string;
  labels: LabelChip[];
}

export type Condition =
  | { field: "label"; value: number }
  | { field: "referrer" | "email"; value: string }
  | { field: "seen_within" | "first_seen_within" | "min_pageviews"; value: number };

export type ConditionField = Condition["field"];
export type RuleGroup = "must" | "should" | "must_not";
export type SegmentRules = Record<RuleGroup, Condition[]>;

export interface Segment {
  id: number;
  name: string;
  description: string;
  kind: PersonKind;
  rules: SegmentRules;
  /** 現在符合的人數 */
  count: number;
  created_by_name: string;
  created_at: string;
  updated_at: string;
}

export interface SegmentInput {
  name?: string;
  description?: string;
  kind?: PersonKind;
  rules?: SegmentRules;
}

export interface SegmentPreview {
  count: number;
  /** 這個網站一共幾個人 */
  total: number;
  sample: Person[];
}

export interface Period {
  visitors: number;
  pageviews: number;
  new_people: number;
  identified: number;
}

export interface Overview {
  engagement: { activity: Activity; member: number; anonymous: number }[];
  totals: { people: number; identified: number; anonymous: number };
  last_7_days: Period;
  previous_7_days: Period;
  /** 最近 30 天，舊的在前 */
  daily: { date: string; pageviews: number; visitors: number }[];
  top_pages: { path: string; title: string; pageviews: number; visitors: number }[];
  top_referrers: { host: string; visitors: number }[];
  labels: (LabelChip & { people: number })[];
  recent_identified: { id: number; email: string; name: string; identified_at: string }[];
}
