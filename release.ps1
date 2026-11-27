# Baut ein Release-Paket nach dist\ (Windows).
# Die Pfade des eigenen Rechners (Benutzername, Projektordner) werden aus den
# exe-Dateien entfernt - Rust bettet sie sonst in Fehlermeldungen ein.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$version = (Select-String -Path syncie-gui\Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$env:RUSTFLAGS = "--remap-path-prefix=$env:USERPROFILE\.cargo=cargo --remap-path-prefix=$env:USERPROFILE\.rustup=rustup --remap-path-prefix=$PSScriptRoot=syncie"
cargo build --release --target-dir target\public
if ($LASTEXITCODE -ne 0) { exit 1 }

$name = "Syncie-$version"
$ordner = "dist\$name"
Remove-Item -Recurse -Force $ordner, "dist\$name-windows.zip" -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $ordner | Out-Null
Copy-Item target\public\release\syncie-gui.exe, target\public\release\syncie-cli.exe, README.md, LICENSE, PRIVACY.md $ordner
Compress-Archive -Path "$ordner\*" -DestinationPath "dist\$name-windows.zip"

$treffer = Select-String -Path "$ordner\*.exe" -Pattern ([regex]::Escape($env:USERNAME)) -SimpleMatch -Quiet
if ($treffer) { Write-Warning "Benutzername steht noch in einer exe" } else { "OK: dist\$name-windows.zip" }
