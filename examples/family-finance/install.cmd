:<<"::FIM-DO-CMD"
@echo off
rem  Raven Financas - instalar compilando nesta maquina.
rem
rem  Um arquivo so para os tres sistemas:
rem    Windows ........ baixe e de dois cliques neste arquivo
rem    Linux e macOS .. baixe e rode: sh install.cmd
rem
rem  Esta parte roda o PowerShell que esta mais abaixo e espera uma tecla no fim,
rem  para a janela nao sumir com a mensagem.
title Instalando o Raven Financas
setlocal
set "FF_SCRIPT=%~f0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; try { $t=[IO.File]::ReadAllText($env:FF_SCRIPT); $n=[char]10; $i=$t.IndexOf($n+'#==POWERSHELL=='); if ($i -lt 0) { throw 'instalador invalido' }; $f=$t.IndexOf($n+'#==FIM-POWERSHELL==',$i); if ($f -le $i) { throw 'instalador incompleto' }; Invoke-Expression $t.Substring($i, $f-$i) } catch { Write-Host ''; Write-Host ('X ' + $_) -ForegroundColor Red; exit 1 }"
set "FF_RESULTADO=%ERRORLEVEL%"
echo.
if not "%FF_SEM_PAUSA%"=="1" pause
exit /b %FF_RESULTADO%
::FIM-DO-CMD
# Raven Finanças: the installer and the updater are this one file. The app runs it to update itself.
#
# Each interpreter reads only its part. `cmd` runs the block above and stops at
# `exit /b`. `sh` passes that block and the PowerShell one as two heredocs given
# to `:`, which does nothing. PowerShell gets only what is between the `#==` marks.
# Line endings must stay LF: `sh` breaks on CRLF.
#
# What works is not touched until the new binary proves it opens:
# compile in its own folder, run `--versao`, keep the old one as `.anterior`, move the new one in.
#
# Options, as environment variables:
#   FF_FONTE     a .tar.gz with the code (default: the master branch on GitHub)
#   FF_CASA      where the code and the build go (default: ~/.raven-financas)
#   FF_DESTINO   where the app goes
#   FF_TARGET    the Cargo build folder (default: inside FF_CASA)
: <<'#==FIM-POWERSHELL=='
#==POWERSHELL==
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$Fonte = if ($env:FF_FONTE) { $env:FF_FONTE } else { "https://github.com/alexkads/RavenRS/archive/refs/heads/master.tar.gz" }
$Casa = if ($env:FF_CASA) { $env:FF_CASA } else { "$env:USERPROFILE\.raven-financas" }
$Destino = if ($env:FF_DESTINO) { $env:FF_DESTINO } else { "$env:LOCALAPPDATA\Programs\RavenFinancas" }
$Alvo = if ($env:FF_TARGET) { $env:FF_TARGET } else { "$Casa\target" }

function Correr([scriptblock]$bloco) {
    $antes = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try { $global:LASTEXITCODE = 0; & $bloco | Out-Host } finally { $ErrorActionPreference = $antes }
    if ($LASTEXITCODE -ne 0) { throw "comando falhou (codigo $LASTEXITCODE): $bloco" }
}

New-Item -ItemType Directory -Force -Path $Casa | Out-Null

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "> instalando o Rust" -ForegroundColor Cyan
    $rustup = Join-Path $Casa "rustup-init.exe"
    Invoke-WebRequest -UseBasicParsing "https://win.rustup.rs/x86_64" -OutFile $rustup
    Correr { & $rustup -y --profile minimal }
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
}

Write-Host "> baixando o codigo" -ForegroundColor Cyan
$pacote = Join-Path $Casa "fonte.tar.gz"
$codigo = Join-Path $Casa "fonte"
if (Test-Path $Fonte) { Copy-Item $Fonte $pacote -Force } else { Invoke-WebRequest -UseBasicParsing $Fonte -OutFile $pacote }
if (Test-Path $codigo) { Remove-Item -Recurse -Force $codigo }
New-Item -ItemType Directory -Force -Path $codigo | Out-Null
Correr { tar -xzf $pacote -C $codigo --strip-components=1 }

Write-Host "> compilando (a primeira vez demora; as seguintes aproveitam o que ja existe)" -ForegroundColor Cyan
$env:CARGO_TARGET_DIR = $Alvo
$env:FAMILY_FINANCE_INSTALLED = "1"
Push-Location $codigo
try { Correr { cargo build --release -p family-finance } } finally { Pop-Location }

$novo = Join-Path $Alvo "release\family-finance.exe"
$resposta = & $novo --versao
if ($LASTEXITCODE -ne 0 -or -not $resposta) { throw "o app novo nao abriu (--versao); o instalado continua como estava" }

New-Item -ItemType Directory -Force -Path $Destino | Out-Null
$instalado = Join-Path $Destino "family-finance.exe"
$anterior = "$instalado.anterior"
if (Test-Path $anterior) { Remove-Item -Force $anterior }
if (Test-Path $instalado) { Move-Item $instalado $anterior }
try { Copy-Item $novo $instalado } catch {
    if (Test-Path $anterior) { Move-Item $anterior $instalado }
    throw "nao consegui colocar o app novo; o anterior voltou"
}

