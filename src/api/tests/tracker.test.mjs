import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { readFileSync } from 'node:fs';
import { webcrypto } from 'node:crypto';

const source = readFileSync(new URL('../src/tracker.js', import.meta.url), 'utf8');
function browser(policy = {}, options = {}) {
  const local = options.shared || new Map(options.seed || []), session = new Map(), listeners = {}, timers = new Map(), sent = [];
  let tick = 0;
  const storage = (map) => ({getItem:k=>map.get(k)||null,setItem:(k,v)=>map.set(k,String(v)),removeItem:k=>map.delete(k)});
  const script = {src:'https://cdp.example.com/t.js',getAttribute:n=>({'data-key':'site-key-123',...options})[n]??null};
  const document = { currentScript:script,title:'Account',referrer:'https://search.example/path?private=yes',visibilityState:'visible',documentElement:{scrollHeight:2000,clientHeight:600},body:{scrollHeight:2000},querySelector:()=>script,addEventListener:(n,f)=>(listeners[n]??=[]).push(f),removeEventListener:(n,f)=>{listeners[n]=(listeners[n]||[]).filter(x=>x!==f);} };
  const location = {href:'https://shop.example/account?email=private&campaign=spring#secret'};
  const fetch = async (url, init) => {
    if (!init || init.method !== 'POST') return {ok:true,json:async()=>({site_id:7,namespace:'site_7',active:true,policy:{consent_required:false,respect_dnt:true,auto_page:true,auto_click:true,auto_scroll:true,spa_mode:'history',query_mode:'strip',referrer_scope:'origin',click_text:false,allowed_traits:[],...policy}})};
    sent.push(JSON.parse(init.body));
    if (options.failures && sent.length <= options.failures) throw new Error("offline");
    return {ok:true,status:204};
  };
  const window = {crypto:webcrypto,localStorage:storage(local),sessionStorage:storage(session),fetch,innerHeight:600,addEventListener:document.addEventListener,removeEventListener:document.removeEventListener,dispatchEvent:()=>{},console:{warn:()=>{}},location,cdpq:options.commands||[]};
  const navigator = {doNotTrack:options.dnt||'0'};
  const history = {pushState:(_a,_b,url)=>{location.href=new URL(url,location.href).href;},replaceState:(_a,_b,url)=>{location.href=new URL(url,location.href).href;}};
  const context = vm.createContext({window,document,navigator,location,history,fetch,URL,URLSearchParams,Uint8Array,Date,JSON,Blob,console:window.console,CustomEvent:class{},setTimeout:(fn)=>{timers.set(++tick,fn);return tick;},clearTimeout:id=>timers.delete(id)});
  const load = () => vm.runInContext(source,context);
  const settle = async () => {for(let i=0;i<12;i++)await Promise.resolve();};
  const flush = async () => {const callbacks=[...timers.values()];timers.clear();for(const fn of callbacks)fn();await settle();};
  load();
  return {window,document,script,local,session,sent,listeners,history,location,load,settle,flush};
}

test('required consent creates no IDs or events until grant; revoke erases and stops',async()=>{
  const b=browser({consent_required:true});await b.settle();await b.flush();
  assert.equal(b.local.size,0);assert.equal(b.session.size,0);assert.equal(b.sent.length,0);
  b.window.cdp.consent(true);await b.flush();
  assert.equal(b.sent.length,1);assert.equal(b.sent[0].consent,true);
  b.window.cdp.consent(false);b.window.cdp.track('private');await b.flush();
  assert.equal(b.sent.length,1);assert.equal([...b.local.keys()].some(k=>k.includes('anon')),false);
});
test('DNT blocks persistent identity even after explicit consent',async()=>{
  const b=browser({respect_dnt:true},{dnt:'1'});await b.settle();
  b.window.cdp.consent(true);await b.flush();
  assert.equal(b.local.size,0);assert.equal(b.sent.length,0);
});
test('same loader is idempotent; conflicting key is visible and never changes destination',async()=>{
  const b=browser();await b.settle();b.load();await b.flush();
  assert.equal(b.sent[0].events.length,1);
  b.script.getAttribute=n=>n==='data-key'?'other-key':null;b.load();
  assert.equal(b.window.cdp.status().last_error,'conflicting_installation');
  b.window.cdp.track('checkout');await b.flush();assert.equal(b.sent.at(-1).key,'site-key-123');
});
test('sanitization happens before transport and events have stable IDs and times',async()=>{
  const b=browser({query_mode:'allowlist',query_allowlist:['campaign']});await b.settle();await b.flush();
  const event=b.sent[0].events[0];assert.equal(event.url,'https://shop.example/account?campaign=spring');
  assert.equal(event.referrer,'https://search.example');assert.match(event.event_id,/^[a-f0-9]{24}$/);assert.ok(Date.parse(event.happened_at));
});
test('manual SPA mode does not double count router navigation',async()=>{
  const b=browser({spa_mode:'manual',auto_page:false});await b.settle();b.history.pushState({},'', '/next');await b.flush();
  assert.equal(b.sent.length,0);b.window.cdp.page();await b.flush();assert.equal(b.sent[0].events.length,1);
});

