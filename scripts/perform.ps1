param(
    [Parameter(Mandatory=$true)][string]$ScriptPath,
    [string]$OutputPath,
    [switch]$Validate
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$portableRuby = Join-Path $projectRoot '.tools\rubyinstaller-3.4.11-1-x64\bin\ruby.exe'
$ruby = if (Test-Path -LiteralPath $portableRuby) { $portableRuby } else { 'ruby' }
$portableFfmpeg = Join-Path $projectRoot '.tools\ffmpeg-master-latest-win64-gpl\bin\ffmpeg.exe'
if (Test-Path -LiteralPath $portableFfmpeg) { $env:FFMPEG = $portableFfmpeg }
$arguments = @('bin/agent_perform', '--script', $ScriptPath)
if ($Validate) {
    $arguments += '--validate'
} else {
    if ([string]::IsNullOrWhiteSpace($OutputPath)) { throw 'Provide -OutputPath or -Validate.' }
    $arguments += @('--output', $OutputPath)
}
& $ruby @arguments
exit $LASTEXITCODE
