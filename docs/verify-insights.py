#!/usr/bin/env python3
"""Exercise real insight drilldowns against an isolated seeded demo.

DEMO_URL=http://127.0.0.1:5173 DEMO_PASSWORD=... python docs/verify-insights.py
Requires Playwright and Chrome. No application data is changed.
"""
import asyncio, csv, io, os
from playwright.async_api import async_playwright, expect

URL=os.environ.get("DEMO_URL","http://127.0.0.1:5173").rstrip("/")
async def main():
    async with async_playwright() as p:
        browser=await p.chromium.launch(channel=os.environ.get("PLAYWRIGHT_CHANNEL","chrome"))
        context=await browser.new_context(viewport={"width":1440,"height":1050},timezone_id="Asia/Taipei",color_scheme="light")
        page=await context.new_page()
        errors=[]
        page.on("pageerror",lambda error:errors.append(str(error)))
        await page.goto(URL,wait_until="networkidle")
        await page.get_by_label("帳號",exact=True).fill(os.environ.get("DEMO_USERNAME","demo"))
        await page.get_by_label("密碼",exact=True).fill(os.environ["DEMO_PASSWORD"])
        await page.get_by_role("button",name="登入",exact=True).click()
        await expect(page.get_by_role("button",name="登出",exact=True)).to_be_visible()
        async def json_get(path):
            response=await context.request.get(URL+path)
            assert response.ok, (path,response.status)
            return await response.json()
        async def csv_count():
            href=await page.get_by_role("link",name="匯出 CSV",exact=True).get_attribute("href")
            response=await context.request.get(URL+href)
            assert response.ok
            return len(list(csv.reader(io.StringIO((await response.body()).decode("utf-8-sig")))))-1
        async def go(path):
            await page.goto(URL+path,wait_until="networkidle")
            assert not await page.evaluate("document.documentElement.scrollWidth > innerWidth"),path
        sites=await json_get("/api/sites")
        sid=sites[0]["id"]
        base=f"/s/{sid}"
        overview=await json_get(f"/api/sites/{sid}/overview?tz_offset=480")
        titles={"recent":"近期活躍","cooling":"回訪降溫","dormant":"久未回訪"}
        for cohort in overview["engagement"]:
            for kind,label in [("member","會員"),("anonymous","匿名訪客")]:
                await go(base+"/overview")
                await page.get_by_role("link",name=f"{titles[cohort['activity']]}的{label}，{cohort[kind]} 人，查看名單").click()
                await expect(page.get_by_label("最近活躍")).to_have_value(cohort["activity"])
                await expect(page.get_by_role("tab",name=label,exact=True)).to_have_attribute("aria-selected","true")
                result=await json_get(f"/api/sites/{sid}/people?kind={kind}&activity={cohort['activity']}")
                assert result["total"]==cohort[kind]
                assert await csv_count()==cohort[kind]
        for width,theme in [(1440,"light"),(390,"light"),(390,"dark"),(1440,"dark")]:
            await page.set_viewport_size({"width":width,"height":900})
            await page.evaluate("theme=>localStorage.setItem('cdp.theme',theme)",theme)
            await go(base+"/overview")
            assert await page.locator("html").evaluate("el=>el.classList.contains('dark')")==(theme=="dark")
            # The route is built from recorded events, not inferred transitions.
            people=await json_get(f"/api/sites/{sid}/people?sort=last_seen&per_page=6")
            for index,person in enumerate(people["items"][:2]):
                await page.locator(".visitor-selector > button").nth(index).click()
                await expect(page.get_by_role("link",name="開啟個人旅程")).to_have_attribute("href",f"/s/{sid}/people/{person['id']}")
                events=await json_get(f"/api/sites/{sid}/people/{person['id']}/events?limit=50")
                groups={}
                for event in events["items"]:
                    groups.setdefault(f"session:{event['session_id']}" if event["session_id"] else f"event:{event['id']}",[]).append(event)
                await expect(page.get_by_label("選擇來訪").locator("option")).to_have_count(len(groups))
                for session,items in list(groups.items())[:2]:
                    await page.get_by_label("選擇來訪").select_option(session)
                    await expect(page.locator(".journey-point")).to_have_count(len(items))
                    ordered=sorted(items,key=lambda e:(e["created_at"],e["id"]))
                    target=page.locator(".journey-point").last
                    await target.focus()
                    await page.keyboard.press("Enter")
                    await expect(target).to_have_attribute("aria-pressed","true")
                    await expect(page.locator(".focus-description code")).to_have_text(ordered[-1]["path"] or "未附頁面路徑")
                assert not await page.evaluate("document.documentElement.scrollWidth > innerWidth")
            await page.get_by_role("button",name="每日訪客",exact=True).click()
            slider=page.get_by_role("slider",name="圖表日期")
            await slider.focus()
            await page.keyboard.press("Home")
            day=overview["daily"][0]
            await expect(slider).to_have_attribute("aria-valuetext",day["date"])
            await expect(page.locator(".traffic-readout")).to_contain_text(f"{day['visitors']}位訪客")
            await page.keyboard.press("ArrowRight")
            await expect(slider).to_have_attribute("aria-valuetext",overview["daily"][1]["date"])
            # SVG styling must not enlarge the navigation icons.
            assert await page.locator(".cohort-open svg").first.evaluate("el=>el.getBoundingClientRect().width") <= 16
            await page.get_by_role("link",name=f"近期活躍的會員，{overview['engagement'][0]['member']} 人，查看名單").click()
            await expect(page.get_by_label("最近活躍")).to_have_value("recent")
            assert not await page.evaluate("document.documentElement.scrollWidth > innerWidth")
        if len(sites)>1:
            await go(f"/s/{sites[1]['id']}/overview")
            other=await json_get(f"/api/sites/{sites[1]['id']}/overview")
            await expect(page.locator(".audience-total strong")).to_have_text(f"{other['totals']['people']:,}")
        assert not errors, errors
        await browser.close()
        print("Real Chrome desktop/mobile/light/dark insight drilldowns, CSV counts and keyboard controls passed.")

asyncio.run(main())
