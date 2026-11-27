#!/usr/bin/env python3
"""Real Chrome integration on disposable databases. No customer data or external services.
Build web/API first; then python docs/verify-signal-console.py [--binary PATH].
Requires Playwright and installed Google Chrome. Writes actual screenshots + JSON evidence.
"""
import argparse, asyncio, html, json, os, socket, subprocess, tempfile, threading, time, urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse
from playwright.async_api import async_playwright, expect
from frontend_style_audit import assert_computed_styles
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'docs/images'
UA='Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36'

def free_port():
    with socket.socket() as sock: sock.bind(('127.0.0.1',0)); return sock.getsockname()[1]

async def main(binary):
    with tempfile.TemporaryDirectory(prefix='lite-cdp-signal-') as tmp:
        port=free_port();url=f'http://127.0.0.1:{port}'
        env={k:v for k,v in os.environ.items() if not k.startswith(('ACCESS_','CDP_'))}
        env.update(CDP_BIND=f'127.0.0.1:{port}',CDP_DATABASE_PATH=f'{tmp}/demo.db',CDP_STATIC_DIR=str(ROOT/'src/web/dist'),CDP_PUBLIC_URL=url,ACCESS_PUBLIC_URL=url,CDP_COOKIE_SECURE='false',CDP_RATE_LIMIT='0',CDP_DEMO_PASSWORD='SignalConsole-Disposable-2026')
        subprocess.run([binary,'demo'],cwd=ROOT,env=env,check=True,capture_output=True)
        server=subprocess.Popen([binary],cwd=ROOT,env=env,stdout=open(f'{tmp}/server.log','w'),stderr=subprocess.STDOUT)
        fixture_state={'key':''}
        class Fixture(BaseHTTPRequestHandler):
            def log_message(self,*_): pass
            def do_GET(self):
                token=parse_qs(urlparse(self.path).query).get('test',[''])[0]
                body=f'''<!doctype html><html><head><title>Signal integration fixture</title><script async src="{url}/t.js" data-key="{fixture_state['key']}" data-test-token="{html.escape(token)}"></script></head><body><h1>Disposable shop</h1><button id="grant" onclick="cdp.consent(true)">Grant consent</button><button id="login" onclick="cdp.identify({{user_id:'member-42',email:'shared@example.com'}});cdp.flush()">Login</button><button id="logout" onclick="cdp.reset()">Logout</button><button id="revoke" onclick="cdp.consent(false)">Revoke</button></body></html>'''.encode()
                self.send_response(200);self.send_header('Content-Type','text/html; charset=utf-8');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
        fixture=ThreadingHTTPServer(('127.0.0.1',0),Fixture);threading.Thread(target=fixture.serve_forever,daemon=True).start()
        fixture_url=f'http://127.0.0.1:{fixture.server_port}'
        try:
            for _ in range(100):
                try:
                    with urllib.request.urlopen(url+'/api/health',timeout=.5) as r:
                        if r.status==200: break
                except Exception: await asyncio.sleep(.1)
            async with async_playwright() as p:
                browser=await p.chromium.launch(channel='chrome')
                context=await browser.new_context(viewport={'width':1440,'height':1050},color_scheme='light',timezone_id='Asia/Taipei',user_agent=UA)
                page=await context.new_page();errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
                await page.goto(url,wait_until='networkidle');await page.get_by_label('帳號',exact=True).fill('demo');await page.get_by_label('密碼',exact=True).fill(env['CDP_DEMO_PASSWORD']);await page.get_by_role('button',name='登入',exact=True).click();await expect(page.get_by_role('button',name='登出',exact=True)).to_be_visible()
                async def api(path,method='get',data=None):
                    response=await getattr(context.request,method)(url+'/api'+path,**({'data':data} if data is not None else {}));assert response.ok,(path,response.status,await response.text());return await response.json()
                sites=await api('/sites');demo=sites[0];sid=demo['id'];base=f'/s/{sid}'
                policy={'consent_required':True,'respect_dnt':True,'auto_click':False,'auto_scroll':False,'query_mode':'allowlist','query_allowlist':['campaign'],'referrer_scope':'origin','click_text':False,'allowed_traits':['plan']}
                site=await api('/sites','post',{'name':'Signal verification','allowed_origins':[fixture_url],'collection_policy':policy});test_sid=site['id'];fixture_state['key']=site['key']
                async def counts():
                    result=await api(f'/sites/{test_sid}/overview');return result['totals']['people'],result['last_7_days']['pageviews']
                c1=await browser.new_context(user_agent=UA);a=await c1.new_page();collected=[]
                a.on('request',lambda req:collected.append(json.loads(req.post_data)) if req.url==url+'/collect' else None)
                await a.goto(fixture_url+'/?email=private&campaign=spring#secret');await a.wait_for_function("window.cdp?.status().status === 'consent_required'")
                assert await a.evaluate('localStorage.length')==0
                await a.wait_for_timeout(900);assert not collected
                await a.locator('#grant').click();await a.wait_for_timeout(950)
                assert collected[0]['events'][0]['url']==fixture_url+'/?campaign=spring'
                await a.locator('#login').click();await a.wait_for_timeout(150)
                people=await api(f'/sites/{test_sid}/people?kind=member');assert people['total']==1 and people['items'][0]['user_id']=='member-42'
                c2=await browser.new_context(user_agent=UA);b=await c2.new_page();await b.goto(fixture_url);await b.wait_for_function('window.cdp?.status().policy');await b.locator('#grant').click();await b.locator('#login').click();await b.wait_for_timeout(200)
                people=await api(f'/sites/{test_sid}/people?kind=member');assert people['total']==1
                detail=await api(f"/sites/{test_sid}/people/{people['items'][0]['id']}");assert len(detail['devices'])==2
                before=await counts();await a.evaluate("() => { const s=document.createElement('script');s.src=window.cdp.endpoint.replace('/collect','/t.js');s.dataset.key=window.cdp.key;document.head.appendChild(s); }");await a.wait_for_function('cdp.status().duplicates === 1');await a.wait_for_timeout(900);assert await counts()==before
                await a.evaluate("() => {const s=document.createElement('script');s.src=window.cdp.endpoint.replace('/collect','/t.js');s.dataset.key='wrong-key';document.head.appendChild(s);}");await a.wait_for_function("cdp.status().last_error === 'conflicting_installation'")
                await b.evaluate("cdp.identify({user_id:'member-other',email:'shared@example.com'});cdp.flush()");await b.wait_for_timeout(150);assert (await api(f'/sites/{test_sid}/people?kind=member'))['total']==2
                sibling=await c1.new_page();await sibling.goto(fixture_url);await sibling.wait_for_function('window.cdp?.status().policy');await sibling.locator('#grant').click();await sibling.wait_for_timeout(900)
                prior=await sibling.evaluate("localStorage.getItem(cdp.status().namespace+'_anon_id')");await a.locator('#logout').click();await sibling.evaluate("cdp.track('after_logout');cdp.flush()");await sibling.wait_for_timeout(150);assert await sibling.evaluate("localStorage.getItem(cdp.status().namespace+'_anon_id')")!=prior
                await a.locator('#revoke').click();await sibling.evaluate("cdp.track('must_not_send');cdp.flush()");assert await sibling.evaluate('cdp.status().consent') is False
                # Simulate a response lost after the collector accepted the first request.
                attempts=0
                async def lose_response(route):
                    nonlocal attempts
                    attempts+=1;response=await route.fetch()
                    if attempts==1: await route.fulfill(status=503,body='retry fixture')
                    else: await route.fulfill(response=response)
                await b.route(url+'/collect',lose_response)
                pre=await counts();await b.evaluate("cdp.page();cdp.flush()");await b.wait_for_timeout(1700);assert attempts==2 and (await counts())[1]==pre[1]+1
                await b.unroute(url+'/collect')
                token=await api(f'/sites/{test_sid}/installation-tests','post',{});pre=await counts()
                c3=await browser.new_context(user_agent=UA);t=await c3.new_page();await t.goto(fixture_url+'/?test='+token['token']);await t.wait_for_function('window.cdp?.status().policy');await t.locator('#grant').click();await t.wait_for_timeout(950)
                receipt=await api(f"/sites/{test_sid}/installation-tests/{token['token']}");assert receipt['receipt']['analytics_written'] is False and receipt['receipt']['events'][0]['type']=='pageview';assert await counts()==pre
                # Real UI: framework, policy save, simulator and receipt creation.
                await page.goto(url+base+'/install',wait_until='networkidle');await page.get_by_role('button',name='使用最少採集設定').click();await page.get_by_role('button',name='儲存採集設定').click();await expect(page.get_by_text('使用已儲存設定',exact=True)).to_be_visible()
                await page.get_by_role('tab',name='Next.js',exact=True).click();assert 'afterInteractive' in await page.get_by_label('追蹤程式碼',exact=True).inner_text()
                await page.get_by_role('button',name='登入同一會員').click();await page.get_by_role('button',name='另一台裝置').click();await expect(page.get_by_label('模擬人物數')).to_have_text('2');await page.get_by_role('button',name='登入同一會員').click();await expect(page.get_by_label('模擬人物數')).to_have_text('1')
                await page.get_by_role('button',name='建立 15 分鐘測試碼').click();await expect(page.get_by_label('安裝測試碼',exact=True)).to_be_visible()
                async def settle():
                    await page.evaluate('document.fonts.ready');await page.evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))');await assert_computed_styles(page);assert not await page.evaluate('document.documentElement.scrollWidth>innerWidth'),page.url
                await page.wait_for_timeout(3600)
                await settle();await page.locator('.identity-lab').screenshot(path=str(OUT/'signal-identity-light.png'),animations='disabled');await page.locator('.integration-panel').screenshot(path=str(OUT/'signal-install-light.png'),animations='disabled');await page.evaluate('window.scrollTo(0,0)');await settle();await page.screenshot(path=str(OUT/'screenshot-install-light.png'),animations='disabled')
                await page.get_by_role('radio',name='深色',exact=True).click();await settle();await page.locator('.identity-lab').screenshot(path=str(OUT/'signal-identity-dark.png'),animations='disabled')
                await page.set_viewport_size({'width':390,'height':844});await settle();await page.locator('.identity-lab').screenshot(path=str(OUT/'signal-identity-mobile.png'),animations='disabled');await page.evaluate('window.scrollTo(0,0)');await settle();await page.screenshot(path=str(OUT/'signal-install-mobile-dark.png'),animations='disabled')
                await page.set_viewport_size({'width':1440,'height':1050});await page.get_by_role('radio',name='淺色',exact=True).click()
                members=await api(f'/sites/{sid}/people?kind=member');segments=await api(f'/sites/{sid}/segments')
                routes=[('/overview','overview'),('/people','people'),(f"/people/{members['items'][0]['id']}",'person'),('/labels','labels'),(f"/segments/{segments[0]['id']}",'segment')]
                for route,name in routes:
                    await page.goto(url+base+route,wait_until='networkidle');await settle();await page.screenshot(path=str(OUT/f'screenshot-{name}-light.png'),full_page=name=='overview',animations='disabled')
                await page.goto(url+base+'/overview',wait_until='networkidle');await page.get_by_role('radio',name='深色',exact=True).click();await settle();await page.screenshot(path=str(OUT/'screenshot-overview-dark.png'),full_page=True,animations='disabled');await page.locator('.journey-observatory').screenshot(path=str(OUT/'screenshot-journey-dark.png'),animations='disabled')
                await page.get_by_role('radio',name='淺色',exact=True).click();await settle();await page.locator('.journey-observatory').screenshot(path=str(OUT/'screenshot-journey-light.png'),animations='disabled');await page.set_viewport_size({'width':390,'height':844});await settle();await page.screenshot(path=str(OUT/'screenshot-mobile.png'),full_page=True,animations='disabled')
                cover=await context.new_page();await cover.set_viewport_size({'width':1440,'height':720});await cover.set_content((ROOT/'docs/images/cover.svg').read_text());await cover.add_style_tag(content='body{margin:0}svg{display:block;width:1440px;height:720px}');await cover.screenshot(path=str(OUT/'cover-preview.png'))
                assert not errors,errors
                report={'browser':'Google Chrome via Playwright','disposable_database':True,'backend':'real Rust/SQLite','checks':['consent-no-storage-no-collection','URL sanitization','two-device user_id merge','shared-email different users separated','duplicate load idempotent','conflicting key diagnostic','cross-tab logout','cross-tab revoke','response-loss retry deduplicated','test receipt isolated','UI policy persistence','Next template','simulator identity merge','desktop light/dark and mobile styles/no overflow'],'page_errors':errors,'retry_attempts':attempts}
                (ROOT/'docs/signal-console-browser.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
                print(json.dumps(report,ensure_ascii=False,indent=2));await browser.close()
        finally:
            fixture.shutdown();server.terminate()
            try:server.wait(timeout=10)
            except subprocess.TimeoutExpired:server.kill()
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--binary',default=str(ROOT/'src/api/target/debug/lite-cdp'));args=parser.parse_args();asyncio.run(main(str(Path(args.binary).resolve())))
