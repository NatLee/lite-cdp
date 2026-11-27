# -*- coding: utf-8 -*-
"""Verify registration/review against a new disposable local database.

Requires a built frontend/API, Python Playwright, and installed Google Chrome.
The script starts and stops its own server, deletes its temporary database,
checks real browser interactions, and writes eight screenshots with empty
password fields. It never connects to an existing server or production database.
"""
import argparse, json, os, socket, subprocess, tempfile, time, urllib.request
from pathlib import Path
from urllib.parse import quote
from playwright.sync_api import sync_playwright, expect
ROOT = Path(__file__).resolve().parents[1]
CONFIG = {'lite-cdp': ('lite-cdp', 'CDP', 'cdp.theme', 0)}
PASSWORD = 'Temporary QA password 2026'
def settle(page):
 page.evaluate('document.fonts.ready')
 page.evaluate('() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))')
def snapshot(page, repo, kind, key):
 out = args.output_dir
 out.mkdir(parents=True,exist_ok=True)
 for width, name in [(1440,'desktop'),(390,'mobile')]:
  page.set_viewport_size({'width':width,'height':1200 if kind=='review' else (1000 if width==1440 else 844)})
  for theme in ['light','dark']:
   page.evaluate('([key,value]) => localStorage.setItem(key,value)', [key,theme])
   page.reload(wait_until='networkidle')
   if kind=='register':
    expect(page.locator('#register-username')).to_be_visible()
    assert page.locator('#register-username').bounding_box()['width'] >= 220
    assert page.locator('#register-confirmation').bounding_box()['width'] >= 220
   else:
    expect(page.locator('.registration-admin')).to_be_visible()
    expect(page.get_by_role('button',name='核准 qa.reader',exact=True)).to_be_visible()
    assert page.locator('#registration-mode').bounding_box()['width'] >= 150
   settle(page)
   assert page.evaluate('document.documentElement.scrollWidth <= innerWidth'), (repo,kind,name,theme,'overflow')
   path = out/f'registration-{kind}-{name}-{theme}.png'
   if kind=='review':
    box=page.locator('.registration-admin').bounding_box()
    page.screenshot(path=str(path),full_page=True,clip={'x':0,'y':box['y']+page.evaluate('scrollY')-8,'width':width,'height':box['height']+16},animations='disabled')
   else: page.screenshot(path=str(path),full_page=True,animations='disabled')
   print(json.dumps({'screenshot':str(path),'kind':kind,'width':width,'theme':theme}),flush=True)
 page.set_viewport_size({'width':1440,'height':1000})
def fill_signup(page, username, password=PASSWORD, confirmation=PASSWORD):
 page.get_by_label('帳號',exact=True).fill(username)
 page.get_by_label('密碼',exact=True).fill(password)
 page.get_by_label('確認密碼',exact=True).fill(confirmation)
 page.get_by_role('button',name='建立帳號',exact=True).click()
def login(page, base, username, password=PASSWORD, suffix=''):
 page.goto(base+'/login'+suffix,wait_until='networkidle')
 page.get_by_label('帳號',exact=True).fill(username)
 page.get_by_label('密碼',exact=True).fill(password)
 page.get_by_role('button',name='登入',exact=True).click()
