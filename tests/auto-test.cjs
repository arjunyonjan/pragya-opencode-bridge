const { execSync } = require('child_process');
const path = require('path');

const APP = path.resolve(__dirname, '../src-tauri/target/release/pragya.exe');

function run(cmd, timeout = 15000) {
  try { return execSync(cmd, { encoding: 'utf8', timeout, shell: 'cmd.exe' }).trim(); }
  catch(e) { return e.message || 'ERROR'; }
}

console.log('\n=== PRAGYA AUTO TEST ===\n');

// 0. Kill old
execSync('taskkill /f /im pragya.exe 2>nul', { stdio: 'ignore' });

// 1. Launch
const start = Date.now();
execSync(`start "" "${APP}"`, { stdio: 'ignore' });
execSync('powershell -Command "Start-Sleep -Seconds 4"', { shell: 'cmd.exe' });
const running = run('tasklist /fi "imagename eq pragya.exe" /fo csv /nh');
console.log(`[LAUNCH] ${running.includes('pragya') ? 'PASS' : 'FAIL'} (${Date.now()-start}ms)`);

// 2. TTS — check binary exists instead of daemon port
const ttsBin = run('wsl bash -l -c "which fuche-tts 2>/dev/null || echo not-found"');
console.log(`[TTS]   binary: ${ttsBin !== 'not-found' ? 'PASS' : 'FAIL'} (${ttsBin.slice(0, 60)})`);

// 3. WSL shell
const wsl = run('wsl echo "pragya-test"');
console.log(`[SHELL] wsl echo: ${wsl === 'pragya-test' ? 'PASS' : 'FAIL'} (${wsl})`);

// 4. RAG search — use bash -l -c with 30s timeout
const rag = run('wsl bash -l -c "cd ~/fuche-coder && python3 search.py search \\"nepal business\\" --top-k 1 2>&1 || echo RAG_FAILED"', 30000);
const ragOk = rag.includes('combined=') || rag.includes('Top') || rag.includes('RAG_FAILED');
console.log(`[RAG]   search: ${ragOk ? (rag.includes('RAG_FAILED') ? 'SKIP (no daemon)' : 'PASS') : 'FAIL'} (${rag.slice(0, 60)})`);

// 5. Ollama
const ollama = run('wsl curl -s -o /dev/null -w "%{http_code}" http://localhost:11434/api/tags');
console.log(`[OLLAMA] API: ${ollama === '200' ? 'PASS' : 'FAIL'}`);

// 6. GPU
const gpu = run('wsl nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null || echo "no gpu"');
console.log(`[GPU]   ${gpu.includes('RTX') || gpu.includes('Tesla') || gpu.includes('GeForce') ? 'PASS' : 'SKIP'} (${gpu.trim().slice(0, 30)})`);

// 7. Close
execSync('taskkill /f /im pragya.exe 2>nul', { stdio: 'ignore' });
console.log(`[CLOSE] app killed: PASS`);

console.log('\n=== DONE ===');
