import { useCallback, useEffect, useRef, useState } from "react";

/** 載入資料：回傳 data、錯誤、是否載入中，以及重新載入的函式。 */
export function useAsync<T>(fn: () => Promise<T>, deps: unknown[]) {
  const [data, setData] = useState<T | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const seq = useRef(0);
  const load = useCallback(fn, deps);

  const reload = useCallback(async () => {
    const id = ++seq.current;
    setLoading(true);
    try {
      const v = await load();
      if (id === seq.current) {
        setData(v);
        setError(null);
      }
    } catch (e) {
      if (id === seq.current) setError(errorText(e));
    } finally {
      if (id === seq.current) setLoading(false);
    }
  }, [load]);

  useEffect(() => {
    void reload();
  }, [reload]);

  return { data, setData, error, loading, reload };
}

/** 停止輸入 `ms` 毫秒後才更新的值（搜尋框、即時檢查語法用）。 */
export function useDebounced<T>(value: T, ms = 250): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), ms);
    return () => window.clearTimeout(timer);
  }, [value, ms]);
  return debounced;
}

export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
