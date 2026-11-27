# 顧客足跡航圖 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this repository plan task-by-task. Root coordinates independent repo workers and one independent review; no per-task agents.

**Goal:** Rebuild the 顧客足跡航圖 frontend with strict visual tokens, domain-specific section hierarchy, meaningful motion and actual feature captures.
**Architecture:** Preserve existing component boundaries and domain data contracts; revise source styles/utilities rather than appending a blanket override. Add focused motion helpers only if state-linked animation requires them.
**Tech Stack:** Existing repository frontend stack and local Python Playwright capture tools; no new frontend framework.
**Spec:** ../specs/2026-11-27-themed-motion-design.md

## Global Constraints

- One primary accent family; semantic data colors have textual labels. No decorative glow, glass, purple-blue gradient, persistent heavy shadow.
- Font weights400/600 only; padding/margin/gap fixed multiples4px, with0/auto for normal layout. Font sizes, borders, chart coordinates are separate.
- Preserve readable fonts, tables, keyboard, responsive states, light/dark. Functional Lucide icons16/20/24px withstroke1.5.
- Preserve all existing auth/data/filters. OAuth/MCP/API implementation is a separate phase under root’s access contract; do not create fake integrations or metrics.
- Isolated demo captures only, credentials fromenv. Restore mutations and never send external notifications.
- Root handles PR integration/merge; worker commits but does not merge.

## Review Focus

- Mobile390px, long labels and error states must not overflow after spacing normalization.
- Reduced-motion must immediately show final visible content and preserve keyboard focus.
- Same selected source/record/session/time/filter must survive visual transitions.
- Loading/empty/error/default states cannot show fake activity or success.
- CSS source cleanup must not leave effective forbidden utilities in secondary pages or broken font faces.

### Task1: Strict styles and distinct sections

**Files:** src/web/src/index.css, components/JourneyExplorer.tsx, components/EngagementAtlas.tsx, components/Layout.tsx, components/ui.tsx; all other frontend source files where existing prohibited utilities occur. Existing component paths listed withoutprefix refer to src/web/src/.
**Interfaces:** Existing data props and API shapes unchanged; sectionDOM and semantic labels remain accessible. Style tokens ownspacing, weights, radii, iconrules.
- [x] Read spec and actualsource; inspect currentworkflowPNG; record affected styles and preserve existing source edits.
- [x] Normalize source CSS and Tailwind utilities across pages to spec. Add small localtokenfile if needed; avoid global!important coverups.
- [x] Strengthen actualdomainsections: 訪客索引／session足跡／事件詳情／完整時間軸；選事件定位游標.
- [x] Build/typecheck using repo commands; run source/computed-style scan for non4px spacing, >2weights, blur/glow/heavyshadows and investigate matches.

### Task2: Functional motion and interaction

**Files:** Above domaincomponents; create a narrowly named motion helper/style file only if necessary. Update existing componenttests only for newfunctionalbehavior.
**Interfaces:** Selected data drives marker/transition; no fake counters/timestamps, no extra API request loops. Reducedmotion has immediate finalstate.
- [x] Implement at least one meaningful state-linked domainanimation (120–240ms) and one clear sectiontransition. Prefer CSS+React key/state, no animationdependency for simple transitions.
- [x] If adding a new action/state, first reproduce missingbehavior with relevant failingtest; implementthenrun. For purestyle do not add mirrorunit tests.
- [x] Run relevant existingtests and build: `pnpm --dir src/web test; pnpm --dir src/web build`. Capture count/outcome; failures must be diagnosed.
- [x] Verify keyboard selection, reducedmotion, nooverflow at1440/390, no pageerrors and stable dataidentity.

### Task3: Real feature demonstration and covers

**Files:** docs/capture-workflow.py, existing docs capture scripts; docs/images/workflow-01..03.png; docs/images/feature-motion.webm or feature-motion.gif; existingcover source/assets; README.md; docs/themed-motion-verification.md.
**Interfaces:** Running isolateddemo→actualthree-step screenshots and shortmotionrecording; preserve consistent IDs/scopes. No staticHTML fake success.
- [x] Restart an isolated localdemo only if needed; record ownedPID/port privately. Reuse safeexistingdemo fixtures; never printcredentials.
- [x] Refresh3workflowPNGs and affected oldREADME screenshots; record a short actual selection/switch workflow withPlaywrightvideo. Linkvideo ifGitHubcannotembed; do notclaimstillimages showanimation.
- [x] Updatecoverfromeditable source to match currentfontweights/palette; maintain distinctivecomposition and readability. Inspect allPNGandmotionframes.
- [x] UpdateREADME with precise sections/actions, screenshotprovenance, motionlink and reducedmotiondescription. Add manual-reviewitems (brandhue, demo vs realdata, alttext, deploymentOAuthconfigpending).
- [x] Run capture/assertions, image decode, Python syntax, README link audit, git diff --check. Markchecklisttruthfullyand document commands/results plus knownlimits.
- [x] Commit reviewedscope and reportSHA/files/testresults/PIDs to root; do notmerge. OAuth work remains separate under the shared access contract.
