$watch = "C:\Users\ACER\OneDrive\ai-screenshots"
$log = "C:\Users\ACER\OneDrive\Obsidian Vault\system\auto-ocr.md"
$seen = @{}

while ($true) {
  Get-ChildItem $watch -Filter *.jpg | Where-Object { !$seen[$_.Name] } | ForEach-Object {
    $seen[$_.Name] = $true
    $wsl = "/mnt/c/Users/ACER/OneDrive/ai-screenshots/$($_.Name)" -replace '\\','/'
    $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host "[$ts] Processing $($_.Name)..."

    $ocr = wsl -- tesseract $wsl stdout -l eng 2>&1 | Out-String
    $ocr = ($ocr -replace '\s+', ' ').Trim()
    if ($ocr.Length -gt 200) { $ocr = $ocr.Substring(0,200) }

    $moon = wsl bash -l -c "ollama run moondream 'describe $wsl' 2>/dev/null" 2>&1 | Out-String
    $moon = $moon -replace '\x1b\[[0-9;]*[a-zA-Z]','' -replace '\r?\n',' ' -replace '\s+',' '
    $moon = $moon.Trim()
    if ($moon.Length -gt 200) { $moon = $moon.Substring(0,200) }

    Add-Content $log "| $ts | $($_.Name) | $($ocr -replace '\|','/') | $($moon -replace '\|','/') |"
    Write-Host "  OCR:$($ocr.Length>0) Moon:$($moon.Length>0)"
  }
  Start-Sleep -Seconds 10
}
