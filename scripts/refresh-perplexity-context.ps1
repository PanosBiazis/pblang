# Refresh PERPLEXITY_CONTEXT.md and optionally copy to clipboard.
# Run from repo root: .\scripts\refresh-perplexity-context.ps1
# Optional: -CopyToClipboard to copy contents after refresh.

param([switch]$CopyToClipboard)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
if (-not (Test-Path (Join-Path $root "Cargo.toml"))) { $root = (Get-Location).Path }

$contextPath = Join-Path $root "PERPLEXITY_CONTEXT.md"
$content = Get-Content $contextPath -Raw
if (-not $content) { exit 1 }

# Touch the file so "last updated" is clear; content is maintained in PERPLEXITY_CONTEXT.md
(Get-Item $contextPath).LastWriteTime = Get-Date
Write-Host "Refreshed: $contextPath"

if ($CopyToClipboard) {
    Set-Clipboard -Value $content
    Write-Host "Copied to clipboard. Paste in Perplexity."
}
