#!/usr/bin/env python3
"""Exercise the native access UI against an isolated local demo, never production.

Password comes from ACCESS_SCREENSHOT_PASSWORD. --mock-providers requires a
feature-gated loopback mock provider; it does not prove production connectivity.
The script never prints or captures a raw credential/code/token.
"""
import argparse,asyncio,base64,hashlib,json,os,secrets,subprocess,tempfile,shutil,threading
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlencode,urlparse,parse_qs
from playwright.async_api import async_playwright
from frontend_style_audit import assert_computed_styles
ROOT=Path(__file__).resolve().parents[1]
INBOX=ROOT.name=='feedback-inbox'
PREFIX='fi' if INBOX else 'cdp'
OPERATION='get_feedback_stats' if INBOX else 'get_site_overview'
ARGS={} if INBOX else {'site_id':1}
SCOPE='feedback:read' if INBOX else 'reports:read'
STYLE_AUDIT=r'''() => {
const issues=[];const props=['paddingTop','paddingRight','paddingBottom','paddingLeft','marginTop','marginRight','marginBottom','marginLeft','rowGap','columnGap'];
for(const e of document.querySelectorAll('.access-workspace *,.access-workspace,.access-provider-login *')){
if(e.namespaceURI!=='http://www.w3.org/1999/xhtml'||!e.getClientRects().length||e.classList.contains('sr-only'))continue;
const s=getComputedStyle(e),label=e.tagName+'.'+e.className;
for(const p of props){if(p.startsWith('margin')&&e.matches('.access-workspace,.access-consent'))continue;const v=s[p],n=parseFloat(v);if(v!=='normal'&&v!=='auto'&&Number.isFinite(n)&&Math.abs(n/4-Math.round(n/4))>.001)issues.push({label,p,v});}
if(!['400','600'].includes(s.fontWeight))issues.push({label,p:'fontWeight',v:s.fontWeight});if(s.backdropFilter!=='none')issues.push({label,p:'backdrop',v:s.backdropFilter});}
return issues;
}'''
async def settle(page):
 await page.wait_for_load_state('networkidle');await page.evaluate('document.fonts.ready')
 await assert_computed_styles(page)
 assert not await page.evaluate('document.documentElement.scrollWidth>innerWidth'),'document overflow'
async def login(page,args):
 await page.goto(args.url);await page.get_by_label('帳號',exact=True).fill(args.username);await page.get_by_label('密碼',exact=True).fill(os.environ['ACCESS_SCREENSHOT_PASSWORD']);await page.get_by_role('button',name='登入',exact=True).click();await settle(page)
async def sdk(token,url):
 env={**os.environ,'ACCESS_TEST_BASE':url,'ACCESS_TEST_TOKEN':token,'ACCESS_TEST_OPERATION':OPERATION,'ACCESS_TEST_ARGUMENTS':json.dumps(ARGS)}
 result=await asyncio.to_thread(subprocess.run,[os.environ['ACCESS_CAPTURE_PROBE']],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=20)
 assert result.returncode==0,'Official SDK probe failed'
async def quota_pause():
 if os.environ.get("ACCESS_CAPTURE_LEGACY_PACING")=="1":
  await asyncio.sleep(32);await asyncio.sleep(32)
async def restrict_mock_network(context):
 async def guard(route):
  if urlparse(route.request.url).hostname in {'127.0.0.1','localhost','::1'}:await route.continue_()
  else:await route.abort()
 await context.route('**/*',guard)
