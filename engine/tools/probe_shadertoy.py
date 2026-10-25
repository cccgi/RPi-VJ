import json
from pathlib import Path
from playwright.sync_api import sync_playwright
from fetch_shadertoy import find_edge_executable
FETCH="""async id=>{const p=new URLSearchParams();p.set('s',JSON.stringify({shaders:[id]}));p.set('nt','1');p.set('nl','1');p.set('np','1');const r=await fetch('https://www.shadertoy.com/shadertoy',{method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded; charset=UTF-8'},credentials:'include',body:p.toString()});return {status:r.status,text:await r.text()}}"""
with sync_playwright() as p:
 c=p.chromium.launch_persistent_context(str(Path('E:/__Projects/RPi-VJ/target/shadertoy-edge-profile').resolve()),executable_path=find_edge_executable(),headless=False,args=['--disable-blink-features=AutomationControlled'])
 pg=c.pages[0] if c.pages else c.new_page(); pg.goto('https://www.shadertoy.com/view/WfS3Dd',wait_until='domcontentloaded',timeout=90000);pg.wait_for_timeout(5000); print('status',pg.evaluate(FETCH,'WfS3Dd').get('status')); input('Complete any verification in the Edge window, then press Enter...'); c.close()
