import base64,json,re,time
from pathlib import Path
from playwright.sync_api import sync_playwright
from fetch_shadertoy import extract_shader_obj,find_edge_executable,collect_asset_entries,write_glsl
ROOT=Path('E:/__Projects/RPi-VJ/target/shadertoy-offline');OUT=Path('E:/__Projects/RPi-VJ/target/shadertoy-downloads')
FETCH="""async id=>{const p=new URLSearchParams();p.set('s',JSON.stringify({shaders:[id]}));p.set('nt','1');p.set('nl','1');p.set('np','1');const r=await fetch('https://www.shadertoy.com/shadertoy',{method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded; charset=UTF-8'},credentials:'include',body:p.toString()});return {status:r.status,text:await r.text()}}"""
ASSET="""async url=>{const r=await fetch(url,{credentials:'include'});if(!r.ok)return {status:r.status};const b=new Uint8Array(await r.arrayBuffer());let s='';for(let i=0;i<b.length;i+=0x8000)s+=String.fromCharCode(...b.subarray(i,i+0x8000));return {status:r.status,b64:btoa(s)}}"""
with sync_playwright() as p:
 c=p.chromium.launch_persistent_context(str(Path('E:/__Projects/RPi-VJ/target/shadertoy-edge-profile').resolve()),executable_path=find_edge_executable(),headless=False,args=['--disable-blink-features=AutomationControlled'])
 pg=c.pages[0] if c.pages else c.new_page();ids=[]
 for page_no in range(99):
  url = ('https://www.shadertoy.com/user/diatribes' if page_no == 0 else
         f'https://www.shadertoy.com/user/diatribes/sort=popular&from={page_no*8}&num=8');
  try:
   pg.goto(url,wait_until='domcontentloaded',timeout=90000)
  except Exception as exc:
   print(f'NAV_RETRY PAGE {page_no+1}: {exc}',flush=True)
  pg.wait_for_timeout(2500)
  for h in pg.eval_on_selector_all('a[href*="/view/"]','els=>els.map(e=>e.href)'):
   m=re.search(r'/view/([A-Za-z0-9]+)',h)
   if m and m.group(1) not in ids:ids.append(m.group(1))
  print(f'PAGE {page_no+1}/99 ids={len(ids)}',flush=True)
 print(f'PROFILE_IDS {len(ids)}',flush=True);existing={x.stem for x in OUT.glob('*.glsl')};ids=[x for x in ids if x not in existing];print(f'PROFILE_NEW {len(ids)}',flush=True)
 for n,sid in enumerate(ids,1):
  r=pg.evaluate(FETCH,sid)
  if r.get('status')!=200:print(f'FAIL {n}/{len(ids)} {sid}: HTTP {r.get("status")}',flush=True);time.sleep(12);continue
  try:shader=extract_shader_obj(json.loads(r['text']))
  except Exception:shader=None
  if not shader:print(f'FAIL {n}/{len(ids)} {sid}: payload',flush=True);time.sleep(12);continue
  sd=ROOT/sid;sd.mkdir(parents=True,exist_ok=True);(sd/'shader.json').write_text(json.dumps(shader,indent=2),encoding='utf-8');write_glsl(sid,shader,OUT);entries=collect_asset_entries(shader);ok=0
  for i,e in enumerate(entries):
   try:a=pg.evaluate(ASSET,str(e.get('url') or ''))
   except Exception:a={'status':0}
   if a.get('status')==200 and a.get('b64'):(sd/str(e.get('filename') or f'asset-{i}.bin')).write_bytes(base64.b64decode(a['b64']));ok+=1
  (sd/'assets.json').write_text(json.dumps({'shader_id':sid,'assets':entries,'downloaded':ok},indent=2),encoding='utf-8');print(f'OK {n}/{len(ids)} {sid}: {ok}/{len(entries)} assets',flush=True);time.sleep(12)
 c.close()
