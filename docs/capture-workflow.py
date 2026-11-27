#!/usr/bin/env python3
"""Capture a real visitor/session/event/profile flow; no application data is changed.

DEMO_URL=http://127.0.0.1:15375 DEMO_PASSWORD=... python docs/capture-workflow.py
Requires Playwright, Chrome and a running isolated instance seeded by `lite-cdp demo`.
"""
import asyncio
import os
from pathlib import Path
from playwright.async_api import async_playwright, expect

URL = os.environ.get("DEMO_URL", "http://127.0.0.1:15375").rstrip("/")
OUT = Path(__file__).resolve().parent / "images"

def groups(events):
    result = {}
    for event in events:
        key = f"session:{event['session_id']}" if event["session_id"] else f"event:{event['id']}"
        result.setdefault(key, []).append(event)
    return sorted(((key, sorted(items, key=lambda e: (e["created_at"], e["id"]))) for key, items in result.items()), key=lambda pair: pair[1][-1]["created_at"], reverse=True)

async def settle(page):
    await page.evaluate("document.fonts.ready")
    await page.evaluate("Promise.all([...document.images].map(i => i.decode().catch(() => {})))")
    await page.evaluate("new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))")
    await page.mouse.move(0, 0)
    assert not await page.evaluate("document.documentElement.scrollWidth > innerWidth"), page.url

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(channel=os.environ.get("PLAYWRIGHT_CHANNEL", "chrome"))
        context = await browser.new_context(viewport={"width": 1440, "height": 1050}, color_scheme="light", timezone_id="Asia/Taipei")
        page = await context.new_page()
        errors = []
        page.on("pageerror", lambda e: errors.append(str(e)))
        async def get(path):
            response = await context.request.get(URL + path)
            assert response.ok, (path, response.status)
            return await response.json()
        await page.goto(URL, wait_until="networkidle")
        await page.get_by_label("帳號", exact=True).fill(os.environ.get("DEMO_USERNAME", "demo"))
        await page.get_by_label("密碼", exact=True).fill(os.environ["DEMO_PASSWORD"])
        await page.get_by_role("button", name="登入", exact=True).click()
        await expect(page.get_by_role("button", name="登出", exact=True)).to_be_visible()
        sites = await get("/api/sites")
        sid = sites[0]["id"]
        base = f"/s/{sid}"
        people = await get(f"/api/sites/{sid}/people?sort=last_seen&per_page=6")
        # Choose a real named member with a compact recorded session for legible images.
        choice = None
        for index, person in enumerate(people["items"]):
            events = await get(f"/api/sites/{sid}/people/{person['id']}/events?limit=50")
            sessions = groups(events["items"])
            compact = next(((key, rows) for key, rows in sessions if 3 <= len(rows) <= 6 and any(e["type"] == "custom" for e in rows)), None)
            if person["name"] and compact:
                choice = index, person, events, sessions, compact
                break
        assert choice, "Built-in demo needs a named recent visitor with a 3–6-event session containing a custom event"
        index, person, events, sessions, compact = choice
        pid = person["id"]
        session, items = compact
        assert items[0]["session_id"], "Do not connect events without session IDs"
        await page.goto(URL + base + "/overview", wait_until="networkidle")
        visitor = page.locator(".visitor-selector > button").nth(index)
        await visitor.click()
        await expect(visitor).to_have_attribute("aria-pressed", "true")
        await expect(page.get_by_role("link", name="開啟個人旅程")).to_have_attribute("href", base + f"/people/{pid}")
        await page.get_by_label("選擇來訪").select_option(session)
        await expect(page.locator(".journey-point")).to_have_count(len(items))
        await expect(page.get_by_label("選擇來訪").locator("option")).to_have_count(len(sessions))
        await settle(page)
        await page.locator(".journey-observatory").screenshot(path=str(OUT / "workflow-01.png"), animations="disabled")
        focus_index = next((i for i, e in reversed(list(enumerate(items))) if e["type"] == "custom"), len(items) - 1)
        target = items[focus_index]
        point = page.locator(".journey-point").nth(focus_index)
        await point.focus()
        await page.keyboard.press("Enter")
        await expect(point).to_have_attribute("aria-pressed", "true")
        await expect(page.locator(".focus-description code")).to_have_text(target["path"] or "未附頁面路徑")
        await expect(page.locator(".focus-description strong")).to_have_text(target["title"] or target["path"] if target["type"] == "pageview" else target["name"])
        await expect(page.get_by_label("選擇來訪")).to_have_value(session)
        await settle(page)
        await page.locator(".journey-observatory").screenshot(path=str(OUT / "workflow-02.png"), animations="disabled")
        await page.get_by_role("link", name="開啟個人旅程").click()
        await expect(page).to_have_url(URL + base + f"/people/{pid}")
        await expect(page.get_by_role("heading", name=person["name"], exact=False)).to_be_visible()
        await expect(page.get_by_label("事件種類")).to_have_value("")
        # The profile exposes all this person's events, including earlier pages.
        all_events = list(events["items"])
        before = events["next_before"]
        while before is not None:
            batch = await get(f"/api/sites/{sid}/people/{pid}/events?limit=50&before={before}")
            all_events.extend(batch["items"])
            before = batch["next_before"]
        await expect(page.locator(".person-event-ledger li").first).to_be_visible()
        while await page.get_by_role("button", name="更早的", exact=True).count():
            button = page.get_by_role("button", name="更早的", exact=True)
            await expect(button).to_be_enabled()
            async with page.expect_response(lambda response: "/events?" in response.url and response.request.method == "GET"):
                await button.click()
            await page.wait_for_timeout(100)
        await expect(page.locator(".person-event-ledger li")).to_have_count(len(all_events))
        await page.get_by_label("選擇來訪").select_option(session)
        await expect(page.locator(".journey-point")).to_have_count(len(items))
        await page.locator(".journey-point").nth(focus_index).click()
        await expect(page.locator(".focus-description code")).to_have_text(target["path"] or "未附頁面路徑")
        assert await get(f"/api/sites/{sid}/people/{pid}/events?limit=50") == events
        await page.evaluate("scrollTo(0,0)")
        await settle(page)
        await page.screenshot(path=str(OUT / "workflow-03.png"), animations="disabled")
        await page.set_viewport_size({"width": 390, "height": 844})
        await settle(page)
        await page.get_by_label("選擇來訪").select_option(session)
        await page.locator(".journey-point").nth(focus_index).focus()
        await page.keyboard.press("Enter")
        await expect(page.locator(".focus-description code")).to_have_text(target["path"] or "未附頁面路徑")
        await page.goto(URL + base + "/overview", wait_until="networkidle")
        await page.locator(".visitor-selector > button").nth(index).click()
        await page.get_by_label("選擇來訪").select_option(session)
        await settle(page)
        assert not errors, errors
        await browser.close()
        print(f"PASS: visitor #{pid}, {session}, event #{target['id']} stay consistent across 3 frames; {len(all_events)} profile events verified; keyboard/mobile no overflow or page errors; read-only.")

asyncio.run(main())
