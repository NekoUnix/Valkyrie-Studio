param([string]$Model, [switch]$Diagnostic)
$ErrorActionPreference='Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$portableRuby = Join-Path $projectRoot '.tools\rubyinstaller-3.4.11-1-x64\bin'
if (Test-Path -LiteralPath $portableRuby) { $env:PATH="$portableRuby;$env:PATH" }
$portableFfmpeg = Join-Path $projectRoot '.tools\ffmpeg-master-latest-win64-gpl\bin'
if (Test-Path -LiteralPath $portableFfmpeg) {
    $env:FFMPEG=Join-Path $portableFfmpeg 'ffmpeg.exe'
    $env:FFPROBE=Join-Path $portableFfmpeg 'ffprobe.exe'
}
$studioArguments=@('bin/studio')
if ($Model) { $studioArguments += @('--model',$Model) }
if ($Diagnostic) { $studioArguments += '--diagnostic' }
& ruby @studioArguments
exit $LASTEXITCODE
