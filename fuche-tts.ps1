#!/usr/bin/env pwsh
# fuche-tts.ps1 — Windows-side TTS bridge (in-memory, no files)

param([string]$Text)
if (-not $Text) { exit 1 }

# Use .NET Process to get raw bytes without text encoding
$arg = "bash -l -c '/home/arjun/.local/bin/fuche-tts $Text'"
$psi = [System.Diagnostics.ProcessStartInfo]@{
    FileName               = 'wsl'
    Arguments              = $arg
    RedirectStandardOutput = $true
    UseShellExecute        = $false
    CreateNoWindow         = $true
}
$p = [System.Diagnostics.Process]::Start($psi)
$ms = [System.IO.MemoryStream]::new()
$p.StandardOutput.BaseStream.CopyTo($ms)
$p.WaitForExit()
$pcm = $ms.ToArray()
$ms.Dispose()

if ($pcm.Length -lt 100) { Write-Warning "TTS failed"; exit 2; return }

# Build WAV header (16-bit mono 24kHz PCM)
$sr = 24000
$bps = 16
$ch = 1
$ds = $pcm.Length

$fmt = [byte[]]@(
    0x52,0x49,0x46,0x46, # RIFF
    0,0,0,0,             # file size placeholder
    0x57,0x41,0x56,0x45, # WAVE
    0x66,0x6d,0x74,0x20, # fmt
    16,0,0,0,            # chunk size
    1,0,                 # PCM
    $ch,0,               # channels
    0,0,0,0,             # sample rate placeholder
    0,0,0,0,             # byte rate placeholder
    0,0,                 # block align placeholder
    $bps,0,              # bits per sample
    0x64,0x61,0x74,0x61, # data
    0,0,0,0              # data size placeholder
)

# Fill placeholders
$fileSize = $ds + 36
$fmt[4] = $fileSize -band 0xFF
$fmt[5] = ($fileSize -shr 8) -band 0xFF
$fmt[6] = ($fileSize -shr 16) -band 0xFF
$fmt[7] = ($fileSize -shr 24) -band 0xFF

$fmt[24] = $sr -band 0xFF
$fmt[25] = ($sr -shr 8) -band 0xFF
$fmt[26] = ($sr -shr 16) -band 0xFF
$fmt[27] = ($sr -shr 24) -band 0xFF

$byteRate = $sr * $ch * $bps / 8
$fmt[28] = $byteRate -band 0xFF
$fmt[29] = ($byteRate -shr 8) -band 0xFF
$fmt[30] = ($byteRate -shr 16) -band 0xFF
$fmt[31] = ($byteRate -shr 24) -band 0xFF

$blockAlign = $ch * $bps / 8
$fmt[32] = $blockAlign -band 0xFF
$fmt[33] = ($blockAlign -shr 8) -band 0xFF

$fmt[40] = $ds -band 0xFF
$fmt[41] = ($ds -shr 8) -band 0xFF
$fmt[42] = ($ds -shr 16) -band 0xFF
$fmt[43] = ($ds -shr 24) -band 0xFF

# Combine header + PCM → play
$stream = [System.IO.MemoryStream]::new()
$stream.Write($fmt, 0, 44)
$stream.Write($pcm, 0, $ds)
$stream.Position = 0

$player = [System.Media.SoundPlayer]::new()
$player.Stream = $stream
$player.PlaySync()
$player.Dispose()
$stream.Dispose()
