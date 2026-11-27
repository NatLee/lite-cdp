import {
  Code2,
  FileText,
  Globe,
  LayoutDashboard,
  LogOut,
  Menu,
  Monitor,
  Moon,
  Settings,
  Sun,
  Tags,
  Users,
  UsersRound,
  X,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import {
  Link,
  NavLink,
  Outlet,
  useLocation,
  useMatch,
  useNavigate,
  useOutletContext,
} from "react-router";
import { api } from "../api";
import { useApp, useUser } from "../context";
import { ROLE_LABEL } from "../lib/format";
import { useAsync } from "../lib/hooks";
import { applyTheme, storedTheme, type ThemeChoice } from "../lib/theme";
import type { Site } from "../types";
import { Empty, ErrorBanner, Loading } from "./ui";

/** Layout 交給每一頁的東西：看得到的網站，和重新載入的函式。 */
export interface SitesContext {
  sites: Site[];
  reloadSites: () => Promise<void>;
}

export function useSites(): SitesContext {
  return useOutletContext<SitesContext>();
}

/** `/s/:siteId/...` 底下的頁面用：現在看的是哪個網站。 */
export function useSite(): Site {
  return useOutletContext<SitesContext & { site: Site }>().site;
}

const LAST_SITE_KEY = "cdp.site";

function lastSiteId(): number | null {
  try {
    return Number(localStorage.getItem(LAST_SITE_KEY)) || null;
  } catch {
    return null;
  }
}

interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
}

/** 每個網站底下的頁面；網址是 `/s/<網站編號>/<to>`。 */
const SITE_NAV: NavItem[] = [
  { to: "overview", label: "總覽", icon: LayoutDashboard },
  { to: "people", label: "名單", icon: Users },
  { to: "pages", label: "頁面", icon: FileText },
  { to: "labels", label: "標籤", icon: Tags },
  { to: "segments", label: "分群", icon: UsersRound },
  { to: "install", label: "安裝與設定", icon: Code2 },
];

const GLOBAL_NAV: NavItem[] = [
  { to: "/sites", label: "所有網站", icon: Globe },
  { to: "/settings", label: "帳號設定", icon: Settings },
];

function Brand() {
  return (
    <Link to="/" className="brand flex items-center gap-3">
      <img src="/favicon.svg" alt="" className="size-8" />
      <span className="leading-tight">
        <span className="block text-sm font-semibold tracking-tight">
          Lite CDP
        </span>
        <span className="block text-[11px] text-zinc-500 dark:text-zinc-400">
          JOURNEY OBSERVATORY
        </span>
      </span>
    </Link>
  );
}

function navClass(isActive: boolean): string {
  return `atlas-nav-link flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors ${
    isActive
      ? "bg-brand-50 font-semibold text-brand-700 dark:bg-brand-950/70 dark:text-brand-200"
      : "text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
  }`;
}

function NavItems({
  items,
  prefix = "",
  onNavigate,
}: {
  items: NavItem[];
  prefix?: string;
  onNavigate?: () => void;
}) {
  return (
    <>
      {items.map(({ to, label, icon: Icon }) => (
        <NavLink
          key={to}
          to={prefix + to}
          onClick={onNavigate}
          className={({ isActive }) => navClass(isActive)}
        >
          <Icon className="size-4" />
          {label}
        </NavLink>
      ))}
    </>
  );
}

