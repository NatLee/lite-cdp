# lite-cdp Cover and Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this independent repository plan task-by-task.

**Goal:** Deliver a distinct 顧客行為航圖 repo cover and a reproducible three-step operation story.

**Architecture:** Reuse the running application and its established domain components. Capture actual isolated demo interactions; only repair behavior if those interactions reveal a concrete defect.

**Tech Stack:** Existing application stack, native HTML/CSS for any new cover, existing Python Playwright capture tooling.

**Spec:** ../specs/2026-11-27-cover-workflows-design.md

## Global Constraints

- Only edit this repo on codex/cover-workflows. Preserve current architecture, permissions and data meanings.
- Real isolated demo captures only; no secret values in scripts, images or docs. No external notifications.
- Desktop captures 1440 px wide with readable viewport or accurately captioned component crops. Fresh page contexts, loaded fonts/images and settled paint.
- Keep source identifiers/filters consistent across the three steps; restore mutated demo records.
- User already authorized implementation and PR lifecycle; parent performs merge and cleanup after review.

## Review Focus

- Screenshot says action succeeded but application data did not change: assert the actual result.
- The three frames silently switch person/article/query: assert identifiers and conditions.
- Long text or mobile width causes clipping: inspect a narrow viewport and the rendered images.
- Exports omit current scope or provenance: inspect the actual downloaded file where the flow uses export.
- Demo credentials, saved personal data or unresolved image paths leak into README: inspect images and local links.

### Task 1: Cover and domain story

**Files:** README.md; docs/images/cover.png and docs/images/src/cover.html for ES/KV only; existing cover assets otherwise.
**Interfaces:** Consumes the established brand/components from the spec; produces a distinct cover and fixed three-step story.

- [x] Read the spec, current README and relevant existing component/capture code.
- [x] 檢查現有獨立封面和現行品牌／功能一致；品質合格即保留，不為這輪任務重做品牌。
- [x] Visually inspect the rendered cover at full size and README-sized width; verify title, theme and legibility.

### Task 2: Reproducible actual operations

**Files:** Create docs/capture-workflow.py and docs/images/workflow-01.png, workflow-02.png, workflow-03.png. Modify relevant product component only for a reproduced flow defect.
**Interfaces:** Consumes a running isolated demo URL and credentials in environment variables; produces three PNGs and assertions for the actual workflow.

- [x] Follow this exact narrative using real controls: 選訪客與一次來訪 → 選事件讀取內容 → 開啟同一人的完整時間軸. Read UI/source first to choose stable accessible selectors.
- [x] Add the small capture script with conditions/assertions tying frames to the same data, settling fonts/images and cleaning temporary changes.
- [x] Run it against a demo; expected: all three actions succeed, three images decode, no page errors and matching data.
- [x] Inspect all three images and verify the workflow at 390 px without document overflow; fix verified defects only. For behavioral fixes add a failing regression first, then run affected suite/build.

### Task 3: README and release evidence

**Files:** README.md, docs/workflow-verification.md; new spec/plan files.
**Interfaces:** Consumes cover and workflow PNGs; produces an accurate guide and reviewable PR.

- [x] Add a prominent three-step walkthrough with action, observed result and demo provenance; keep additional screenshots in an optional gallery.
- [x] Run Python compile on the capture script, git diff --check, image decode and README local-link audit; expected: all pass. No mirror tests for prose or images.
- [x] Record the exact commands/outcomes and any product changes in docs/workflow-verification.md. Mark completed plan checkboxes truthfully.
- [x] Commit/push and open a focused PR; report exact SHA, screenshot paths, CI and demo server PID to parent. Do not merge; parent handles independent review and cleanup.
