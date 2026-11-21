// Sonda de interface no Windows: cliques REAIS do sistema (SetCursorPos/mouse_event) e leitura do DOM via CDP.
const { chromium } = require("playwright-core");
const { execFileSync, spawn } = require("child_process");
const fs = require("fs"), path = require("path");
const OUT = process.argv[2];
fs.mkdirSync(OUT, { recursive: true });
let failures = 0;
const fail = (m) => { failures++; log("FALHA", m); };
const log = (...a) => { const l = `[${new Date().toISOString().slice(11, 23)}] ${a.join(" ")}`; console.log(l); fs.appendFileSync(path.join(OUT, "log.txt"), l + "\n"); };
// PowerShell persistente com user32 carregado
const ps = spawn("powershell.exe", ["-NoProfile", "-Command", "-"], { stdio: ["pipe", "pipe", "inherit"] });
let psBuf = "", psWait = null;
ps.stdout.on("data", (d) => { psBuf += d; if (psBuf.includes("@@OK")) { const r = psBuf; psBuf = ""; psWait && psWait(r); } });
const run = (cmd) => new Promise((res) => { psWait = res; ps.stdin.write(cmd + "\nWrite-Output '@@OK'\n"); });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
(async () => {
  await run(`Add-Type -AssemblyName System.Drawing,System.Windows.Forms; Add-Type 'using System;using System.Runtime.InteropServices;public class M{[DllImport("user32.dll")]public static extern bool SetCursorPos(int x,int y);[DllImport("user32.dll")]public static extern void mouse_event(int f,int x,int y,int d,int e);[DllImport("user32.dll")]public static extern IntPtr GetForegroundWindow();[DllImport("user32.dll")]public static extern int GetWindowText(IntPtr h,System.Text.StringBuilder s,int n);[DllImport("user32.dll")]public static extern IntPtr FindWindow(string c,string t);[StructLayout(LayoutKind.Sequential)]public struct R{public int l,t,r,b;}[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out R r);[DllImport("user32.dll")]public static extern bool SetProcessDPIAware();public delegate bool EP(IntPtr h,IntPtr l);[DllImport("user32.dll")]public static extern bool EnumWindows(EP f,IntPtr l);[DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);[DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);public static string Biggest(uint[] pids){string best="0,0,0,0";long area=0;EnumWindows(delegate(IntPtr h,IntPtr l){uint p;GetWindowThreadProcessId(h,out p);if(Array.IndexOf(pids,p)<0||!IsWindowVisible(h))return true;R r;GetWindowRect(h,out r);long a=(long)(r.r-r.l)*(r.b-r.t);if(a>area&&a<4000000){area=a;best=r.l+","+r.t+","+r.r+","+r.b;}return true;},IntPtr.Zero);return best;}}'`);
  const move = (x, y) => run(`[M]::SetCursorPos(${Math.round(x)},${Math.round(y)})`);
  const glide = async (x, y) => { for (let k = 6; k >= 0; k--) { await move(x + k * 9, y + k * 14); await sleep(25); } };
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
  page.on("pageerror", (e) => fail("erro na página: " + e.message));
  page.on("console", (m) => { if (["error", "warning"].includes(m.type())) log("CONSOLE", m.type(), m.text().slice(0, 300)); });
  await sleep(1500);
  await run(`[void][M]::SetProcessDPIAware()`);
  const title = await page.title();
  const wr = (await run(`$ids=[uint32[]]@(Get-Process electron | ForEach-Object { $_.Id }); Write-Output ("WR=" + [M]::Biggest($ids))`)).match(/WR=(-?\d+),(-?\d+),(-?\d+),(-?\d+)/).slice(1).map(Number);
  const win = await page.evaluate(() => ({ x: window.screenX, y: window.screenY, w: innerWidth, h: innerHeight, dpr: devicePixelRatio, sw: screen.width }));
  log("janela", JSON.stringify(win), "retângulo físico:", wr.join(","), "foreground:", await fg());
  const kx = (wr[2] - wr[0]) / win.w, ky = (wr[3] - wr[1]) / win.h;
  win.x = wr[0]; win.y = wr[1];
  const S = (r) => ({ x: wr[0] + (r.x + r.w / 2) * kx, y: wr[1] + (r.y + r.h / 2) * ky });
  const rectOf = (sel) => page.evaluate((sel) => {
    const pick = (s) => { if (s.title) return [...document.querySelectorAll("button")].find((b) => b.title === s.title); if (s.text) return [...document.querySelectorAll("button,div,span")].find((b) => b.textContent.trim() === s.text && b.offsetParent); if (s.idx != null) return document.querySelectorAll(s.nav)[s.idx]; };
    const el = pick(sel); if (!el) return null; const r = el.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height };
  }, sel);
  const state = () => page.evaluate(() => {
    const t = document.body.innerText;
    return { sheet: /Configurações|Arraste para reordenar/.test(t), panel: !!([...document.querySelectorAll("button")].find((b) => b.title === "Ajustes e Pages")), tab: (() => { const g = [...document.querySelectorAll("button")].find((b) => b.title === "Ajustes e Pages"); if (!g) return ""; const nav = [...g.parentElement.parentElement.querySelectorAll("button")]; return (nav.find((b) => !b.title && b.innerText.trim().length > 2)?.innerText || "").trim(); })(), sheetTitle: (document.querySelector("h2,h3")?.innerText || "") };
  });
  const cx = wr[0] + (wr[2] - wr[0]) / 2;
  await shot("inicio");

  log("1) passa o mouse no notch");
  await glide(cx, wr[1] + 14 * ky); await sleep(700);
  log("estado", JSON.stringify(await state())); await shot("hover");
  if (!(await state()).panel) { log("  (hover não abriu; clicando no notch)"); await click(cx, wr[1] + 14 * ky); await sleep(700); log("  ", JSON.stringify(await state())); }

  log("2) clica em cada página da barra");
  const navCount = await page.evaluate(() => { const g = [...document.querySelectorAll("button")].find((b) => b.title === "Ajustes e Pages"); window.__nav = g ? [...g.parentElement.parentElement.querySelectorAll("button")] : []; return window.__nav.length; });
  log("botões na barra:", navCount);
  for (let i = 0; i < navCount; i++) {
    const r = await page.evaluate((i) => { const b = window.__nav[i]; if (!b || !b.offsetParent) return null; const q = b.getBoundingClientRect(); return { x: q.x, y: q.y, w: q.width, h: q.height, t: b.title || b.innerText.trim() }; }, i);
    if (!r || /Ajustes e Pages|Perguntar/.test(r.t)) continue;
    const before = JSON.stringify(await state()); const p = S(r); const t0 = Date.now();
    await click(p.x, p.y);
    let after = before; while (Date.now() - t0 < 1500) { after = JSON.stringify(await state()); if (after !== before) break; await sleep(30); }
    log(`  botão ${i} "${r.t}" → ${after} em ${Date.now() - t0} ms ${after === before ? "(SEM MUDANÇA)" : ""}`);
    if (after === before && i > 0) fail(`botão ${i} da barra não respondeu`);
    await sleep(150);
  }
  await shot("paginas");

  log("3) abre Ajustes pela engrenagem");
  let r = await rectOf({ title: "Ajustes e Pages" });
  if (!r) { await move(cx, wr[1] + 14 * ky); await sleep(700); r = await rectOf({ title: "Ajustes e Pages" }); }
  log("engrenagem", JSON.stringify(r));
  if (r) { const p = S(r); await click(p.x, p.y); }
  for (const ms of [150, 400, 900, 1600]) { await sleep(ms === 150 ? 150 : ms - 150); log(`  +${ms} ms`, JSON.stringify(await state()), "fg:", await fg()); }
  if (!(await state()).sheet) fail("a engrenagem não abriu os Ajustes");
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
  await move(cx, wr[1] + 300 * ky); await run(`[M]::mouse_event(0x800,0,0,-360,0)`); await sleep(500); await shot("rolou"); log("  depois de rolar", JSON.stringify(await state()));

  log("4) clique fora (no Bloco de Notas) e volta");
  await click(cx + 500, 700); await sleep(800); log("  ", JSON.stringify(await state()), "fg:", await fg());
  await glide(cx, wr[1] + 14 * ky); await sleep(800); log("  hover de novo", JSON.stringify(await state())); await shot("hover2");

  log("5) cliques rápidos alternando páginas (40 cliques)");
  const ids = await page.evaluate(() => { const g = [...document.querySelectorAll("button")].find((b) => b.title === "Ajustes e Pages"); window.__nav = g ? [...g.parentElement.parentElement.querySelectorAll("button")].filter((b) => b.offsetParent && !b.title) : []; return window.__nav.length; });
  const rects = await page.evaluate(() => window.__nav.map((b) => { const q = b.getBoundingClientRect(); return { x: q.x, y: q.y, w: q.width, h: q.height }; }));
  const t0 = Date.now();
  for (let k = 0; k < 40 && rects.length; k++) { const p = S(rects[k % rects.length]); await click(p.x, p.y); await sleep(70); }
  log(`  40 cliques em ${Date.now() - t0} ms`, JSON.stringify(await state()));
  await sleep(800); await shot("rapidos"); log("  depois", JSON.stringify(await state()));
  // ainda responde?
  r = await rectOf({ title: "Ajustes e Pages" });
  if (r) { const p = S(r); await click(p.x, p.y); await sleep(800); const st = await state(); log("  engrenagem depois dos cliques rápidos", JSON.stringify(st)); await shot("ajustes-depois"); if (!st.sheet) fail("depois dos cliques rápidos a engrenagem não respondeu"); }
  else fail("depois dos cliques rápidos o painel não estava aberto");

  log("6) abre Ajustes pelo atalho do menu da bandeja não dá; testa Ctrl+Alt+Espaço (Perguntar)");
  await click(cx + 500, 700); await sleep(500);
  await run(`[System.Windows.Forms.SendKeys]::SendWait("^%( )")`); await sleep(900); log("  ", JSON.stringify(await state()), "foco:", await page.evaluate(() => document.activeElement?.tagName), "fg:", await fg()); await shot("perguntar");
  log("7) gera um relatório de problemas (deve trazer cliques.json e nenhuma chave)");
  log("  ", JSON.stringify(await page.evaluate(() => window.kon.report({ note: "sonda", withShot: false }).then((r) => ({ ok: r?.ok, err: r?.error })))));
  await sleep(1500);
  try { const os = require("os"); const d = fs.readdirSync(os.tmpdir()).filter((n) => n.startsWith("kon-relatorio-")).sort().pop(); for (const f of fs.readdirSync(path.join(os.tmpdir(), d))) fs.copyFileSync(path.join(os.tmpdir(), d, f), path.join(OUT, "rel-" + f)); log("  relatório copiado", d); } catch (e) { log("  sem relatório", e.message); }
  log(failures ? `FIM com ${failures} falha(s)` : "FIM sem falhas");
  await browser.close().catch(() => {});
  ps.stdin.end(); process.exit(failures ? 1 : 0);
})().catch((e) => { log("ERRO", e.stack); process.exit(1); });
