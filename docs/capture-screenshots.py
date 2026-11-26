#!/usr/bin/env python3
"""Capture the running demo UI; requires `pip install playwright` and Chromium.

DEMO_URL=http://127.0.0.1:5173 DEMO_PASSWORD=... python docs/capture-screenshots.py
Use an isolated database seeded with the demo command. This script only reads data.
"""
import asyncio
import os
from pathlib import Path
from playwright.async_api import async_playwright, expect

URL = os.environ.get("DEMO_URL", "http://127.0.0.1:5173").rstrip("/")
PASSWORD = os.environ["DEMO_PASSWORD"]
OUT = Path(__file__).resolve().parent / "images"

async def main():
    async with async_playwright() as p:
        options = {}
        if os.environ.get("PLAYWRIGHT_CHROMIUM_EXECUTABLE"):
            options["executable_path"] = os.environ["PLAYWRIGHT_CHROMIUM_EXECUTABLE"]
        browser = await p.chromium.launch(**options)
        context = await browser.new_context(viewport={"width": 1440, "height": 1050}, color_scheme="light", device_scale_factor=1)
        page = await context.new_page()
        errors = []
        page.on("pageerror", lambda error: errors.append(str(error)))
        await page.goto(URL, wait_until="networkidle")
        await page.screenshot(path=str(OUT / "screenshot-login-light.png"))
        await page.set_viewport_size({"width": 390, "height": 844})
        assert not await page.evaluate("document.documentElement.scrollWidth > innerWidth"), "mobile login"
        await page.set_viewport_size({"width": 1440, "height": 1050})
        await page.get_by_label("帳號", exact=True).fill(os.environ.get("DEMO_USERNAME", "demo"))
        await page.get_by_label("密碼", exact=True).fill(PASSWORD)
        await page.get_by_role("button", name="登入", exact=True).click()
        await expect(page.get_by_role("button", name="登出", exact=True)).to_be_visible()
        async def shot(path, filename, full=False):
            await page.goto(URL + path, wait_until="networkidle")
            await page.evaluate("document.fonts.ready")
            await page.mouse.move(0, 0)
            await page.screenshot(path=str(OUT / filename), full_page=full, animations="disabled")
            assert not await page.evaluate("document.documentElement.scrollWidth > innerWidth"), path
        sites = await (await context.request.get(URL + "/api/sites")).json()
        site = sites[0]["id"]
        base = f"/s/{site}"
        people = await (await context.request.get(URL + f"/api/sites/{site}/people?kind=member")).json()
        segments = await (await context.request.get(URL + f"/api/sites/{site}/segments")).json()
        person = people["items"][0]["id"]
        segment = segments[0]["id"]
        routes = [(base + "/overview", "overview"), (base + "/people", "people"), (base + f"/people/{person}", "person"), (base + "/labels", "labels"), (base + f"/segments/{segment}", "segment"), (base + "/install", "install")]
        for route, name in routes:
            await shot(route, f"screenshot-{name}-light.png")
        await page.get_by_role("radio", name="深色", exact=True).click()
        await shot(base + "/overview", "screenshot-overview-dark.png")
        await page.get_by_role("radio", name="淺色", exact=True).click()
        await page.set_viewport_size({"width": 390, "height": 844})
        await shot(base + "/overview", "screenshot-mobile.png")
        await page.get_by_role("button", name="開啟選單").click()
        await expect(page.get_by_role("button", name="關閉選單")).to_be_visible()
        await page.get_by_role("button", name="關閉選單").click()
        for route, _ in routes:
            await page.goto(URL + route, wait_until="networkidle")
            assert not await page.evaluate("document.documentElement.scrollWidth > innerWidth"), route

        assert not errors, errors
        print("Desktop, dark and mobile captures passed; no page errors or horizontal overflow.")
        await browser.close()

asyncio.run(main())
