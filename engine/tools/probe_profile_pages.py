import re
from pathlib import Path
from playwright.sync_api import sync_playwright
from fetch_shadertoy import find_edge_executable
with sync_playwright() as p:
 c=p.chromium.launch_persistent_context(str(Path('E:/__Projects/RPi-VJ/target/shadertoy-edge-profile').resolve()),executable_path=find_edge_executable(),headless=False,args=['--disable-blink-features=AutomationControlled'])
 pg=c.pages[0] if c.pages else c.new_page()
 for n in (1,2,99):
  pg.goto(f'https://www.shadertoy.com/user/diatribes?page={n}',wait_until='domcontentloaded',timeout=90000);pg.wait_for_timeout(2500)
  hs=pg.eval_on_selector_all('a[href*="/view/"]','els=>els.map(e=>e.href)');ids=[]
  for h in hs:
   m=re.search(r'/view/([A-Za-z0-9]+)',h)
   if m and m.group(1) not in ids:ids.append(m.group(1))
  print(n,len(ids),ids[:3],flush=True)
 c.close()
