#!/usr/bin/env python3
"""Teste de cliques reais no Kon nativo (Linux + Xvfb): abre o painel, clica em cada botão da barra,
confere pelo log que cada página abriu, abre Ajustes e Perguntar, e falha se o app cair."""
import os, subprocess, sys, time, re
BIN = sys.argv[1] if len(sys.argv) > 1 else "target/debug/kon"
import tempfile
tmp = tempfile.mkdtemp()  # ajustes zerados a cada teste
env = dict(os.environ, DISPLAY=":9", KON_DEBUG="1", KON_DEMO="painel", KON_PAGE="home", TMPDIR=tmp)
xv = subprocess.Popen(["Xvfb", ":9", "-screen", "0", "1200x700x24"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1)
log = open("/tmp/kon-clicks.log", "w")
app = subprocess.Popen([BIN], env=env, stderr=log, stdout=subprocess.DEVNULL)
time.sleep(3)
def x(*a): subprocess.run(["xdotool", *map(str, a)], env=env, check=False)
def key(k):
    w = subprocess.run(["xdotool", "search", "--name", "Kon"], env=env, capture_output=True, text=True).stdout.split()
    if w: x("windowfocus", w[0]); time.sleep(0.1)
    x("key", k)
def click(px, py): x("mousemove", px, py); time.sleep(0.15); x("click", 1); time.sleep(0.45)
def shot(name): subprocess.run(["import", "-window", "root", "-crop", "660x480+270+0", f"/tmp/clk_{name}.png"], env=env)
fails = []
# barra: y = 278 - 26; percorre da esquerda para a direita
seen = []
for px in range(330, 925, 7):
    if app.poll() is not None: fails.append(f"o app fechou ao clicar em x={px}"); break
    before = open("/tmp/kon-clicks.log").read()
    click(px, 252)
    after = open("/tmp/kon-clicks.log").read()[len(before):]
    for m in re.findall(r"view=(\w+)/(\w+)", after):
        if m not in seen: seen.append(m)
    if "Sheet" in after:  # engrenagem: abriu Ajustes; fecha com Esc e segue
        shot("ajustes"); key("Escape"); time.sleep(0.4)
    if "Ask" in after:
        shot("perguntar"); key("Escape"); time.sleep(0.4); click(600, 20); time.sleep(0.6)
pages = ["home"] + [p for v, p in seen if v == "Panel"]
want = ["home", "files", "clip", "music", "weather", "focus", "todo", "claude", "quick", "system", "water", "sleep", "lock"]
for w in want:
    if w not in pages: fails.append(f"a página {w} não abriu pelo clique")
if not any(v == "Sheet" for v, _ in seen): fails.append("a engrenagem não abriu Ajustes")
if not any(v == "Ask" for v, _ in seen): fails.append("a busca não abriu Perguntar")
alive = app.poll() is None
app.terminate(); xv.terminate()
if not alive: fails.append("o app não está mais rodando")
print("visto:", seen)
if fails: print("FALHOU:\n- " + "\n- ".join(fails)); sys.exit(1)
print("OK: todas as páginas, Ajustes e Perguntar abriram com cliques reais")
