#!/usr/bin/env python3
"""Exercise built nginx/API images in a disposable, isolated Compose project.
Run `docker compose build` first. No existing volumes or production servers are used.
"""
import json, os, secrets, socket, subprocess, tempfile, time, urllib.error, urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
with socket.socket() as sock:
    sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
origin=f'http://127.0.0.1:{port}'
project='signal-check-'+secrets.token_hex(4)
env={k:v for k,v in os.environ.items() if not k.startswith(('ACCESS_','CDP_'))}
env.update(CDP_PORT=str(port),CDP_PUBLIC_URL=origin,ACCESS_PUBLIC_URL=origin,CDP_COOKIE_SECURE='false')
with tempfile.TemporaryDirectory(prefix=project) as tmp:
    override=Path(tmp)/'override.yml'
    override.write_text(f'''services:
  lite-cdp:
    container_name: {project}-api
  nginx:
    container_name: {project}-nginx
    ports: !override
      - "127.0.0.1:{port}:80"
''')
    command=['docker','compose','--env-file','/dev/null','-p',project,'-f',str(ROOT/'docker-compose.yml'),'-f',str(override)]
    def compose(*args):
        return subprocess.run(command+list(args),env=env,cwd=ROOT,check=True,capture_output=True,text=True)
    cookie=''
    def call(method,path,data=None,expected=200,tracking=False):
        headers={'Origin':'https://shop.example' if tracking else origin,'User-Agent':'SignalVerification/1.0'}
        if cookie:headers['Cookie']=cookie
        if data is not None:headers['Content-Type']='text/plain' if tracking else 'application/json'
        request=urllib.request.Request(origin+path,data=json.dumps(data).encode() if data is not None else None,headers=headers,method=method)
        try:response=urllib.request.urlopen(request,timeout=10)
        except urllib.error.HTTPError as error:response=error
        with response:
            raw=response.read();assert response.status==expected,(method,path,response.status,raw[:300])
            return (json.loads(raw) if raw and 'json' in response.headers.get('Content-Type','') else raw),response.headers
    try:
        compose('up','-d','--no-build')
        for _ in range(120):
            try:call('GET','/api/health');break
            except (OSError,AssertionError):time.sleep(.25)
        else:raise AssertionError('backend failed to start')
        call('GET','/healthz');call('GET','/assets/missing.js',expected=404)
        _,headers=call('POST','/api/auth/setup',{'username':'admin','password':secrets.token_urlsafe(24)})
        cookie=headers['Set-Cookie'].split(';')[0]
        site,_=call('POST','/api/sites',{'name':'Signal Compose','allowed_origins':['https://shop.example'],'collection_policy':{'consent_required':True,'query_mode':'strip','auto_click':False}})
        sid,key=site['id'],site['key']
        config,headers=call('GET','/tracker-config/'+key)
        assert config['namespace']==f'site_{sid}' and config['policy']['consent_required'] is True and 'secret' not in json.dumps(config)
        assert headers['Access-Control-Allow-Origin']=='*' and headers['Cache-Control']=='no-store'
        script,headers=call('GET','/t.js');assert b'conflicting_installation' in script and 'javascript' in headers['Content-Type']
        payload={'key':key,'anon_id':'compose-browser-1','events':[{'type':'pageview','event_id':'compose-event-001','url':'https://shop.example/pricing?private=yes'}]}
        call('POST','/collect',payload,403,True)
        payload['consent']=True
        call('POST','/collect',payload,204,True);call('POST','/collect',payload,204,True)
        people,_=call('GET',f'/api/sites/{sid}/people');assert people['total']==1 and people['items'][0]['pageviews']==1
        pid=people['items'][0]['id'];events,_=call('GET',f'/api/sites/{sid}/people/{pid}/events');assert events['items'][0]['url']=='https://shop.example/pricing'
        token,_=call('POST',f'/api/sites/{sid}/installation-tests',{})
        payload.update(anon_id='compose-test-browser',test_token=token['token']);payload['events'][0]['event_id']='compose-test-001'
        call('POST','/collect',payload,204,True)
        receipt,_=call('GET',f'/api/sites/{sid}/installation-tests/'+token['token']);assert receipt['receipt']['analytics_written'] is False
        people,_=call('GET',f'/api/sites/{sid}/people');assert people['total']==1 and people['items'][0]['pageviews']==1
        backend=compose('ps','-q','lite-cdp').stdout.strip()
        inspect=json.loads(subprocess.run(['docker','inspect',backend],capture_output=True,text=True,check=True).stdout)[0]
        assert inspect['Config']['User']=='app' and not any(inspect['NetworkSettings']['Ports'].values())
        result={'status':'passed','checks':['built nginx static and API','public config CORS/no-store/no-secret','real tracker route','required consent enforcement','event replay deduplication','URL sanitization','isolated installation receipt','nonroot backend with no published port'],'disposable_project':True}
        (ROOT/'docs/signal-console-compose.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
    finally:
        compose('down','--volumes','--remove-orphans')
