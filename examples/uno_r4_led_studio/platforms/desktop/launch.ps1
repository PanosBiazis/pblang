# Windows source launcher. Does not download or build a runtime.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $Runtime
)
$ErrorActionPreference = 'Stop'
$runtimeFile = Get-Item -LiteralPath $Runtime -ErrorAction Stop
if ($runtimeFile.PSIsContainer -or $runtimeFile.Extension -ne '.exe') {
    throw 'Select a compatible native PBLang .exe file.'
}
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
if (-not (Test-Path -LiteralPath (Join-Path $repoRoot 'stdlib/graphics.sapp') -PathType Leaf) -or
    -not (Test-Path -LiteralPath (Join-Path $repoRoot 'examples/uno_r4_led_studio/view.sapp') -PathType Leaf)) {
    throw 'Keep this launcher inside the complete PBLang source package.'
}
$runtimePath = $runtimeFile.FullName
Push-Location -LiteralPath $repoRoot
try {
    & $runtimePath run 'examples/uno_r4_led_studio/platforms/desktop/main.sapp' --allow-all
    $runtimeExit = $LASTEXITCODE
} finally {
    Pop-Location
}
exit $runtimeExit
