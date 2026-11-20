# Kon — Dynamic Island / Notch para Windows

Notch preto OLED colado no topo da tela, no estilo Nusa 1.3 / Apple Intelligence, feito com Electron, React,
TypeScript, Tailwind e Framer Motion (mola `stiffness 380 / damping 30`).

## Recursos

| Recurso | Como funciona | Rede |
|---|---|---|
| **Ask** (`Ctrl+Alt+Space`) | Chat com **Claude** (`@anthropic-ai/sdk`, padrão `claude-haiku-5-5`: rápido e barato) ou **Gemini** (`@google/genai` + Google Search com fontes). O botão na barra alterna entre as duas IAs. Se o modelo Gemini escolhido tiver sido desativado, o Kon usa o `gemini-flash-latest` automaticamente | sua chave |
| **Ditado em pt-BR** | Usa `webkitSpeechRecognition`. Se o Electron não liberar, grava o áudio e transcreve com o Gemini | sua chave |
| **Matemática** (`1.500.000,50 × 12% =`) | 100% local, 0 ms; clique no resultado para copiar | — |
| **Câmbio** (`100usd em brl=`) | Resposta instantânea do cache (RAM + disco). Atualiza em segundo plano a cada 45 min e mostra o horário da cotação; sem internet, usa a última salva | ExchangeRate-API |
| **Tradutor** (`Tab`) | Gemini ou Claude quando há chave; dicionário local quando não há | opcional |
| **Smart OCR** (`Ctrl+Alt+T`) | Tela cheia transparente para recortar → `Windows.Media.Ocr` **offline** (upscale 2x, pt-BR) → área de transferência → texto completo na ilha | — |
| **Arquivos** (File Stash) | Arraste arquivos/pastas até a ilha: só a referência vai para `kon-stash.json`. Dá para abrir, mostrar na pasta, arrastar de volta e remover. Nada é lido nem enviado para IA | — |
| **Music** | Player ativo do Windows (GSMTC): título, artista, capa, play/pause, próxima, anterior, seek + letra sincronizada. A capa é relida por ~12 s após a troca de faixa; se o app não entregar, vem do catálogo do iTunes | LRCLIB, iTunes |
| **Clima** | Página própria: temperatura, sensação, umidade, vento, chance de chuva, próximas 6 horas e arco do sol (nascer → pôr). Avisa no notch quando há chuva provável nas próximas horas. Localização atual (IP) ou capitais: Porto Velho, São Paulo, Brasília, Rio, Manaus, Curitiba | Open-Meteo |
| **Volume** | Aviso de volume ao mudar o volume do Windows | — |
| **Claude Code: ao vivo** | Tarefa, arquivo, diff, etapas e sessões abertas via hooks HTTP | local |
| **Claude Code: aprovar pelo notch** | Quando o Claude pede permissão, o notch mostra um card com **Negar / Permitir** (+ "Sempre permitir" e "No terminal"). Sem resposta em ~100 s, a pergunta volta para o terminal | local |
| **Claude Code: uso** | Página **Uso**: janela de 5 h (estimativa), tokens e custo aproximado de hoje e da semana, gráfico de 7 dias, divisão por modelo, sessões e tokens do chat do Kon. Lido dos transcripts em `~/.claude/projects`; nada sai do PC. Acima de 80% aparece uma barra no notch fechado | — |
| **Área de transferência** (`Ctrl+Alt+V`) | Histórico de textos copiados (40 itens, fixar, apagar, pausar). Ignora o que parece senha, chave de API ou cartão | — |
| **Zonas de soltar** | Ao arrastar um arquivo: **Guardar** (só referência), **Perguntar à IA** (envia o arquivo, só quando você solta ali) ou **Compactar** (.zip ao lado) | opcional |
| **Agenda** | Cole o link .ics (Google, Outlook, iCloud). Eventos de hoje no To-do, aviso 2 min antes e botão **Entrar** com o link do Meet/Zoom/Teams | sua agenda |
| **Privacidade** | Pontinho laranja (microfone) e verde (câmera) no notch, com o nome do app no painel | — |
| **Discreto** | Some em jogos, vídeos e slides em tela cheia; vira uma linha fina sobre janelas maximizadas; fica fora de prints e de compartilhamento de tela | — |
| **To-do + calendário, Focus, Water** | Salvos no computador | — |
| **Notificações** | O próprio notch mostra avisos com o mascote: arquivo guardado, OCR copiado, chuva chegando, fim do foco, Claude terminou | — |
| **Mascote tofu** | Vive dentro da ilha. Estados: repouso, trabalhando (selo azul), precisa de você (selo laranja + brilho âmbar), concluído (^ ^, selo verde + brilho verde), erro (olhos tontos + brilho roxo), música, foco, dormindo, ouvindo e "engolindo arquivo" ao arrastar | — |