$atalho = Join-Path ([Environment]::GetFolderPath("Programs")) "Raven Financas.lnk"
$s = (New-Object -ComObject WScript.Shell).CreateShortcut($atalho)
$s.TargetPath = $instalado
$s.Save()
Write-Host "OK Raven Financas $resposta instalado. Abra pelo menu Iniciar." -ForegroundColor Green
#==FIM-POWERSHELL==
{
set -eu

FONTE="${FF_FONTE:-https://github.com/alexkads/RavenRS/archive/refs/heads/master.tar.gz}"
CASA="${FF_CASA:-$HOME/.raven-financas}"
NOME="Raven Finanças"

diga() { printf '\n> %s\n' "$1"; }
erro() { printf 'X %s\n' "$1" >&2; }

mkdir -p "$CASA"

if ! command -v cargo >/dev/null 2>&1 && [ -x "$HOME/.cargo/bin/cargo" ]; then
  PATH="$HOME/.cargo/bin:$PATH"
fi
if ! command -v cc >/dev/null 2>&1; then
  if [ "$(uname -s)" = Darwin ]; then
    erro "falta o compilador C, que o SQLite usa. Rode: xcode-select --install"
  else
    erro "falta o compilador C, que o SQLite usa. Instale o gcc pelo gerenciador de pacotes do sistema."
  fi
  exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
  diga "instalando o Rust"
  # A file, never `curl | sh`: a failed download would hand `sh` an empty script, which succeeds.
  curl -fsSL https://sh.rustup.rs -o "$CASA/rustup-init.sh"
  sh "$CASA/rustup-init.sh" -y --profile minimal
  PATH="$HOME/.cargo/bin:$PATH"
fi

diga "baixando o código"
PACOTE="$CASA/fonte.tar.gz"
CODIGO="$CASA/fonte"
case "$FONTE" in
  http*) curl -fsSL "$FONTE" -o "$PACOTE" ;;
  *) cp "$FONTE" "$PACOTE" ;;
esac
rm -rf "${CODIGO:?}"
mkdir -p "$CODIGO"
tar -xzf "$PACOTE" -C "$CODIGO" --strip-components=1

diga "compilando (a primeira vez demora; as seguintes aproveitam o que já existe)"
CARGO_TARGET_DIR="${FF_TARGET:-$CASA/target}"
export CARGO_TARGET_DIR
( cd "$CODIGO" && FAMILY_FINANCE_INSTALLED=1 cargo build --release -p family-finance )

BINARIO="$CARGO_TARGET_DIR/release/family-finance"
RESPOSTA="$("$BINARIO" --versao 2>/dev/null | head -1 || true)"
if [ -z "$RESPOSTA" ]; then
  erro "o app novo não abriu (--versao); o instalado continua como estava"
  exit 1
fi

if [ "$(uname -s)" = Darwin ]; then
  DESTINO="${FF_DESTINO:-$HOME/Applications}"
  mkdir -p "$DESTINO"
  NOVO="$DESTINO/.$NOME.app.novo"
  rm -rf "${NOVO:?}"
  mkdir -p "$NOVO/Contents/MacOS"
  cp "$BINARIO" "$NOVO/Contents/MacOS/family-finance"
  cat > "$NOVO/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key><string>$NOME</string>
	<key>CFBundleDisplayName</key><string>$NOME</string>
	<key>CFBundleIdentifier</key><string>dev.ravenrs.family-finance</string>
	<key>CFBundleExecutable</key><string>family-finance</string>
	<key>CFBundleShortVersionString</key><string>$RESPOSTA</string>
	<key>CFBundleVersion</key><string>$RESPOSTA</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>LSApplicationCategoryType</key><string>public.app-category.finance</string>
	<key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
  rm -rf "${DESTINO:?}/${NOME:?}.app.anterior"
  if [ -d "$DESTINO/$NOME.app" ]; then
    mv "$DESTINO/$NOME.app" "$DESTINO/$NOME.app.anterior" \
      || { rm -rf "${NOVO:?}"; erro "não consegui afastar o app instalado; ele continua como estava"; exit 1; }
  fi
  if ! mv "$NOVO" "$DESTINO/$NOME.app"; then
    [ -d "$DESTINO/$NOME.app.anterior" ] && mv "$DESTINO/$NOME.app.anterior" "$DESTINO/$NOME.app"
    erro "não consegui colocar o app novo; o anterior voltou"
    exit 1
  fi
  printf '\nOK %s %s instalado em %s.\n' "$NOME" "$RESPOSTA" "$DESTINO"
else
  DESTINO="${FF_DESTINO:-$HOME/.local}"
  mkdir -p "$DESTINO/bin" "$DESTINO/share/applications"
  INSTALADO="$DESTINO/bin/family-finance"
  [ -f "$INSTALADO" ] && cp -p "$INSTALADO" "$INSTALADO.anterior"
  # `mv` is atomic on the same disk, so the installed binary is never half written.
  cp "$BINARIO" "$INSTALADO.novo"
  mv "$INSTALADO.novo" "$INSTALADO"
  cat > "$DESTINO/share/applications/family-finance.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=$NOME
Exec=$INSTALADO
Categories=Office;Finance;
DESKTOP
  printf '\nOK %s %s instalado em %s.\n' "$NOME" "$RESPOSTA" "$INSTALADO"
fi
# Fim do instalador. Apagar a pasta de compilação é seguro: ela volta na próxima atualização.
}
