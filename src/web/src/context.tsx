import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { api, ApiError, UNAUTHORIZED_EVENT } from "./api";
import type { AuthState, Meta, User } from "./types";

interface AppState {
  meta: Meta | null;
  /** 登入狀態；載入前是 null。 */
  auth: AuthState | null;
  /** 讀不到登入狀態時的錯誤（後端沒開之類的） */
  authError: string | null;
  login: (username: string, password: string) => Promise<void>;
  setup: (input: { username: string; display_name: string; password: string }) => Promise<void>;
  logout: () => Promise<void>;
  /** 改了個人資料之後更新畫面上的名字 */
  setUser: (user: User) => void;
}

const Ctx = createContext<AppState | null>(null);

export function AppProvider({ children }: { children: ReactNode }) {
  const [meta, setMeta] = useState<Meta | null>(null);
  const [auth, setAuth] = useState<AuthState | null>(null);
  const [authError, setAuthError] = useState<string | null>(null);

  useEffect(() => {
    api.meta().then(setMeta).catch(() => setMeta(null));
    api
      .authState()
      .then(setAuth)
      .catch((e) => setAuthError(e instanceof Error ? e.message : String(e)));
    const onUnauthorized = () => setAuth((a) => (a ? { ...a, user: null } : a));
    window.addEventListener(UNAUTHORIZED_EVENT, onUnauthorized);
    return () => window.removeEventListener(UNAUTHORIZED_EVENT, onUnauthorized);
  }, []);

  const login = useCallback(async (username: string, password: string) => {
    const user = await api.login(username, password);
    setAuth({ setup_required: false, user });
  }, []);

  const setup = useCallback(async (input: { username: string; display_name: string; password: string }) => {
    try {
      const user = await api.setup(input);
      setAuth({ setup_required: false, user });
    } catch (error) {
      if (error instanceof ApiError && error.status === 409) {
        // A conflict proves setup is closed, even if the refresh is offline.
        setAuth({ setup_required: false, user: null });
        try { setAuth(await api.authState()); } catch { /* Keep the confirmed closed state. */ }
      }
      throw error;
    }
  }, []);

  const logout = useCallback(async () => {
    try {
      await api.logout();
    } finally {
      setAuth((a) => (a ? { ...a, user: null } : a));
    }
  }, []);

  const setUser = useCallback((user: User) => setAuth((a) => (a ? { ...a, user } : a)), []);

  const value = useMemo<AppState>(
    () => ({ meta, auth, authError, login, setup, logout, setUser }),
    [meta, auth, authError, login, setup, logout, setUser],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useApp(): AppState {
  const v = useContext(Ctx);
  if (!v) throw new Error("useApp must be used inside <AppProvider>");
  return v;
}

/** 登入之後的頁面用：一定有使用者。 */
export function useUser(): User {
  const { auth } = useApp();
  if (!auth?.user) throw new Error("useUser must be used after sign-in");
  return auth.user;
}
