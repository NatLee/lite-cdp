import { useEffect } from "react";
export function ContinueAuthorization({ to }: { to: string }) {
  useEffect(() => { window.location.replace(to); }, [to]);
  return <p role="status" className="p-8">正在返回授權流程…</p>;
}