def run(repo, browser, args):
 binary,prefix,key,port=CONFIG[repo]
 port=args.port
 if not port:
  with socket.socket() as address:
   address.bind(('127.0.0.1',0));port=address.getsockname()[1]
 base=f'http://127.0.0.1:{port}'
 temp=tempfile.TemporaryDirectory(prefix=f'registration-{repo}-')
 log=open(Path(temp.name)/'server.log','w')
 env=dict(os.environ,ACCESS_PUBLIC_URL=base)
 env.update({prefix+'_BIND':f'127.0.0.1:{port}',prefix+'_DATA_DIR':temp.name,prefix+'_STATIC_DIR':str(ROOT/'src/web/dist'),prefix+'_PUBLIC_URL':base,prefix+'_COOKIE_SECURE':'false'})
 server=subprocess.Popen([str(args.binary)],cwd=str(ROOT),env=env,stdout=log,stderr=log)
 contexts=[]
 errors=[]
 try:
  for _ in range(100):
   if server.poll() is not None:raise RuntimeError('server exited: '+Path(log.name).read_text())
   try:
    urllib.request.urlopen(base+'/api/auth/state',timeout=1).close();break
   except Exception: time.sleep(.1)
  admin=browser.new_context(viewport={'width':1440,'height':1000},reduced_motion='reduce');contexts.append(admin)
  p=admin.new_page();p.on('pageerror',lambda e:errors.append(str(e)))
  p.goto(base+'/register',wait_until='networkidle')
  expect(p.get_by_role('button',name='建立管理員',exact=True)).to_be_visible()
  p.get_by_label('帳號',exact=True).fill('qa.admin')
  p.get_by_label('密碼',exact=True).fill(PASSWORD)
  p.get_by_label('確認密碼',exact=True).fill(PASSWORD)
  p.get_by_role('button',name='建立管理員',exact=True).click()
  expect(p.get_by_role('button',name='登出',exact=True)).to_be_visible()
  adminpath='/users' if repo=='bigdata-keypo' else '/settings'
  p.goto(base+adminpath,wait_until='networkidle')
  expect(p.locator('#registration-mode')).to_have_value('closed')
  anon=browser.new_context(viewport={'width':1440,'height':1000},reduced_motion='reduce');contexts.append(anon)
  q=anon.new_page();q.on('pageerror',lambda e:errors.append(str(e)))
  q.goto(base+'/register',wait_until='networkidle')
  expect(q.get_by_role('heading',name='目前未開放註冊',exact=True)).to_be_visible()
  expect(q.get_by_label('確認密碼',exact=True)).to_have_count(0)
  p.get_by_label('註冊方式',exact=True).select_option('approval')
  p.get_by_role('button',name='儲存註冊方式',exact=True).click()
  expect(p.get_by_text('註冊方式已更新。',exact=True)).to_be_visible()
  q.reload(wait_until='networkidle')
  snapshot(q,repo,'register',key)
  q.goto(base+'/register?next='+quote('/oauth/authorize?client_id=qa-safe',safe=''),wait_until='networkidle')
  expect(q.get_by_label('帳號',exact=True)).to_have_attribute('maxlength','64')
  submitted=[]
  q.on('request',lambda request:submitted.append(request.url) if request.method=='POST' and request.url.endswith('/api/auth/register') else None)
  fill_signup(q,'qa.reader',password='short',confirmation='short')
  expect(q.get_by_role('alert')).to_contain_text('12 到 256')
  fill_signup(q,'qa.reader',confirmation='Different password 2026')
  expect(q.get_by_role('alert')).to_contain_text('兩次輸入的密碼不一致')
  assert submitted==[]
  fill_signup(q,'qa.reader')
  expect(q.get_by_role('heading',name='申請已送出，等待審核',exact=True)).to_be_visible()
  assert len(submitted)==1
  assert anon.request.get(base+'/api/auth/state').json()['user'] is None
  assert not anon.cookies(),anon.cookies()
  assert anon.request.get(base+'/api/access/overview').status in (401,403)
  assert q.get_by_role('link',name='已有帳號？返回登入').get_attribute('href')=='/login?next=%2Foauth%2Fauthorize%3Fclient_id%3Dqa-safe'
  login(q,base,'qa.reader')
  expect(q.get_by_role('alert')).to_contain_text('審核')
  p.get_by_role('button',name='更新申請與紀錄',exact=True).click()
  expect(p.get_by_role('button',name='核准 qa.reader',exact=True)).to_be_visible()
  snapshot(p,repo,'review',key)
  p.get_by_role('button',name='核准 qa.reader',exact=True).click()
  expect(p.get_by_text('已核准 qa.reader，現在可以登入。',exact=True)).to_be_visible()
  login(q,base,'qa.reader',suffix='?next='+quote('/oauth/authorize?client_id=qa-safe',safe=''))
  q.wait_for_url('**/oauth/authorize?client_id=qa-safe')
  assert q.url.startswith(base+'/oauth/authorize?')
  assert anon.request.get(base+'/api/auth/state').json()['user']['username']=='qa.reader'
  q.goto(base+'/',wait_until='networkidle')
  expect(q.get_by_role('button',name='登出',exact=True)).to_be_visible()
  rejected=browser.new_context(viewport={'width':1440,'height':1000},reduced_motion='reduce');contexts.append(rejected)
  r=rejected.new_page();r.on('pageerror',lambda e:errors.append(str(e)))
  r.goto(base+'/register',wait_until='networkidle');fill_signup(r,'qa.rejected')
  expect(r.get_by_role('heading',name='申請已送出，等待審核',exact=True)).to_be_visible()
  p.get_by_role('button',name='更新申請與紀錄',exact=True).click()
  expect(p.get_by_role('button',name='拒絕 qa.rejected',exact=True)).to_be_disabled()
  p.get_by_label('qa.rejected 的拒絕原因',exact=True).fill('QA: team membership could not be verified')
  p.get_by_role('button',name='拒絕 qa.rejected',exact=True).click()
  expect(p.get_by_text('已拒絕 qa.rejected。',exact=True)).to_be_visible()
  login(r,base,'qa.rejected');expect(r.get_by_role('alert')).to_contain_text('拒絕')
  assert rejected.request.get(base+'/api/auth/state').json()['user'] is None
  p.get_by_label('註冊方式',exact=True).select_option('open')
  p.get_by_role('button',name='儲存註冊方式',exact=True).click()
  expect(p.get_by_text('註冊方式已更新。',exact=True)).to_be_visible()
  r.goto(base+'/register?next=https%3A%2F%2Fevil.example%2F',wait_until='networkidle')
  boundary_username='reader.'+'r'*57
  boundary_password='p'*256
  r.get_by_label('顯示名稱（選填）',exact=True).fill('A'*100)
  fill_signup(r,boundary_username,password=boundary_password,confirmation=boundary_password)
  expect(r.get_by_role('button',name='登出',exact=True)).to_be_visible()
  assert r.url.startswith(base+'/') and 'evil.example' not in r.url
  assert rejected.request.get(base+'/api/auth/state').json()['user']['username']==boundary_username
  assert rejected.request.get(base+'/api/auth/state').json()['user']['display_name']=='A'*100
  r.goto(base+'/settings',wait_until='networkidle')
  field=r.get_by_label('顯示名稱',exact=True)
  expect(field).to_have_attribute('maxlength','100')
  field.fill('B'*100)
  field.locator('xpath=ancestor::form').get_by_role('button',name='儲存',exact=True).click()
  r.wait_for_function('async () => (await (await fetch("/api/auth/state")).json()).user.display_name === "B".repeat(100)')
  r.get_by_role('button',name='登出',exact=True).click()
  login(r,base,boundary_username,password=boundary_password)
  expect(r.get_by_role('button',name='登出',exact=True)).to_be_visible()
  assert rejected.request.get(base+'/api/auth/state').json()['user']['display_name']=='B'*100
  if repo=='bigdata-keypo':assert len(rejected.request.get(base+'/api/filters').json())==5
  p.get_by_label('申請狀態',exact=True).select_option('rejected')
  expect(p.get_by_text('拒絕原因：QA: team membership could not be verified',exact=True)).to_be_visible()
  assert not errors,errors
  result={'repo':repo,'browser':browser.version,'checks':['first-admin','closed','approval','password-limits','confirmation-no-request','pending-no-cookie','pending-no-access','approve','safe-continuation','approved-login','reject-reason','rejected-login-denied','open','unsafe-continuation-dropped','audit','eight-screens','no-overflow','no-pageerrors','username64','password256','display100-profile-edit','boundary-logout-login']}
  print(json.dumps(result),flush=True)
 finally:
  for context in contexts:context.close()
  server.terminate()
  try:server.wait(timeout=10)
  except subprocess.TimeoutExpired:server.kill();server.wait()
  log.close();temp.cleanup()
if __name__ == '__main__':
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--binary',type=Path,default=ROOT/'src/api/target/debug/lite-cdp',help='Built Rust host binary; defaults to src/api/target/debug/lite-cdp')
 parser.add_argument('--port',type=int,default=0,help='Local port; default chooses an available port')
 parser.add_argument('--channel',default='chrome',help='Playwright browser channel; default uses installed Google Chrome')
 parser.add_argument('--output-dir',type=Path,default=ROOT/'docs/images',help='Directory for eight password-free screenshots')
 args=parser.parse_args()
 args.binary=args.binary.resolve();args.output_dir=args.output_dir.resolve()
 if not args.binary.is_file():parser.error('Build the API first or pass --binary with an existing host executable')
 if not (ROOT/'src/web/dist/index.html').is_file():parser.error('Build src/web first')
 with sync_playwright() as playwright:
  browser=playwright.chromium.launch(headless=True,channel=args.channel)
  try:run('lite-cdp',browser,args)
  finally:browser.close()
