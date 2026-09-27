param(
    [Parameter(Mandatory=$true)][string]$Model,
    [string]$Camera = '0',
    [ValidateSet('auto','dshow','msmf','v4l2','avfoundation')][string]$Backend = 'auto',
    [int]$Width = 640,
    [int]$Height = 480,
    [int]$Fps = 30,
    [int]$Port = 15483,
    [switch]$Preview,
    [string]$Python = 'python'
)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$arguments=@((Join-Path $root 'helpers/webcam.py'),'--model',$Model,'--camera',$Camera,
    '--backend',$Backend,'--width',"$Width",'--height',"$Height",'--fps',"$Fps",'--port',"$Port")
if ($Preview) { $arguments += '--preview' }
& $Python @arguments
exit $LASTEXITCODE
