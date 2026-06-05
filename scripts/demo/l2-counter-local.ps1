param(
  [string]$ApiUrl = "http://127.0.0.1:8080",
  [switch]$Reset,
  [switch]$ResetOnly
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $repo

if ($Reset) {
  cargo run -p l2-node --bin l2-db-reset -- --yes
}

if ($ResetOnly) {
  exit 0
}

$env:ENTROPIS_API_URL = $ApiUrl
npm --prefix sdk run sandbox:l2-counter
