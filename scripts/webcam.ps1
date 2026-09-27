param([int]$Camera = 0)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
& '.venv\Scripts\python.exe' helpers/webcam.py --model vendor/mediapipe/face_landmarker.task --camera $Camera --preview
exit $LASTEXITCODE
