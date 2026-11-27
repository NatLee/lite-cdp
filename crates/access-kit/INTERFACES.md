# Access Kit 0.1 adapter and wire interfaces

Self-contained stable interface for the shared OAuth/MCP/API contract, version0.1.0. Validate SNAPSHOT.json before integration.

 (pin before host coding)

```rust
pub type HostFuture<'a,T> = Pin<Box<dyn Future<Output=Result<T,AccessError>> + Send + 'a>>;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="lowercase")]
pub enum Role { Admin, Member }
#[derive(Clone, Serialize)]
pub struct Principal { pub user_id:i64, pub username:String, pub display_name:String, pub role:Role }
// Not Serialize/Deserialize/Debug: opaque hash of existing session token, never raw cookie.
pub struct CookieSession { pub principal:Principal, pub session_binding:[u8;32] }
pub struct HostPaths { pub login:&'static str, pub connections:&'static str, pub consent:&'static str }
#[derive(Clone)]
pub struct DomainOperation { pub name:&'static str, pub description:&'static str,
    pub input_schema:Value, pub required_scope:&'static str, pub read_only:bool }
pub trait Host: Send+Sync+'static {
    fn cookie_principal<'a>(&'a self, headers:&'a HeaderMap) -> HostFuture<'a,Option<CookieSession>>;
    fn active_principal(&self, user_id:i64) -> HostFuture<'_,Option<Principal>>;
    fn create_session(&self, user_id:i64) -> HostFuture<'_,HeaderValue>;
    fn operations(&self) -> Vec<DomainOperation>;
    fn execute(&self, invocation:Invocation) -> HostFuture<'_,Value>;
    fn paths(&self) -> HostPaths;
}
impl Invocation {
    pub fn principal(&self)->&Principal;
    pub fn operation(&self)->&str;
    pub fn arguments(&self)->&Value;
    pub fn trusted_principal(&self)->TrustedPrincipal;
}
// Clone+Send+Sync, private constructor/fields, no Deserialize/Serialize.
impl TrustedPrincipal { pub fn user_id(&self)->i64; }
impl AccessConfig { pub fn from_env(default_origin:&str)->Result<Self,AccessError>; pub fn secure_cookies(&self)->bool; }
impl AccessError {
    pub fn new(status:StatusCode, code:&'static str, message:impl Into<String>)->Self;
    pub fn status(&self)->StatusCode;
    pub fn code(&self)->&'static str;
}
// IntoResponse: status + {"error":controlled_message,"code":stable_code}.
impl AccessKit {
    pub async fn new(pool:SqlitePool, config:AccessConfig, host:Arc<dyn Host>)->Result<Self,AccessError>;
    pub fn router(&self)->Router<()>;
}
```

Host next-numbered migration contains byte-identical `migrations/0001_access.sql` and `migrations/0002_client_registration.sql` and `migrations/0003_registration.sql` in order. Apply host migrations before AccessKit::new; constructor verifies schema and registry and never migrates host users itself. Host stores kit in OnceLock on Inner and adapter owns Weak<Inner>, avoiding Arc cycles. Core router borrows a cloneable Arc-backed handle and returns state-erased Router. Merge alongside the existing router after its legacy Origin guard; core applies its own cookie-origin checks so OAuth token clients are not blocked by cookie-only middleware.

Host execute maps only fixed operations to internal handler URLs, inserts `invocation.trusted_principal()` extension, converts validated arguments, calls an uncounted in-process Router oneshot, bounds returned body, and preserves controlled400/403/404/429/503. Existing CurrentUser extractor checks that extension and queries active current full host user (including hotel fields), otherwise uses original cookie. Reject headers pretending to be this extension. Domain permission check is still required; the extension is authentication, not authorization.

Scope names are exact registry values and advertised as their union. Sinyi/Hotel/KEYPO reports:read; CDP get_person_events people:read. Core allows no unknown scopes and exposes only registered read_only operations in this release. JSON schemas are object schemas, <=32KiB each, no remote/file refs; compile per-kit immutable validators keyed by this registry, never global name-only cache.

## Wire shapes and limits shared with UI implementers

All timestamps RFC3339UTC; record IDs opaque strings except existing user_id integer. List responses wrap `{items:[...]}`. No credential hash/session binding/provider token fields serialized.

