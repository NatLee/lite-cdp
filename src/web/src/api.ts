import type {
  Account,
  AuthState,
  EventType,
  EventsPage,
  Label,
  LabelInput,
  Meta,
  Overview,
  PageRow,
  PeopleFilter,
  PeoplePage,
  PersonDetail,
  PersonKind,
  Role,
  Segment,
  SegmentInput,
  SegmentPreview,
  SegmentRules,
  Site,
  SiteInput,
  User,
} from "./types";

/** session 過期或沒登入會收到 401；App 聽到這個事件就回到登入頁。 */
export const UNAUTHORIZED_EVENT = "cdp:unauthorized";

/** 後端回 `{"error": "訊息"}` 時丟出這個，訊息直接給使用者看。 */
export class ApiError extends Error {
  readonly status: number;
  /** 完整的回應內容 */
  readonly data: unknown;
  constructor(status: number, message: string, data: unknown) {
    super(message);
    this.status = status;
    this.data = data;
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const init: RequestInit = { method, headers: {} };
  if (body !== undefined) {
    init.body = JSON.stringify(body);
    (init.headers as Record<string, string>)["content-type"] = "application/json";
  }
  const res = await fetch(`/api${path}`, init);
  if (res.status === 204) return undefined as T;
  const text = await res.text();
  let data: unknown = undefined;
  try {
    data = text ? JSON.parse(text) : undefined;
  } catch {
    data = text;
  }
  if (res.status === 401 && !path.startsWith("/auth/")) {
    window.dispatchEvent(new Event(UNAUTHORIZED_EVENT));
  }
  if (!res.ok) {
    const message =
      typeof data === "object" && data !== null && "error" in data
        ? String((data as { error: unknown }).error)
        : `伺服器回應 ${res.status}`;
    throw new ApiError(res.status, message, data);
  }
  return data as T;
}

const get = <T>(path: string) => request<T>("GET", path);
const post = <T>(path: string, body?: unknown) => request<T>("POST", path, body ?? {});
const patch = <T>(path: string, body: unknown) => request<T>("PATCH", path, body);
const del = <T = void>(path: string) => request<T>("DELETE", path);

function query(params: object): string {
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== null && v !== "") q.set(k, String(v));
  }
  const s = q.toString();
  return s ? `?${s}` : "";
}

/** 瀏覽器的時區比 UTC 快幾分鐘（台北是 480）；每天的統計照這個切日。 */
const tzOffset = () => -new Date().getTimezoneOffset();

export const api = {
  meta: () => get<Meta>("/meta"),

  authState: () => get<AuthState>("/auth/state"),
  // 顯示名稱沒填就不送，後端會用帳號當名字
  setup: (input: { username: string; display_name: string; password: string }) =>
    post<User>("/auth/setup", { ...input, display_name: input.display_name || undefined }),
  login: (username: string, password: string) => post<User>("/auth/login", { username, password }),
  logout: () => post<void>("/auth/logout"),
  updateMe: (input: { display_name: string }) => patch<User>("/auth/me", input),
  changePassword: (currentPassword: string, newPassword: string) =>
    post<void>("/auth/password", { current_password: currentPassword, new_password: newPassword }),

  users: () => get<Account[]>("/users"),
  createUser: (input: { username: string; display_name: string; password: string; role: Role }) =>
    post<Account>("/users", { ...input, display_name: input.display_name || undefined }),
  updateUser: (id: number, input: { display_name?: string; role?: Role; disabled?: boolean; password?: string }) =>
    patch<Account>(`/users/${id}`, input),

  sites: () => get<Site[]>("/sites"),
  createSite: (input: SiteInput) => post<Site>("/sites", input),
  updateSite: (id: number, input: SiteInput) => patch<Site>(`/sites/${id}`, input),
  deleteSite: (id: number) => del(`/sites/${id}`),
  createInstallationTest: (id: number) => post<{token:string;expires_at:string}>(`/sites/${id}/installation-tests`),
  installationReceipt: (id:number, token:string) => get<{expired:boolean;received_at:string|null;receipt:null|{events:Record<string,unknown>[];origin:string;analytics_written:boolean;identity:unknown;bot:boolean}}>(`/sites/${id}/installation-tests/${encodeURIComponent(token)}`),
  rotateKey: (id: number) => post<Site>(`/sites/${id}/rotate-key`),

  overview: (site: number) => get<Overview>(`/sites/${site}/overview${query({ tz_offset: tzOffset() })}`),

  people: (site: number, filter: PeopleFilter = {}) => get<PeoplePage>(`/sites/${site}/people${query(filter)}`),
  person: (site: number, id: number) => get<PersonDetail>(`/sites/${site}/people/${id}`),
  deletePerson: (site: number, id: number) => del(`/sites/${site}/people/${id}`),
  events: (site: number, id: number, options: { before?: number; type?: EventType; limit?: number } = {}) =>
    get<EventsPage>(`/sites/${site}/people/${id}/events${query(options)}`),
  exportUrl: (site: number, filter: PeopleFilter) =>
    `/api/sites/${site}/people/export${query({ ...filter, page: undefined, per_page: undefined })}`,

  pages: (site: number, options: { days?: number; q?: string } = {}) =>
    get<PageRow[]>(`/sites/${site}/pages${query(options)}`),

  labels: (site: number) => get<Label[]>(`/sites/${site}/labels`),
  createLabel: (site: number, input: LabelInput) => post<Label>(`/sites/${site}/labels`, input),
  updateLabel: (site: number, id: number, input: LabelInput) => patch<Label>(`/sites/${site}/labels/${id}`, input),
  deleteLabel: (site: number, id: number) => del(`/sites/${site}/labels/${id}`),

  segments: (site: number) => get<Segment[]>(`/sites/${site}/segments`),
  segment: (site: number, id: number) => get<Segment>(`/sites/${site}/segments/${id}`),
  previewSegment: (site: number, kind: PersonKind, rules: SegmentRules) =>
    post<SegmentPreview>(`/sites/${site}/segments/preview`, { kind, rules }),
  createSegment: (site: number, input: SegmentInput) => post<Segment>(`/sites/${site}/segments`, input),
  updateSegment: (site: number, id: number, input: SegmentInput) =>
    patch<Segment>(`/sites/${site}/segments/${id}`, input),
  deleteSegment: (site: number, id: number) => del(`/sites/${site}/segments/${id}`),
};
