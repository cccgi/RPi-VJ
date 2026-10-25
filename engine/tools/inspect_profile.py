from pathlib import Path
from playwright.sync_api import sync_playwright
from fetch_shadertoy import find_edge_executable
with sync_playwright() as p:
 c=p.chromium.launch_persistent_context(str(Path('E:/__Projects/RPi-VJ/target/shadertoy-edge-profile').resolve()),executable_path=find_edge_executable(),headless=False,args=['--disable-blink-features=AutomationControlled']);pg=c.pages[0] if c.pages else c.new_page();pg.goto('https://www.shadertoy.com/user/diatribes',wait_until='domcontentloaded',timeout=90000);pg.wait_for_timeout(3000);print(pg.locator('body').inner_text()[-2000:]);print(pg.eval_on_selector_all('a','els=>els.map(e=>({t:e.innerText,h:e.href})).filter(x=>x.t||x.h.includes("page"))')) ; c.close()