- providers: `{items:[{provider,name,enabled,login_start_url,disabled_reason}]}`. Provider names github/google always present.
- identities: `{items:[{id,provider,subject_mask,linked_at,last_login_at}]}`; no email-based identity matching.
- create credential body `{name,scopes,expires_in_days?:30,audiences?:["api","mcp"]}`; response `{credential:{id,name,prefix,scopes,audiences,created_at,expires_at,last_used_at,revoked_at},token}` once. List `{items:[credential]}`; DELETE204.
- grants: `{items:[{id,client_id,client_name,scopes,resource,created_at,expires_at,last_used_at,revoked_at}]}`; DELETE204(owner/admin).
- clients: admin GET/POST `{items:[{client_id,client_name,redirect_uris,created_at}]}` / registration response per RFC7591; DCR never fetches remote metadata.
- overview: `{identities:[...],credentials:[...],grants:[...],summary:{days:7,from,to,timezone:"UTC",attempts,successes,failures,pending}}`.
- usage: `{days,from,to,timezone:"UTC",transport:"api"|"mcp"|null,totals:{attempts,successes,failures,pending},daily:[{date,attempts,successes,failures,pending}]}`; fill zero dates server-side; no permanent all-time claim.
- events: `{items:[{id,request_id,at,kind,transport,operation,user_id,client_id,credential_id,status,outcome,duration_ms}],next_cursor}`. Limit default25/max100, opaque bounded cursor; kindoperation/security; outcomespending/success/failure/interrupted.
- consent GET `/api/access/consents/{request_id}`: `{request_id,client_name,scopes,resource,expires_at,user:{id,username,display_name}}`; POST `/oauth/consent/{request_id}` accepts JSON or application/x-www-form-urlencoded with only `{decision:"approve"|"deny"}` and returns303 to the already validated redirect. Host consent SPA route comes from HostPaths with `?request_id=` and uses this API; native form POST permits the final303 external-client navigation without fetch CORS.
- POST `/api/v1/{operation}` consumes exactly its registered JSON argument object and returns raw domain JSON. Header X-Request-Id server-generated; do not trust supplied value.
- client POST body: `{client_name,redirect_uris,token_endpoint_auth_method:"none",grant_types:["authorization_code","refresh_token"],response_types:["code"]}`; latter3 may be omitted and default only these exact supported values. Provider login may carry an encoded next only for local `/oauth/authorize` plus query; store it in provider flow and reject any other next.
- protocol errors `/oauth/token|revoke|register` use RFC fields`error,error_description`, never raw provider or SQL errors; token responses no-store. All access management responses no-store.

Limits: all JSON/form bodies1MiB (provider response256KiB), schemas32KiB, args depths32, operation max64chars, names80chars, input scope max32 entries, total registered ops128, per-user active credentials32, outstanding browser flows8, global pending flows2048, pending public clients256 (managed/previously authorized clients exempt), redirect URIs/client8(length2048). DCR10/minute/IP, provider start10/minute/browser, bearer120/minute/credential+user, management reads240/minute/user and writes60/minute/user; independent authorized/public endpoint maps, max4096 buckets per namespace with expiry. IP derived from socket ConnectInfo only, not arbitrary forwarded headers. Domain execution timeout30s; stale pending recovery600s, limit100 rows/pass; recovery at startup and bounded maintenance interval60s. 90days detail purge bounded1000/pass. No secret/payload logging by rate limit keys.



Provider continuation additionally accepts exactly HostPaths.connections. The `test-support`-only ACCESS_TEST_MOCK_PROVIDER_URL accepts loopback and calls AccessConfig::with_mock_providers; production code never reads it. See README for fixture launch and logging boundaries.

Public client registration is disabled by default. Set `ACCESS_DYNAMIC_CLIENT_REGISTRATION=true` to advertise and enable `/oauth/register`; administrator registration remains available. Public registrations without a user grant expire after24hours, unless a live consent/code still references them. Their pending quota never consumes administrator registration capacity. Schema2 is additive: host migrations must preserve original0002 bytes and add the new core migration as0003. Existing clients remain managed.

Rate identity uses the trusted socket peer for unauthenticated token attempts. Invalid client IDs/codes cannot consume a verified user's token budget. MCP finalization occurs after collecting the SDK response, including transport/body-limit failures; SQLite audit writes reserve the writer with `BEGIN IMMEDIATE` before reading so concurrent WAL requests do not strand successful calls.

## Schema 3 account registration

`RegistrationUser` carries validated username/display_name, Argon2 password_hash, account_status and password_login_enabled. `RegistrationAccount` returns status and password_login_enabled. Hosts implement five transaction-aware hooks on the supplied `&mut SqliteConnection`: `registration_count`, `registration_create`, `registration_account`, `registration_set_status` and `registration_admin`. The latter must revalidate the exact cookie session, expiry, active status, disabled flag and administrator role inside the write transaction. Status review only changes pending members. Default hooks fail closed.

The core owns `/api/auth/registration-policy` GET/PATCH, `/api/auth/register` POST, `/api/auth/registrations` GET, `/api/auth/registrations/{id}/review` POST and `/api/auth/registration-events` GET. Policy is closed, approval or open; pre-bootstrap responses remain effectively closed. Password signup rejects authenticated callers and unknown body properties. New usernames are normalized ASCII lowercase (3–64 characters), passwords12–256 characters, display names1–100 trimmed characters. Administrators review only pending applications, rejection requires a trimmed reason at most500 characters, stale reviews return409. Lists use descending integer IDs with limit1–100/default50 and optional before cursor. Registration writes require an explicit matching Origin header. All mutations and their events commit together under BEGIN IMMEDIATE; creation rechecks bootstrap, policy and the500-account cap.

`/oauth/login/{provider}?intent=register` stores purpose signup; plain login still requires an existing stable provider+subject binding. Signup has no email matching, generates a unique host username, creates a member with password_login_enabled=false, and commits account/identity/application/event together. Pending accounts receive no session. Callback rechecks current policy. Existing identities conflict on signup; verified pending/rejected provider login redirects with account_pending/account_rejected. Final identity unlink is denied with last_login_method when no password is enabled.

Hosts append account_status (active default, pending or rejected) and password_login_enabled (true default) columns without changing old migrations or IDs. Every host password login, cookie lookup, trusted-principal lookup, active principal and session insertion must enforce active status and disabled=false. Administrative password reset enables password login. Existing auth/state is unchanged; account administration DTOs expose account_status.
