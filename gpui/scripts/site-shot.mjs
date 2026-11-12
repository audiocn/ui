// node site-shot.mjs <chrome> <url> <out.png> <w> <h> <dpr> <wait> [dark] [scrollSelector]
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
const [chrome, url, out, w, h, dpr, wait, dark, sel] = process.argv.slice(2);
const profile = mkdtempSync(join(tmpdir(), "site-shot-"));
const proc = spawn(chrome, ["--headless","--no-first-run","--hide-scrollbars",`--user-data-dir=${profile}`,"--remote-debugging-port=0",`--window-size=${w},${h}`,"about:blank"], { stdio: ["ignore","ignore","pipe"] });
setTimeout(() => { console.error("timeout"); proc.kill("SIGKILL"); process.exit(1); }, 90000).unref();
const wsUrl = await new Promise((r) => { let b=""; proc.stderr.on("data", d => { b+=d; const m=b.match(/DevTools listening on (ws:\/\/\S+)/); if (m) r(m[1]); }); });
const port = new URL(wsUrl).port;
const page = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).find(t => t.type==="page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j;});
let id=0; const pend=new Map();
ws.onmessage = ev => { const m=JSON.parse(ev.data); if (m.id&&pend.has(m.id)) { const p=pend.get(m.id); pend.delete(m.id); m.error?p.j(new Error(JSON.stringify(m.error))):p.r(m.result);} };
const send=(method,params={})=>new Promise((r,j)=>{const i=++id;pend.set(i,{r,j});ws.send(JSON.stringify({id:i,method,params}));});
await send("Page.enable"); await send("Runtime.enable");
await send("Emulation.setDeviceMetricsOverride",{width:+w,height:+h,deviceScaleFactor:+dpr,mobile:false});
if (dark==="dark") await send("Emulation.setEmulatedMedia",{features:[{name:"prefers-color-scheme",value:"dark"}]});
await send("Page.navigate",{url});
await new Promise(r=>setTimeout(r,+wait));
if (dark==="dark") { await send("Runtime.evaluate",{expression:"document.documentElement.classList.add('dark');document.documentElement.classList.remove('light');localStorage.setItem('theme','dark')"}); await new Promise(r=>setTimeout(r,500)); }
if (process.env.CLICK) {
  const r = await send("Runtime.evaluate",{expression:`(()=>{const e=document.querySelectorAll(${JSON.stringify(process.env.CLICK)})[${process.env.CLICK_IDX||0}]; e.scrollIntoView({block:'center'}); const r=e.getBoundingClientRect(); return JSON.stringify({x:r.x+r.width/2,y:r.y+r.height/2});})()`,returnByValue:true});
  await new Promise(r=>setTimeout(r,500));
  const c = JSON.parse(r.result.value);
  for (const type of ["mouseMoved","mousePressed","mouseReleased"]) await send("Input.dispatchMouseEvent",{type,x:c.x,y:c.y,button:"left",clickCount:1});
  await new Promise(r=>setTimeout(r,800));
}
let clip;
if (sel) {
  const res = await send("Runtime.evaluate",{expression:`(()=>{const e=document.querySelectorAll(${JSON.stringify(sel)})[${process.env.IDX||0}]; if(!e) return null; ${process.env.CLICK ? '' : "e.scrollIntoView({block:'center'});"} const r=e.getBoundingClientRect(); return JSON.stringify({x:r.x+scrollX,y:r.y+scrollY,width:r.width,height:r.height});})()`,returnByValue:true});
  await new Promise(r=>setTimeout(r,800));
  if (res.result.value) { const c=JSON.parse(res.result.value); const pad=+(process.env.PAD||16); clip={x:c.x-pad,y:c.y-pad,width:c.width+2*pad,height:c.height+2*pad,scale:1}; }
}
const shot = await send("Page.captureScreenshot",{format:"png",...(clip?{clip,captureBeyondViewport:true}:{})});
writeFileSync(out, Buffer.from(shot.data,"base64"));
ws.close(); proc.kill("SIGKILL"); setTimeout(()=>{try{rmSync(profile,{recursive:true,force:true})}catch{};process.exit(0)},200);
