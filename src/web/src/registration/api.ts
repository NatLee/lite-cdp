import { ApiError } from "../api";
export type Mode = "closed" | "approval" | "open";
export type Status = "pending" | "active" | "rejected";
export interface Policy { mode: Mode; password_enabled: boolean; providers: string[]; setup_required: boolean }
export interface Registration { id: number; user_id: number; username: string; display_name: string; provider: "password" | "google" | "github"; status: Status; created_at: string; reviewed_at: string | null; reviewed_by: number | null; rejection_reason: string | null }
export interface RegistrationEvent { id: number; actor_id: number | null; actor_username: string | null; action: "registered" | "approved" | "rejected" | "policy_changed"; registration_id: number | null; mode: string | null; reason: string | null; created_at: string }
export interface Page<T> { items: T[]; next_cursor: number | null }
export async function registrationRequest<T>(path: string, method = "GET", body?: unknown): Promise<T> {
  const response = await fetch(`/api/auth/${path}`, { method, credentials: "same-origin", cache: "no-store", headers: body === undefined ? {} : { "content-type": "application/json" }, body: body === undefined ? undefined : JSON.stringify(body) });
  const data = await response.json();
  if (!response.ok) throw new ApiError(response.status, typeof data.error === "string" ? data.error : `伺服器回應 ${response.status}`, data);
  return data as T;
}
export function safeRegistrationNext(search: string): string | null {
  const next = new URLSearchParams(search).get("next");
  if (!next || !next.startsWith("/oauth/authorize?") || next.includes("\\") || next.includes("#") || /[\u0000-\u001f]/.test(next)) return null;
  const url = new URL(next, "https://local.invalid");
  return url.origin === "https://local.invalid" && url.pathname === "/oauth/authorize" && url.search ? url.pathname + url.search : null;
}
export function continuation(search: string): string { const next = safeRegistrationNext(search); return next ? `?next=${encodeURIComponent(next)}` : ""; }
