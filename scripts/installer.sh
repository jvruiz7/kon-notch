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
N=$(ls -d ~/.cache/electron-builder/nsis-3.*/nsis-3.* | grep -v '\.7z$' | head -1)
chmod +x "$N/linux/makensis"
NSISDIR="$N" "$N/linux/makensis" -V2 -DVERSION="$V" -DSRC="$PWD/release/win-unpacked" -DOUT="$PWD/$OUT" build/installer.nsi
node -e '
const fs=require("fs"),c=require("crypto");const [v,f]=process.argv.slice(1);
const b=fs.readFileSync(f);const sha=c.createHash("sha512").update(b).digest("base64");const name=f.split("/").pop();
fs.writeFileSync("release/latest.yml",`version: ${v}\nfiles:\n  - url: ${name}\n    sha512: ${sha}\n    size: ${b.length}\npath: ${name}\nsha512: ${sha}\nreleaseDate: '"'"'${new Date().toISOString()}'"'"'\n`);
' "$V" "$OUT"
ls -la "$OUT" release/latest.yml
