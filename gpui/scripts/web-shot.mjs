// Headless screenshot of a page via the Chrome DevTools Protocol. No npm deps (Node >= 22).
//   node scripts/web-shot.mjs <chrome-binary> <url> <out.png> <width> <height> <wait-ms>
// Launches chrome-headless-shell with no display access, waits for index.html to set
// window.__galleryReady (wasm loaded, window opened), waits <wait-ms> more for the first
// frames, then captures the viewport. Browser console output is echoed to stderr.
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [chrome, url, out, width, height, waitMs] = process.argv.slice(2);
const W = Number(width), H = Number(height), WAIT = Number(waitMs ?? 1500);
const profile = mkdtempSync(join(process.env.TMPDIR || tmpdir(), "audiocn-web-shot-"));

// GPU in headless: WebGPU on SwiftShader (Vulkan) is tried first by gpui_web; if it is
// unavailable gpui_web falls back to WebGL2 on ANGLE/SwiftShader. Both are software.
const flags = [
  "--headless",
  "--no-first-run",
  "--no-default-browser-check",
  "--hide-scrollbars",
  "--mute-audio",
  `--user-data-dir=${profile}`,
  "--remote-debugging-port=0",
  `--window-size=${W},${H}`,
  "--force-device-scale-factor=1",
  "--use-angle=swiftshader",
  "--enable-unsafe-swiftshader",
  "--enable-webgl",
  "--ignore-gpu-blocklist",
  ...(process.env.WEB_SHOT_WEBGPU === "0"
    ? []
    : ["--enable-unsafe-webgpu", "--use-webgpu-adapter=swiftshader", "--enable-features=Vulkan"]),
  ...(process.env.WEB_SHOT_FLAGS ? process.env.WEB_SHOT_FLAGS.split(/\s+/) : []),
  "about:blank",
];
const env = { ...process.env };
delete env.DISPLAY;
delete env.WAYLAND_DISPLAY;
const proc = spawn(chrome, flags, { env, stdio: ["ignore", "ignore", "pipe"] });

let done = false;
const fail = (msg) => {
  if (done) return;
  done = true;
  console.error(`web-shot: ${msg}`);
  cleanup(1);
};
function cleanup(code) {
  try { proc.kill("SIGKILL"); } catch {}
  setTimeout(() => {
    try { rmSync(profile, { recursive: true, force: true }); } catch {}
    process.exit(code);
  }, 200);
}
setTimeout(() => fail("timed out after 90s"), 90_000).unref();
proc.on("exit", (code) => fail(`chrome exited early (${code})`));

const wsUrl = await new Promise((resolve) => {
  let buf = "";
  proc.stderr.on("data", (d) => {
    buf += d;
    const m = buf.match(/DevTools listening on (ws:\/\/\S+)/);
    if (m) resolve(m[1]);
  });
});
const port = new URL(wsUrl).port;
const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find((t) => t.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });

let nextId = 0;
const pending = new Map();
ws.onmessage = (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result);
  } else if (msg.method === "Runtime.consoleAPICalled") {
    const text = msg.params.args.map((a) => a.value ?? a.description ?? "").join(" ");
    console.error(`[console.${msg.params.type}] ${text}`);
  } else if (msg.method === "Runtime.exceptionThrown") {
    console.error(`[exception] ${msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text}`);
  }
};
const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const id = ++nextId;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });

try {
  await send("Runtime.enable");
  await send("Page.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url });
  let state = null;
  for (let i = 0; i < 600 && !state; i++) {
    await new Promise((r) => setTimeout(r, 100));
    const res = await send("Runtime.evaluate", {
      expression: "window.__galleryError ? 'error: ' + window.__galleryError : (window.__galleryReady ? 'ready' : '')",
      returnByValue: true,
    });
    state = res.result.value || null;
  }
  if (state !== "ready") throw new Error(`gallery did not start: ${state ?? "timeout"}`);
  await new Promise((r) => setTimeout(r, WAIT));
  const shot = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(out, Buffer.from(shot.data, "base64"));
  done = true;
  ws.close();
  cleanup(0);
} catch (e) {
  fail(e.message);
}