## Instalar

Baixe o **Kon-Setup-x.y.z.exe** em [Releases](https://github.com/jvruiz7/kon-notch/releases/latest) e abra.
A instalação é só para o seu usuário (não pede administrador) e cria atalho no Menu Iniciar.

> O instalador ainda não tem assinatura digital, então o Windows SmartScreen pode avisar
> "O Windows protegeu o computador". Clique em **Mais informações → Executar assim mesmo**.

**Atualização automática:** o Kon procura versões novas aqui no GitHub ao abrir e a cada 4 horas,
baixa em segundo plano e mostra um card **Reiniciar** no notch. Dá para desligar em Ajustes.

## Desenvolver

Requisitos: Node.js 20 ou superior.

```bash
npm install
npm run dev        # desenvolvimento (Vite + Electron)
npm run installer  # release/Kon-Setup-<versão>.exe + latest.yml (Linux ou WSL, sem Wine)
```

**Publicar uma versão:** aumente `version` no `package.json` e envie para o `main`. O GitHub Actions
compila o instalador e cria a Release `v<versão>` com `Kon-Setup-<versão>.exe` e `latest.yml`;
os apps instalados se atualizam sozinhos.

Primeira vez: o Kon abre as **boas-vindas em 3 passos** (IA, Claude Code e o que mostrar). Depois, tudo fica em **engrenagem** → **Ajustes** → cole `GEMINI_API_KEY`
(https://aistudio.google.com/apikey) e/ou `ANTHROPIC_API_KEY` (https://console.anthropic.com/settings/keys).
As chaves são criptografadas com `safeStorage` (DPAPI do Windows) e nunca voltam para a interface.

**OCR em português:** o Windows precisa do pacote de idioma *Português (Brasil)*
(Configurações → Hora e idioma → Idioma). Sem ele, o Kon usa os idiomas do seu perfil.

## Arquivos principais

```
electron/main.js            janela overlay 820x560, click-through por hit-test do cursor, IA dupla,
                            safeStorage, stash, OCR (overlay fullscreen), hooks do Claude Code, cache
electron/preload.js         ponte segura window.kon
electron/kon-helper.ps1     mídia (GSMTC), volume (Core Audio) e Windows.Media.Ocr
src/App.tsx                 a ilha inteira
src/components/KonMascot.tsx  mascote tofu (KonTofu + KonTofuEating)
src/OcrOverlay.tsx          recorte em tela cheia
src/services.js             câmbio, localização, clima, letras e capas (stale-while-revalidate)
src/data.js, src/ui.jsx     constantes, calculadora, tradutor local e componentes visuais
```

O click-through não depende de `onMouseEnter`: o processo principal consulta o cursor do sistema a cada 40 ms e
só aceita o mouse sobre a ilha. Assim, arrastar um arquivo do Explorer até o notch funciona, o que não daria
certo com `setIgnoreMouseEvents` sozinho.

## Créditos dos dados

Câmbio: <a href="https://www.exchangerate-api.com">Rates By Exchange Rate API</a> · Clima: [Open-Meteo](https://open-meteo.com) ·
Letras: [LRCLIB](https://lrclib.net) · Capas: [iTunes Search API](https://performance-partners.apple.com/search-api) · Localização: [GeoJS](https://www.geojs.io)
