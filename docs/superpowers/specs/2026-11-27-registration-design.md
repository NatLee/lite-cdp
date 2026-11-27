# Registration and administrator review

User requests subsequent accounts via password or Google/GitHub, with administrator review and open/closed registration. Existing first-login remains the only first-admin creation path. User previously authorized direct implementation and PR merge/deletion. Default closed is an implementation decision announced to the user.

## Approach
Extend existing local accounts/provider flows rather than adding an identity service deployment. Keep each project's theme and account-admin page. All new applicants are members. Existing accounts migrate active. No email-based merging; third-party identity uses stable provider+subject. Google/GitHub remain the agreed providers.

## HTTP contract
- Keep existing auth/state unchanged.
- Public GET /api/auth/registration-policy -> {mode:closed|approval|open,password_enabled:boolean,providers:[configured provider keys],setup_required:boolean}. Before first-admin: effectiveclosed,false,[],true. PATCH same endpoint requires active admin cookie + same Origin, accepts exactly {mode}, returns policy.
- POST /api/auth/register {username,password,display_name?} ->201 {status:pending|active,user_id}. Active sets existing cookie; pending no session. Username3–64 ASCII letters/digits/_.-, password12–256 chars, display1–100 trimmed chars. Reject role injection. Five attempts/min per real peer, no X-Forwarded-For trust; maximum500 accounts including pending/rejected. Beforebootstrap orclosed reject.
- GET /api/auth/registrations?status=pending|active|rejected&limit=50&before=id adminonly ->{items:[{id,user_id,username,display_name,provider:password|google|github,status,created_at:ISO UTC,reviewed_at:ISO|null,reviewed_by:number|null,rejection_reason:string|null}],next_cursor:number|null}. Defaultpending,limit1..100,descendingID cursor.
- POST /api/auth/registrations/{id}/review admin+Origin {decision:approve|reject,reason?:trimmed string<=500} ->record. Onlypending transition, stale409; rejectrequires nonemptyreason. Never mutate role or disabled.
- GET /api/auth/registration-events?limit=50&before=id adminonly ->{items:[{id,actor_id:number|null,actor_username:string|null,action:registered|approved|rejected|policy_changed,registration_id:number|null,mode:string|null,reason:string|null,created_at:ISO UTC}],next_cursor:number|null}. Events append transactionally with creation/review/policy; no secrets or provider subjects.
- OAuth signup GET /oauth/login/{provider}?intent=register&next=<existing safe local authorize continuation>. State stores explicit signup purpose. Plain login stays linked-login. Callback rechecks policy transactionally; pendingredirect /login?registration=pending; activecookie+safecontinuation. Existingidentity conflict never creates a duplicate. Allowlisted browser status errors: account_pending,account_rejected,registration_closed,setup_required. Pendingstatus notice alone grants nothing.

## Data/authorization
Add users.account_status active(default)|pending|rejected and password_login_enabled boolean(defaulttrue). Existing users/manualadmincreation stayactive. Provider-only accounts have random unguessable password hash,flagfalse. Prevent unlinking final identity if no usable password (last_login_method). No emailauto-link.
Shared Rust access-kit schema3 adds policy/applications/events identically across7; each host appends additive user-column migration. Python AccessStore migrates equivalently. Same transaction includes currentpolicy,usercap,user,identity,application,event. All password/cookie/trusted-domain/API/MCP/OAuthcode/refresh paths require active+notdisabled. Existing admin disabled toggle cannotapproveapplicants. Closingregistration leaves existingusers working.

## UI
/register themed passwordconfirmation+configuredproviderbuttons, disabled/closed statusnoactions. Pending outcome outside authenticated layout. Adminsections: policy, paginated/filterable reviewqueue, audit history. Busy/error handling,rejectreason,stale409refresh,keyboardlabels. Existingfirst-login andsafeOAuthcontinuation unchanged. Oneaccent,weights400/600,4pxspacing,no gradients/glass/emoji/heavyshadows. Light/dark/mobile/reducedmotion. No passwords in URLs/storage/screenshots.

## Acceptance
Test oldDBmigration; closed/approval/open; cookieadmin+Origin; pending/rejected isolation anddisabledtoggle; approve/reject/stale; signup/reviewrace; callbackpolicyflip; noemailmerge; finalidentity protection; pagination/auditredaction. Fullbackend/frontend suites, Rustfmt/clippy, PythonRuff/Node/styleaudit, finalDockerbuild. ActualChrome closed→approval→apply→approve→login, rejection andopenregistration ondisposableDB. Screenshots1440/390light/dark. Mockprovidertransporttests; no claimliveSSOwithoutcredentials. UpdateREADME+verificationdocs; independentreview thenPR/merge/delete,distinguish privateCIbillingblock fromlocaltests.

## References
https://developers.google.com/identity/openid-connect/openid-connect
https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps
