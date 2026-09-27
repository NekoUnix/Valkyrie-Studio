param([Parameter(Mandatory=$true)][string]$OutputPath, [string]$Voice)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Speech
$speaker = New-Object System.Speech.Synthesis.SpeechSynthesizer
try {
    if ($Voice) { $speaker.SelectVoice($Voice) }
    $speaker.SetOutputToWaveFile($OutputPath)
    $speaker.Speak([Console]::In.ReadToEnd())
} finally { $speaker.Dispose() }