async def main(args):
 assert urlparse(args.url).scheme=='http' and urlparse(args.url).hostname in {'127.0.0.1','localhost'},'isolated loopback demo required'
 class Callback(BaseHTTPRequestHandler):
  def do_GET(self):
   self.send_response(200);self.send_header('content-type','text/html');self.end_headers();self.wfile.write(b'<title>Local demo callback</title><p>Local test client callback received.</p>')
  def log_message(self,*_args):pass
 callback_server=ThreadingHTTPServer(('127.0.0.1',0),Callback);threading.Thread(target=callback_server.serve_forever,daemon=True).start()
 video_dir=tempfile.TemporaryDirectory(prefix='inbox-cdp-access-video-')
 errors=[];requests=[];checks=[];mock_checks=[]
 async with async_playwright() as pw:
  launch={'executable_path':os.environ['PLAYWRIGHT_CHROMIUM_EXECUTABLE']} if os.environ.get('PLAYWRIGHT_CHROMIUM_EXECUTABLE') else {'channel':args.channel}
  browser=await pw.chromium.launch(**launch)
  context=await browser.new_context(viewport={'width':1440,'height':1080},reduced_motion='reduce',color_scheme='light')
  if args.mock_providers:await restrict_mock_network(context)
  page=await context.new_page();page.on('pageerror',lambda e:errors.append(str(e)));page.on('request',lambda r:requests.append(r.url) if '/events/notify' in r.url or '/alert/send' in r.url else None)
  await page.goto(args.url);await settle(page)
  if args.mock_providers:
   async with page.expect_navigation(wait_until='networkidle'):
    await page.get_by_role('button',name='使用 GitHub 登入',exact=True).click()
   assert parse_qs(urlparse(page.url).query).get('access_error')==['identity_not_linked'],'Unbound mock identity must be rejected before local account creation'
   await page.get_by_text('這個外部帳號尚未綁定。請先以原帳號登入，再到連線與存取綁定。',exact=True).wait_for()
   mock_checks.append('unbound identity cannot create an account')
  await page.screenshot(path=str(ROOT/'docs/images/access-01-login.png'))
  await login(page,args);state=await (await context.request.get(args.url+'/api/auth/state')).json();user_id=state['user']['id']
  await page.get_by_role('link',name='連線與存取',exact=True).first.click();await page.get_by_role('heading',name='連線與存取',exact=True).wait_for();await settle(page)
  if args.mock_providers:
   for provider in ['GitHub','Google']:
    async with page.expect_navigation(wait_until='networkidle'):
     await page.get_by_role('button',name='綁定 '+provider,exact=True).click()
    await page.get_by_role('button',name='解除 '+provider+' 綁定',exact=True).wait_for();await settle(page)
   identities=await (await context.request.get(args.url+'/api/access/identities')).json();assert {i['provider'] for i in identities['items']}=={'github','google'}
   await context.request.post(args.url+'/api/auth/logout',data={})
   await page.goto(args.url)
   async with page.expect_navigation(wait_until='networkidle'):
    await page.get_by_role('button',name='使用 GitHub 登入',exact=True).click()
   await page.get_by_role('heading',name='連線與存取',exact=True).wait_for();await settle(page)
   external_state=await (await context.request.get(args.url+'/api/auth/state')).json();assert external_state['user']['id']==user_id
   assert any(c['name']==PREFIX+'_session' for c in await context.cookies())
   mock_checks.extend(['GitHub and signed Google mock OIDC bound to the existing account','provider login issues the original host cookie and same user id'])
  baseline=await (await context.request.get(args.url+'/api/access/usage?days=7&transport=api')).json()
  name='本機示範研究終端'
  await page.get_by_label('金鑰名稱',exact=True).fill(name)
  async with page.expect_response(lambda r:r.request.method=='POST' and r.url.endswith('/api/access/credentials')) as created:
   await page.get_by_role('button',name='建立 API 金鑰',exact=True).click()
  key=await (await created.value).json();token=key['token'];credential_id=key['credential']['id']
  await page.get_by_role('dialog',name='一次性 API 金鑰',exact=True).wait_for()
  assert await page.get_by_label('金鑰原值',exact=True).input_value()==token
  await page.get_by_role('button',name='關閉並清除金鑰原值',exact=True).click()
  assert await page.get_by_label('金鑰名稱',exact=True).evaluate('(e)=>e===document.activeElement')
  assert token not in await page.inner_text('body')
  await settle(page);await page.screenshot(path=str(ROOT/'docs/images/access-02-credentials.png'),full_page=True)
  storage=await context.storage_state();await context.close()
  # A real quota-window pause keeps this scripted multi-flow run within production limits.
  await quota_pause()
  context=await browser.new_context(storage_state=storage,viewport={'width':1440,'height':1080},color_scheme='light',record_video_dir=video_dir.name,record_video_size={'width':1440,'height':1080})
  if args.mock_providers:await restrict_mock_network(context)
  page=await context.new_page();page.on('pageerror',lambda e:errors.append(str(e)));await page.goto(args.url+'/connections');await settle(page)
  await page.locator('#access-credentials').scroll_into_view_if_needed();await page.wait_for_timeout(400)
  await sdk(token,args.url)
  response=await context.request.post(args.url+'/api/v1/'+OPERATION,headers={'Authorization':'Bearer '+token},data=ARGS);assert response.ok
  api_calls=1
  if args.mock_providers:
   client_name='本機示範研究工具';callback=f'http://127.0.0.1:{callback_server.server_port}/callback'
   await page.get_by_label('應用名稱',exact=True).fill(client_name);await page.get_by_label('回呼網址（每行一個）',exact=True).fill(callback)
   async with page.expect_response(lambda r:r.request.method=='POST' and r.url.endswith('/api/access/clients')) as registered:
    await page.get_by_role('button',name='註冊 OAuth 應用',exact=True).click()
   client=await (await registered.value).json();client_id=client['client_id']
   verifier=secrets.token_urlsafe(32);challenge=base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b'=').decode();resource=args.url+'/api/v1'
   code=None
   for decision in ['deny','approve']:
    state_value=secrets.token_urlsafe(24)
    params={'response_type':'code','client_id':client_id,'redirect_uri':callback,'scope':SCOPE,'state':state_value,'resource':resource,'code_challenge':challenge,'code_challenge_method':'S256'}
    await page.goto(args.url+'/oauth/authorize?'+urlencode(params));await page.get_by_role('heading',name='授權 '+client_name,exact=True).wait_for();await settle(page)
    assert await page.locator('form input,form select,form textarea').count()==0
    if decision=='approve':await page.screenshot(path=str(ROOT/'docs/images/access-consent.png'))
    async with page.expect_navigation(wait_until='networkidle'):
     await page.get_by_role('button',name='拒絕' if decision=='deny' else '同意授權',exact=True).click()
    query=parse_qs(urlparse(page.url).query);assert query['state']==[state_value]
    if decision=='deny':assert query['error']==['access_denied']
    else:code=query['code'][0]
   exchange=await context.request.post(args.url+'/oauth/token',form={'grant_type':'authorization_code','client_id':client_id,'redirect_uri':callback,'code':code,'code_verifier':verifier,'resource':resource});assert exchange.ok
   assert 'no-store' in exchange.headers.get('cache-control','')
   oauth=await exchange.json();oauth_access=oauth['access_token']
   assert (await context.request.post(args.url+'/api/v1/'+OPERATION,headers={'Authorization':'Bearer '+oauth_access},data=ARGS)).ok;api_calls+=1
   await page.goto(args.url+'/connections');await page.get_by_role('button',name='撤銷授權 '+client_name,exact=True).wait_for()
   grant_row=page.get_by_role('button',name='撤銷授權 '+client_name,exact=True).locator('..')
   await page.get_by_role('button',name='撤銷授權 '+client_name,exact=True).click();await grant_row.get_by_text('已撤銷',exact=True).wait_for();await settle(page)
   assert (await context.request.post(args.url+'/api/v1/'+OPERATION,headers={'Authorization':'Bearer '+oauth_access},data=ARGS)).status==401
   mock_checks.extend(['native immutable consent deny and approve preserve state','PKCE exchange and resource-bound OAuth token call a real domain operation','revoked application token is rejected immediately'])
  # Finish the short, secret-free native authorization recording before the next quota window.
  await page.wait_for_timeout(400);video_path=await page.video.path()
  storage=await context.storage_state();await context.close();shutil.copyfile(video_path,ROOT/'docs/images/access-workflow.webm');video_dir.cleanup()
  await quota_pause()
  context=await browser.new_context(storage_state=storage,viewport={'width':1440,'height':1080},color_scheme='light',reduced_motion='reduce')
  if args.mock_providers:await restrict_mock_network(context)
  page=await context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
  await page.goto(args.url+'/connections');await page.get_by_label('通道').select_option('api');await settle(page)
  after=await (await context.request.get(args.url+'/api/access/usage?days=7&transport=api')).json()
  assert after['totals']['attempts']-baseline['totals']['attempts']==api_calls
  assert after['totals']['successes']-baseline['totals']['successes']==api_calls
  key_row=page.get_by_role('button',name='撤銷金鑰 '+name,exact=True).locator('..')
  await page.get_by_role('button',name='撤銷金鑰 '+name,exact=True).click();await key_row.get_by_text('已撤銷',exact=True).wait_for();await settle(page)
  assert (await context.request.post(args.url+'/api/v1/'+OPERATION,headers={'Authorization':'Bearer '+token},data=ARGS)).status==401
  final=await (await context.request.get(args.url+'/api/access/usage?days=7&transport=api')).json();assert final['totals']['attempts']==after['totals']['attempts']
  await page.goto(args.url+'/connections');await page.get_by_label('通道').select_option('api');await settle(page)
  await page.get_by_text('嘗試 '+str(final['totals']['attempts']),exact=True).wait_for()
  await page.locator('.access-audit').screenshot(path=str(ROOT/'docs/images/access-03-usage.png'))
  mcp=await(await context.request.get(args.url+'/api/access/usage?transport=mcp')).json();assert mcp['totals']['attempts']==1 and mcp['totals']['successes']==1
  await page.wait_for_timeout(500)

  if args.mock_providers:
   await page.get_by_role('button',name='解除 GitHub 綁定',exact=True).click();await settle(page)
   await page.get_by_role('button',name='綁定 GitHub',exact=True).wait_for()
   await context.request.post(args.url+'/api/auth/logout',data={});await login(page,args)
   assert (await (await context.request.get(args.url+'/api/auth/state')).json())['user']['id']==user_id
   mock_checks.append('unlink preserves local password login')
  storage=await context.storage_state();await context.close()
  await quota_pause()
  for theme in ['light','dark']:
   for width in [1440,390]:
    ctx=await browser.new_context(storage_state=storage,viewport={'width':width,'height':1080 if width==1440 else 844},color_scheme=theme,reduced_motion='reduce');await ctx.add_init_script(f"localStorage.setItem('{PREFIX}.theme','{theme}')")
    p=await ctx.new_page();p.on('pageerror',lambda e:errors.append(str(e)));await p.goto(args.url+'/connections');await p.get_by_label('金鑰名稱',exact=True).wait_for();await settle(p)
    issues=await p.evaluate(STYLE_AUDIT);assert not issues,issues
    if width==390:await p.screenshot(path=str(ROOT/f'docs/images/access-mobile-{theme}.png'),full_page=True)
    assert await p.locator('.access-heading').evaluate('(e)=>getComputedStyle(e).animationName')=='none'
    checks.append({'theme':theme,'width':width,'overflow':False,'style_issues':issues})
    await ctx.close()
  assert not requests,'No notification actions are permitted';assert not errors,errors
  try:await asyncio.wait_for(browser.close(),15)
  except asyncio.TimeoutError:await asyncio.wait_for(pw.stop(),10)
  callback_server.shutdown();callback_server.server_close()
 snapshot=json.loads((ROOT/'crates/access-kit/SNAPSHOT.json').read_text())
 result={'core_source_sha256':snapshot['source_sha256'],'core_schema_sha256':snapshot['schema_sha256'],'demo_data':True,'mock_providers':args.mock_providers,'operation':OPERATION,'api_success_delta':api_calls,'mcp_success_delta':1,'credential_id':credential_id,'revocation_denied':True,'mock_checks':mock_checks,'views':checks,'page_errors':errors,'notifications':len(requests)}
 (ROOT/'docs/access-qa.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
 print('PASS: actual UI/key/API/audit/revocation; native mock provider/consent checks='+str(len(mock_checks))+'; four themed viewport checks; no secrets captured')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--url',required=True);p.add_argument('--username',default='demo');p.add_argument('--channel',default='chrome');p.add_argument('--mock-providers',action='store_true');asyncio.run(main(p.parse_args()))
