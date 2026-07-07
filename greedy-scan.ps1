$watch = "C:\Users\ACER\OneDrive\ai-screenshots"
$log = "C:\Users\ACER\OneDrive\Obsidian Vault\system\auto-ocr.md"
$processed = Get-Content $log -ErrorAction SilentlyContinue | Select-String '\| ([^|]+?) \|' | ForEach-Object { $_.Matches.Groups[1].Value.Trim() }
$total = 0; $errors = 0

Get-ChildItem $watch -Filter *.jpg | Sort-Object LastWriteTime | ForEach-Object {
  if ($processed -contains $_.Name) { return }
  $wsl = "/mnt/c/Users/ACER/OneDrive/ai-screenshots/$($_.Name)" -replace '\\','/'
  $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
  $total++
  Write-Host "[$ts] ($total) $($_.Name)..." -NoNewline

  # OCR via wsl timeout (10s kill)
  $ocr = wsl timeout 10 tesseract $wsl stdout -l eng 2>&1
  if ($LASTEXITCODE -eq 124) { Write-Host " OCR:HALT" -NoNewline; $ocr=""; $errors++ }
  else { $ocr = ($ocr -join ' ') -replace '\s+',' '; if ($ocr.Length -gt 200) { $ocr=$ocr.Substring(0,200) }; Write-Host " OCR:$($ocr.Length)" -NoNewline }

  # Moondream via wsl timeout (10s kill)
  $moon = wsl timeout 10 bash -l -c "ollama run moondream 'describe $wsl' 2>/dev/null" 2>&1
  if ($LASTEXITCODE -eq 124) { Write-Host " Moon:HALT" -NoNewline; $moon=""; $errors++ }
  else { $moon = $moon -join ' ' -replace '\x1b\[[0-9;]*[a-zA-Z]','' -replace '\s+',' '; if ($moon.Length -gt 200) { $moon=$moon.Substring(0,200) }; Write-Host " Moon:$($moon.Length)" -NoNewline }

  Add-Content $log "| $ts | $($_.Name) | $($ocr -replace '\|','/') | $($moon -replace '\|','/') |"
  Write-Host ""
}
Write-Host "=== GREEDY DONE: $total total, $errors errors ==="