function SidebarNav({
  sites,
  onNavigate,
}: {
  sites: Site[];
  onNavigate?: () => void;
}) {
  const navigate = useNavigate();
  const match = useMatch("/s/:siteId/:section/*");
  const routeId = Number(match?.params.siteId) || null;
  // 不在某個網站底下時（設定頁之類），選單還是指向上次看的那個網站
  const current =
    sites.find((s) => s.id === (routeId ?? lastSiteId())) ?? sites[0];

  useEffect(() => {
    if (!routeId) return;
    try {
      localStorage.setItem(LAST_SITE_KEY, String(routeId));
    } catch {
      // 記不住就算了
    }
  }, [routeId]);

  return (
    <nav className="atlas-navigation">
      {current && (
        <>
          <label
            className="sr-only"
            htmlFor={onNavigate ? "site-switcher-mobile" : "site-switcher"}
          >
            網站
          </label>
          <select
            id={onNavigate ? "site-switcher-mobile" : "site-switcher"}
            className="input mb-2 font-semibold"
            value={current.id}
            onChange={(e) => {
              // 換網站時留在同一種頁面
              navigate(
                `/s/${e.target.value}/${match?.params.section ?? "overview"}`,
              );
              onNavigate?.();
            }}
          >
            {sites.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
          <NavItems
            items={SITE_NAV}
            prefix={`/s/${current.id}/`}
            onNavigate={onNavigate}
          />
          <div className="nav-divider" />
        </>
      )}
      <NavItems items={GLOBAL_NAV} onNavigate={onNavigate} />
    </nav>
  );
}

const THEMES: { value: ThemeChoice; label: string; icon: LucideIcon }[] = [
  { value: "light", label: "淺色", icon: Sun },
  { value: "dark", label: "深色", icon: Moon },
  { value: "system", label: "跟系統", icon: Monitor },
];

interface ThemeControls {
  theme: ThemeChoice;
  onThemeChange: (theme: ThemeChoice) => void;
}

function ThemeToggle({ theme, onThemeChange }: ThemeControls) {
  return (
    <div
      className="flex rounded-lg bg-zinc-100 p-1 dark:bg-zinc-800"
      role="radiogroup"
      aria-label="外觀"
    >
      {THEMES.map(({ value, label, icon: Icon }) => (
        <button
          key={value}
          role="radio"
          aria-checked={theme === value}
          title={label}
          onClick={() => onThemeChange(value)}
          className={`flex flex-1 items-center justify-center rounded py-1 transition-colors ${
            theme === value
              ? "bg-white text-brand-700  dark:bg-zinc-950 dark:text-brand-300"
              : "text-zinc-500 hover:text-zinc-800 dark:text-zinc-400 dark:hover:text-zinc-100"
          }`}
        >
          <Icon className="size-4" />
          <span className="sr-only">{label}</span>
        </button>
      ))}
    </div>
  );
}

function SidebarFooter({ theme, onThemeChange }: ThemeControls) {
  const { logout, meta } = useApp();
  const user = useUser();
  return (
    <div className="atlas-account">
      <div className="flex items-center gap-1">
        <span className="flex min-w-0 flex-1 items-center gap-2 px-2 py-2">
          <span className="grid size-7 shrink-0 place-items-center rounded-full bg-brand-100 text-xs font-semibold text-brand-700 dark:bg-brand-950 dark:text-brand-300">
            {Array.from(user.display_name)[0]}
          </span>
          <span className="min-w-0">
            <span className="block truncate text-sm font-semibold">
              {user.display_name}
            </span>
            <span className="block text-[11px] text-zinc-500 dark:text-zinc-400">
              {ROLE_LABEL[user.role]}
              {meta ? `・v${meta.version}` : ""}
            </span>
          </span>
        </span>
        <button
          className="btn btn-ghost px-2"
          onClick={() => void logout()}
          title="登出"
        >
          <LogOut className="size-4" />
          <span className="text-xs">登出</span>
        </button>
      </div>
      <ThemeToggle theme={theme} onThemeChange={onThemeChange} />
    </div>
  );
}

export function Layout() {
  const [theme, setTheme] = useState<ThemeChoice>(storedTheme);
  useEffect(() => applyTheme(theme), [theme]);
  const [open, setOpen] = useState(false);
  const location = useLocation();
  const { data, error, reload } = useAsync(() => api.sites(), []);
  const sites = data ?? [];

  useEffect(() => setOpen(false), [location.pathname]);

  return (
    <div className="atlas-app min-h-screen">
      <header className="atlas-header">
        <div className="atlas-masthead">
          <Brand />
          <span className="atlas-motto">
            Every visit is part of a bigger story.
          </span>
          <div className="hidden lg:block">
            <SidebarFooter theme={theme} onThemeChange={setTheme} />
          </div>
          <button
            className="btn btn-ghost px-2 lg:hidden"
            onClick={() => setOpen(true)}
            aria-label="開啟選單"
          >
            <Menu className="size-5" />
          </button>
        </div>
        <div className="hidden lg:block">
          <SidebarNav sites={sites} />
        </div>
      </header>
      {open && (
        <div className="fixed inset-0 z-40 lg:hidden">
          <div
            className="absolute inset-0 bg-zinc-950/50"
            onClick={() => setOpen(false)}
          />
          <div className="atlas-mobile absolute inset-y-0 left-0 flex w-72 flex-col gap-6 bg-white px-3 py-4  dark:bg-zinc-900">
            <div className="flex items-center justify-between px-2">
              <Brand />
              <button
                className="btn btn-ghost px-2"
                onClick={() => setOpen(false)}
                aria-label="關閉選單"
              >
                <X className="size-4" />
              </button>
            </div>
            <div className="flex-1 overflow-y-auto">
              <SidebarNav sites={sites} onNavigate={() => setOpen(false)} />
            </div>
            <SidebarFooter theme={theme} onThemeChange={setTheme} />
          </div>
        </div>
      )}

      <main className="min-w-0 flex-1">
        {error && (
          <PageBody>
            <ErrorBanner message={error} />
          </PageBody>
        )}
        {!data && !error && (
          <PageBody>
            <Loading />
          </PageBody>
        )}
        {data && (
          <Outlet
            context={{ sites, reloadSites: reload } satisfies SitesContext}
          />
        )}
      </main>
    </div>
  );
}

/** `/s/:siteId` 底下的頁面先經過這裡：網站不存在（或你看不到）就不往下畫。 */
export function SiteScope() {
  const context = useSites();
  const match = useMatch("/s/:siteId/*");
  const site = context.sites.find((s) => s.id === Number(match?.params.siteId));
  if (!site) {
    return (
      <PageBody>
        <div className="card">
          <Empty icon={Globe} title="找不到這個網站">
            可能已經被刪除，或是你不在這個網站的成員裡。
            <div className="mt-4">
              <Link to="/sites" className="btn btn-secondary">
                看所有網站
              </Link>
            </div>
          </Empty>
        </div>
      </PageBody>
    );
  }
  // key：換網站時整頁重新來過，不會留著上一個網站的篩選和輸入
  return <Outlet key={site.id} context={{ ...context, site }} />;
}

export function PageBody({ children }: { children: ReactNode }) {
  return (
    <div className="page-body mx-auto max-w-7xl px-4 py-6 sm:px-7 lg:px-10 lg:py-8">
      {children}
    </div>
  );
}
