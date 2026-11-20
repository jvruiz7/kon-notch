// Sonda de interface no Windows: cliques REAIS do sistema (SetCursorPos/mouse_event) e leitura do DOM via CDP.
const { chromium } = require("playwright-core");
const { execFileSync, spawn } = require("child_process");
const fs = require("fs"), path = require("path");
const OUT = process.argv[2];
fs.mkdirSync(OUT, { recursive: true });
const log = (...a) => { const l = `[${new Date().toISOString().slice(11, 23)}] ${a.join(" ")}`; console.log(l); fs.appendFileSync(path.join(OUT, "log.txt"), l + "\n"); };
// PowerShell persistente com user32 carregado
const ps = spawn("powershell.exe", ["-NoProfile", "-Command", "-"], { stdio: ["pipe", "pipe", "inherit"] });
let psBuf = "", psWait = null;
ps.stdout.on("data", (d) => { psBuf += d; if (psBuf.includes("@@OK")) { const r = psBuf; psBuf = ""; psWait && psWait(r); } });
const run = (cmd) => new Promise((res) => { psWait = res; ps.stdin.write(cmd + "\nWrite-Output '@@OK'\n"); });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
(async () => {
  await run(`Add-Type -AssemblyName System.Drawing,System.Windows.Forms; Add-Type 'using System;using System.Runtime.InteropServices;public class M{[DllImport("user32.dll")]public static extern bool SetCursorPos(int x,int y);[DllImport("user32.dll")]public static extern void mouse_event(int f,int x,int y,int d,int e);[DllImport("user32.dll")]public static extern IntPtr GetForegroundWindow();[DllImport("user32.dll")]public static extern int GetWindowText(IntPtr h,System.Text.StringBuilder s,int n);}'`);
  const move = (x, y) => run(`[M]::SetCursorPos(${Math.round(x)},${Math.round(y)})`);
  const click = async (x, y) => { await move(x, y); await sleep(60); await run(`[M]::mouse_event(2,0,0,0,0); Start-Sleep -Milliseconds 40; [M]::mouse_event(4,0,0,0,0)`); };
  let shotN = 0;
  const shot = async (name) => { const f = path.join(OUT, `${String(++shotN).padStart(2, "0")}-${name}.png`); await run(`$b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds; $bmp=New-Object System.Drawing.Bitmap 1100,620; $g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen([int](($b.Width-1100)/2),0,0,0,$bmp.Size); $bmp.Save('${f}'); $g.Dispose(); $bmp.Dispose()`); };
  const fg = async () => (await run(`$s=New-Object System.Text.StringBuilder 256; [void][M]::GetWindowText([M]::GetForegroundWindow(),$s,256); Write-Output ("FG=" + $s.ToString())`)).match(/FG=(.*)/)?.[1]?.trim();

  // abre um app "de trabalho" para ter quem roube/perca o foco, como no PC real
  spawn("notepad.exe", [], { detached: true }); await sleep(2500);
  let browser;
  for (let i = 0; i < 40 && !browser; i++) { try { browser = await chromium.connectOverCDP("http://127.0.0.1:9222"); } catch { await sleep(1000); } }
  if (!browser) { log("SEM CDP"); process.exit(1); }
  const page = browser.contexts()[0].pages().find((p) => !p.url().includes("#ocr")) || browser.contexts()[0].pages()[0];
  page.on("pageerror", (e) => log("PAGEERROR", e.message));
  page.on("console", (m) => { if (["error", "warning"].includes(m.type())) log("CONSOLE", m.type(), m.text().slice(0, 300)); });
  await sleep(1500);
  const win = await page.evaluate(() => ({ x: window.screenX, y: window.screenY, w: innerWidth, h: innerHeight, dpr: devicePixelRatio, sw: screen.width }));
  log("janela", JSON.stringify(win), "foreground:", await fg());
  const S = (r) => ({ x: win.x + r.x + r.w / 2, y: win.y + r.y + r.h / 2 });
  const rectOf = (sel) => page.evaluate((sel) => {
    const pick = (s) => { if (s.title) return [...document.querySelectorAll("button")].find((b) => b.title === s.title); if (s.text) return [...document.querySelectorAll("button,div,span")].find((b) => b.textContent.trim() === s.text && b.offsetParent); if (s.idx != null) return document.querySelectorAll(s.nav)[s.idx]; };
    const el = pick(sel); if (!el) return null; const r = el.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height };
  }, sel);
  const state = () => page.evaluate(() => {
    const t = document.body.innerText;
    return { sheet: /Configurações|Arraste para reordenar/.test(t), panel: !!([...document.querySelectorAll("button")].find((b) => b.title === "Pages e configurações")), tab: ([...document.querySelectorAll("button")].find((b) => /bg-white|text-black/.test(b.className) && b.querySelector("svg") && b.innerText.trim().length > 2)?.innerText || "").trim().slice(0, 20) };
  });
  const cx = win.x + win.w / 2;
  await shot("inicio");

  log("1) passa o mouse no notch");
  await move(cx, win.y + 14); await sleep(700);
  log("estado", JSON.stringify(await state())); await shot("hover");

  log("2) clica em cada página da barra");
  const navCount = await page.evaluate(() => { const g = [...document.querySelectorAll("button")].find((b) => b.title === "Pages e configurações"); window.__nav = g ? [...g.parentElement.parentElement.querySelectorAll("button")] : []; return window.__nav.length; });
  log("botões na barra:", navCount);
  for (let i = 0; i < navCount; i++) {
    const r = await page.evaluate((i) => { const b = window.__nav[i]; if (!b || !b.offsetParent) return null; const q = b.getBoundingClientRect(); return { x: q.x, y: q.y, w: q.width, h: q.height, t: b.title || b.innerText.trim() }; }, i);
    if (!r || /Pages e configurações|Perguntar/.test(r.t)) continue;
    const before = JSON.stringify(await state()); const p = S(r); const t0 = Date.now();
    await click(p.x, p.y);
    let after = before; while (Date.now() - t0 < 1500) { after = JSON.stringify(await state()); if (after !== before) break; await sleep(30); }
    log(`  botão ${i} "${r.t}" → ${after} em ${Date.now() - t0} ms ${after === before ? "(SEM MUDANÇA)" : ""}`);
    await sleep(150);
  }
  await shot("paginas");

  log("3) abre Ajustes pela engrenagem");
  let r = await rectOf({ title: "Pages e configurações" });
  if (!r) { await move(cx, win.y + 14); await sleep(700); r = await rectOf({ title: "Pages e configurações" }); }
  log("engrenagem", JSON.stringify(r));
  if (r) { const p = S(r); await click(p.x, p.y); }
  for (const ms of [150, 400, 900, 1600]) { await sleep(ms === 150 ? 150 : ms - 150); log(`  +${ms} ms`, JSON.stringify(await state()), "fg:", await fg()); }
  await shot("ajustes-aberto");
  r = await rectOf({ text: "Ajustes" }); log("aba Ajustes", JSON.stringify(r));
  if (r) { const p = S(r); await click(p.x, p.y); await sleep(600); log("  depois de clicar Ajustes", JSON.stringify(await state()), "fg:", await fg()); }
  await shot("aba-ajustes");
  // clica num campo de texto e digita
  r = await page.evaluate(() => { const i = [...document.querySelectorAll("input")].find((x) => x.offsetParent && x.type !== "checkbox"); if (!i) return null; const q = i.getBoundingClientRect(); return { x: q.x, y: q.y, w: q.width, h: q.height }; });
  if (r) { const p = S(r); await click(p.x, p.y); await sleep(300); await run(`[System.Windows.Forms.SendKeys]::SendWait("abc")`); await sleep(400); log("  campo focado?", await page.evaluate(() => document.activeElement?.tagName + " valor=" + (document.activeElement?.value || "").slice(0, 5)), "fg:", await fg(), JSON.stringify(await state())); }
  await shot("digitou");
  // select nativo (Local)
  r = await page.evaluate(() => { const s = [...document.querySelectorAll("select")].find((x) => x.offsetParent); if (!s) return null; const q = s.getBoundingClientRect(); return { x: q.x, y: q.y, w: q.width, h: q.height }; });
  log("select", JSON.stringify(r));
  if (r) { const p = S(r); await click(p.x, p.y); await sleep(900); await shot("select-aberto"); log("  com o select aberto", JSON.stringify(await state()), "fg:", await fg()); await run(`[System.Windows.Forms.SendKeys]::SendWait("{ESC}")`); await sleep(500); log("  depois do ESC", JSON.stringify(await state())); }
  // rolar dentro dos ajustes
  await move(cx, win.y + 300); await run(`[M]::mouse_event(0x800,0,0,-360,0)`); await sleep(500); await shot("rolou"); log("  depois de rolar", JSON.stringify(await state()));

  log("4) clique fora (no Bloco de Notas) e volta");
  await click(cx + 500, 700); await sleep(800); log("  ", JSON.stringify(await state()), "fg:", await fg());
  await move(cx, win.y + 14); await sleep(800); log("  hover de novo", JSON.stringify(await state())); await shot("hover2");

  log("5) cliques rápidos alternando páginas (40 cliques)");
  const ids = await page.evaluate(() => { const g = [...document.querySelectorAll("button")].find((b) => b.title === "Pages e configurações"); window.__nav = g ? [...g.parentElement.parentElement.querySelectorAll("button")].filter((b) => b.offsetParent && !b.title) : []; return window.__nav.length; });
  const rects = await page.evaluate(() => window.__nav.map((b) => { const q = b.getBoundingClientRect(); return { x: q.x, y: q.y, w: q.width, h: q.height }; }));
  const t0 = Date.now();
  for (let k = 0; k < 40 && rects.length; k++) { const p = S(rects[k % rects.length]); await click(p.x, p.y); await sleep(70); }
  log(`  40 cliques em ${Date.now() - t0} ms`, JSON.stringify(await state()));
  await sleep(800); await shot("rapidos"); log("  depois", JSON.stringify(await state()));
  // ainda responde?
  r = await rectOf({ title: "Pages e configurações" });
  if (r) { const p = S(r); await click(p.x, p.y); await sleep(800); log("  engrenagem depois dos cliques rápidos", JSON.stringify(await state())); await shot("ajustes-depois"); }

  log("6) abre Ajustes pelo atalho do menu da bandeja não dá; testa Ctrl+Alt+Espaço (Perguntar)");
  await click(cx + 500, 700); await sleep(500);
  await run(`[System.Windows.Forms.SendKeys]::SendWait("^%( )")`); await sleep(900); log("  ", JSON.stringify(await state()), "foco:", await page.evaluate(() => document.activeElement?.tagName), "fg:", await fg()); await shot("perguntar");
  log("FIM");
  await browser.close().catch(() => {});
  ps.stdin.end(); process.exit(0);
})().catch((e) => { log("ERRO", e.stack); process.exit(1); });