test('denial before config completes clears an existing site identity',async()=>{
  const b=browser({consent_required:true},{seed:[['cdp_site_7_anon_id','old-identity']]});
  b.window.cdp.consent(false);await b.settle();await b.flush();
  assert.equal(b.local.has('cdp_site_7_anon_id'),false);assert.equal(b.sent.length,0);
});
test('logout in one tab rotates a sibling tab before its next event',async()=>{
  const shared=new Map();const a=browser({}, {shared}),b=browser({}, {shared});
  await a.settle();await b.settle();await a.flush();await b.flush();
  const previous=b.sent[0].anon_id;a.window.cdp.reset();b.window.cdp.track('after-logout');await b.flush();
  assert.notEqual(b.sent.at(-1).anon_id,previous);assert.equal(b.sent.at(-1).anon_id,a.local.get('cdp_site_7_anon_id'));
});
test('retries retain event IDs and stop after three transport attempts',async()=>{
  const b=browser({}, {failures:20});await b.settle();for(let i=0;i<6;i++)await b.flush();
  assert.equal(b.sent.length,3);assert.equal(new Set(b.sent.map(p=>p.events[0].event_id)).size,1);
  assert.equal(b.window.cdp.status().dropped,1);
});
test('hash router records route changes and required consent discards old actions',async()=>{
  const b=browser({spa_mode:'hash',consent_required:true});await b.settle();b.window.cdp.track('before-grant');b.window.cdp.consent(true);await b.flush();
  b.location.href='https://shop.example/#/next';for(const fn of b.listeners.hashchange||[])fn();await b.flush();await b.flush();
  assert.equal(b.sent.flatMap(p=>p.events).length,2);assert.equal(b.sent.flatMap(p=>p.events).some(e=>e.name==='before-grant'),false);
});

test('ready initialization grants consent before a queued login and legacy ID is claimed once',async()=>{
  const shared=new Map([['cdp_anon_id','legacy-browser-123']]);
  const b=browser({consent_required:true},{shared,commands:[['ready',tracker=>{tracker.consent(true);tracker.identify({user_id:'member-ready'});}]]});await b.settle();await b.flush();
  assert.equal(b.sent[0].anon_id,'legacy-browser-123');assert.equal(b.sent[0].identify.user_id,'member-ready');
  assert.equal(shared.get('cdp_legacy_claim'),'cdp_site_7');
});
test('invalid custom data cannot throw into the host application',async()=>{
  const b=browser();await b.settle();await b.flush();const cyclic={};cyclic.self=cyclic;
  b.window.cdp.track('cycle',cyclic);assert.doesNotThrow(()=>b.window.cdp.flush());await b.settle();
  assert.equal(b.window.cdp.status().last_error,'invalid_payload');
  b.window.cdp.track('valid');b.window.cdp.flush();await b.settle();assert.equal(b.sent.at(-1).events[0].name,'valid');
});
test('large batches are split below the collector body limit without changing event IDs',async()=>{
  const b=browser();await b.settle();await b.flush();
  for(let i=0;i<20;i++)b.window.cdp.track('large',{text:'x'.repeat(3800)});
  await b.settle();const payloads=b.sent.slice(1);assert.ok(payloads.length>=2);
  assert.ok(payloads.every(p=>Buffer.byteLength(JSON.stringify(p))<65536));
  assert.equal(new Set(payloads.flatMap(p=>p.events.map(e=>e.event_id))).size,20);
});
