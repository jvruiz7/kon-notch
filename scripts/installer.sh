#!/bin/sh
# Gera release/Kon-Setup-<versão>.exe + latest.yml (atualização automática) sem Wine:
# empacota com electron-builder --dir e compila o instalador com o NSIS nativo.
set -e
cd "$(dirname "$0")/.."
V=$(node -p "require('./package.json').version")
OUT="release/Kon-Setup-$V.exe"
rm -rf release/win-unpacked
npx vite build
npx electron-builder --win dir --x64 --publish never
ARGS="-V2 -DVERSION=$V -DSRC=$PWD/release/win-unpacked -DOUT=$PWD/$OUT build/installer.nsi"
if command -v makensis >/dev/null 2>&1; then
  makensis $ARGS                                   # NSIS do sistema (ex.: apt install nsis)
else
  N=$(ls -d ~/.cache/electron-builder/nsis-3.*/nsis-3.* | grep -v '\.7z$' | head -1)
  chmod +x "$N/linux/makensis"
  NSISDIR="$N" "$N/linux/makensis" $ARGS           # NSIS que vem com o electron-builder
fi
node -e '
const fs=require("fs"),c=require("crypto");const [v,f]=process.argv.slice(1);
const b=fs.readFileSync(f);const sha=c.createHash("sha512").update(b).digest("base64");const name=f.split("/").pop();
fs.writeFileSync("release/latest.yml",`version: ${v}\nfiles:\n  - url: ${name}\n    sha512: ${sha}\n    size: ${b.length}\npath: ${name}\nsha512: ${sha}\nreleaseDate: '"'"'${new Date().toISOString()}'"'"'\n`);
' "$V" "$OUT"
# pacote pequeno (só o app.asar compactado) para atualizar sem baixar o instalador inteiro
node -e '
const fs=require("fs"),c=require("crypto"),z=require("zlib");const v=process.argv[1];
const asar=fs.readFileSync("release/win-unpacked/resources/app.asar");
const gz=z.gzipSync(asar,{level:9});const file=`app-${v}.asar.gz`;
fs.writeFileSync(`release/${file}`,gz);
const electron=require("./node_modules/electron/package.json").version;
fs.writeFileSync("release/kon-patch.json",JSON.stringify({version:v,electron,file,size:gz.length,sha512:c.createHash("sha512").update(asar).digest("base64")},null,2));
' "$V"
ls -la "$OUT" release/latest.yml release/kon-patch.json "release/app-$V.asar.gz"
