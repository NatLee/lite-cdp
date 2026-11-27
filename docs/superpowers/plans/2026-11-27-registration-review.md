# Registration and review implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Password and Google/GitHub signup controlled by admin policy/review.
**Architecture:** Existing host accounts, transactional policy/application/events; identical revised Rust access-kit across seven hosts and equivalent Python contract.
**Tech Stack:** Existing Rust/axum or Python/Flask, SQLite, React or vanilla JS.
**Spec:** ../specs/2026-11-27-registration-design.md

## Global Constraints
Defaultclosed,newmembers only,noemailauto-link,active+notdisabled everywhere. Existingfirst-admin untouched. Dedicated codex/registration-review branch in existing clean repo copies; root coordinates commits. API and DTO names are fixed in spec.

## Review Focus
- Migration retains existing users/login methods.
- Closingpolicy duringcallback prevents creation.
- Pending/rejected cannot bypass withdisabledtoggle orAPI/MCP/refresh.
- Concurrentduplicateidentity/review has oneconsistentresult.
- Provider-onlyuser retainsatleastone loginmethod; noticesneverauthorize.

### Task1: Backend
**Files:** Rust crates/access-kit/src/registration.rs and migrations/tests, host auth/access/migrations. Python web/access/registration.py,store.py,schema.sql,accounts.py,providers.py; tests/test_registration.py.
**Interfaces:** Shared HTTP contract in spec, auth/state unchanged.
- [ ] Write and run failing tests: oldmigration,defaultclosed,setuprequired,approval/open,memberonly,pendingnocookie.
- [ ] Implement transactional policy,user,application,event andactivegates.
- [ ] Add failing admin/Origin/reason/stale/cursor/audit/disabledtoggle tests; implementreview.
- [ ] Add mockedproviderpolicyflip/concurrentsubject/state/linkage/lastidentity tests; implementexplicit signup.
- [ ] Run full backend suites andfmt/clippy orRuff/Node/styleaudit; ledgerresults.

### Task2: Frontend
**Files:** React src/web/src/registration components/client/tests,App/Login/SettingsorUsers; ES server/web/access.mjs/css; Python web/static/access.html/mjs/css,registration.mjs,entry_pages.py.
**Interfaces:** Policy/register/review/events endpoints in spec.
- [ ] Write failingtests closedactions,confirmation,pending,activesession,adminpolicy/review/audit.
- [ ] Implementthemed /register route andstatus notices; keepfirstlogin/safenext.
- [ ] Add adminpolicy/queue/history withpagination,errors,busy/reasoncontrols.
- [ ] Run completeUItests/build orJSsyntax/Node/stylechecks.

### Task3: Integration and delivery
**Files:** README.md,docs/registration-verification.md,docs/images/registration-*.png,repeatablebrowserscript.
- [ ] ActualChrome passwordapply/adminreview/login,reject/openflows;1440/390light/dark screenshots.
- [ ] FinalDockerbuild,oldDBmigration,independentsecurity/UIreview.
- [ ] Record preciseevidence andSSO/GPU/CIlimitations; commitscopefiles.
- [ ] Push/createPR,checkexacthead,merge/delete with existingauthorization.
- [ ] Verify cleanmain=reviewedtree=remote; cleanupownedtestresources.
