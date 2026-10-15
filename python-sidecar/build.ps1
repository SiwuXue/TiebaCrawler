$ErrorActionPreference = 'Stop'
$sidecarRoot = Split-Path -Parent $MyInvocation.MyCommand.Path

uv sync --project $sidecarRoot
uv run --project $sidecarRoot --with pyinstaller pyinstaller `
  --noconfirm --clean --onefile --name tieba-sidecar `
  --distpath (Join-Path $sidecarRoot 'dist') `
  --workpath (Join-Path $sidecarRoot '.pyinstaller-work') `
  --specpath (Join-Path $sidecarRoot '.pyinstaller-spec') `
  --collect-all playwright --collect-all greenlet --collect-all pyee `
  --collect-all bs4 --hidden-import=playwright.async_api `
  (Join-Path $sidecarRoot 'main.py')

Write-Host "Built $(Join-Path $sidecarRoot 'dist\tieba-sidecar.exe')"
