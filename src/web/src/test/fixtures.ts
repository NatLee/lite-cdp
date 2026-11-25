import type { Account, Label, Meta, Overview, Person, PersonDetail, Segment, Site, User } from "../types";

export const meta: Meta = { version: "1.0.0", public_url: "https://cdp.example.com", retention_days: 0 };

export const admin: User = { id: 1, username: "nat", display_name: "Nat", role: "admin" };
export const member: User = { id: 2, username: "amy", display_name: "Amy", role: "member" };

export const accounts: Account[] = [
  { ...admin, disabled: false, created_at: "2026-01-01T00:00:00Z", site_count: 1 },
  { ...member, disabled: false, created_at: "2026-01-02T00:00:00Z", site_count: 0 },
];

export const site: Site = {
  id: 7,
  name: "官方網站",
  url: "https://www.example.com",
  key: "abcd1234abcd1234",
  allowed_origins: ["https://www.example.com"],
  active: true,
  created_at: "2026-01-01T00:00:00Z",
  people: 12,
  identified: 3,
  last_event_at: "2026-03-01T08:00:00Z",
  members: [],
};

export const label: Label = {
  id: 3,
  name: "看過定價",
  color: "amber",
  description: "",
  min_hits: 1,
  rules: [{ kind: "path_prefix", value: "/pricing" }],
  people: 5,
  created_at: "2026-01-05T00:00:00Z",
};

export function person(id: number, overrides: Partial<Person> = {}): Person {
  return {
    id,
    email: `visitor${id}@example.com`,
    name: `訪客 ${id}`,
    account: "",
    pageviews: id * 3,
    first_seen: "2026-02-01T08:00:00Z",
    last_seen: "2026-03-01T08:00:00Z",
    identified_at: "2026-02-02T08:00:00Z",
    labels: [],
    ...overrides,
  };
}

export const detail: PersonDetail = {
  ...person(5, { name: "王小明", email: "ming@example.com" }),
  traits: { plan: "pro" },
  events: 2,
  sessions: 1,
  labels: [{ id: 3, name: "看過定價", color: "amber", hits: 2, min_hits: 1, last_at: "2026-03-01T08:00:00Z" }],
  devices: [{ id: 1, browser: "Chrome", os: "macOS", first_seen: "2026-02-01T08:00:00Z", last_seen: "2026-03-01T08:00:00Z" }],
  referrers: [{ host: "www.google.com", visits: 1 }],
  top_pages: [{ path: "/pricing", title: "定價", pageviews: 2 }],
};

export const segment: Segment = {
  id: 9,
  name: "看過定價的會員",
  description: "",
  kind: "member",
  rules: { must: [{ field: "label", value: 3 }], should: [], must_not: [] },
  count: 4,
  created_by_name: "Nat",
  created_at: "2026-02-01T00:00:00Z",
  updated_at: "2026-02-01T00:00:00Z",
};

const period = { visitors: 0, pageviews: 0, new_people: 0, identified: 0 };

export const emptyOverview: Overview = {
  totals: { people: 0, identified: 0, anonymous: 0 },
  last_7_days: period,
  previous_7_days: period,
  daily: [],
  top_pages: [],
  top_referrers: [],
  labels: [],
  recent_identified: [],
};
