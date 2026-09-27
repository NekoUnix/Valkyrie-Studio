param([Parameter(Mandatory=$true)][string]$Label)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$stage = Join-Path $root "target\package\valkyrie-studio-$Label"
$licenses = Join-Path $stage 'licenses'
$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $licenses, $dist | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'target\release\valkyrie-studio.exe') -Destination $stage
Copy-Item -LiteralPath (Join-Path $root 'target\release\valkyrie-agent.exe') -Destination $stage
Copy-Item -LiteralPath (Join-Path $root 'target\release\valkyrie-perform.exe') -Destination $stage
Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination $stage
Copy-Item -LiteralPath (Join-Path $root 'third_party\purism_core\LICENSE') -Destination (Join-Path $licenses 'PurismCore-LICENSE.txt')
Copy-Item -LiteralPath (Join-Path $root 'README.md') -Destination $stage
Copy-Item -LiteralPath (Join-Path $root 'THIRD_PARTY.md') -Destination $stage
Copy-Item -LiteralPath (Join-Path $root 'docs') -Destination $stage -Recurse
Copy-Item -LiteralPath (Join-Path $root 'examples') -Destination $stage -Recurse
Copy-Item -LiteralPath (Join-Path $root 'media') -Destination $stage -Recurse
Copy-Item -LiteralPath (Join-Path $root 'helpers') -Destination $stage -Recurse
New-Item -ItemType Directory -Force (Join-Path $stage 'scripts') | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'scripts\webcam.ps1') -Destination (Join-Path $stage 'scripts')
$archive = Join-Path $dist "valkyrie-studio-$Label.zip"
if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive }
Compress-Archive -LiteralPath $stage -DestinationPath $archive
