export type Transport = "api" | "mcp";
export interface Provider { provider: "github" | "google"; name: string; enabled: boolean; login_start_url: string; disabled_reason: string | null }
export interface Identity { id: string; provider: string; subject_mask: string; linked_at: string; last_login_at: string | null }
export interface Credential { id: string; name: string; prefix: string; scopes: string[]; audiences: Transport[]; created_at: string; expires_at: string; last_used_at: string | null; revoked_at: string | null }
export interface Grant { id: string; client_id: string; client_name: string; scopes: string[]; resource: string; created_at: string; expires_at: string; last_used_at: string | null; revoked_at: string | null }
export interface Totals { attempts: number; successes: number; failures: number; pending: number }
export interface Summary extends Totals { days: number; from: string; to: string; timezone: "UTC" }
export interface Overview { identities: Identity[]; credentials: Credential[]; grants: Grant[]; summary: Summary }
export interface Usage { days: number; from: string; to: string; timezone: "UTC"; transport: Transport | null; totals: Totals; daily: ({ date: string } & Totals)[] }
export interface AuditEvent { id: string; request_id: string; at: string; kind: string; transport: Transport | null; operation: string; user_id: number | null; client_id: string | null; credential_id: string | null; status: number | null; outcome: string; duration_ms: number | null }
export interface Client { client_id: string; client_name: string; redirect_uris: string[]; created_at: string }
export interface Consent { request_id: string; client_name: string; scopes: string[]; resource: string; expires_at: string; user: { id: number; username: string; display_name: string } }
