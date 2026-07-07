const { execSync } = require('child_process');
const path = require('path');

const APP = path.resolve(__dirname, '../src-tauri/target/release/pragya.exe');

function run(cmd) {
  try { return execSync(cmd, { encoding: 'utf8', timeout: 15000 }).trim(); }
  catch(e) { return e.message || 'ERROR'; }
}

console.log('\n=== PRAGYA AUTO TEST ===\n');

// 0. Kill old
execSync('taskkill /f /im pragya.exe 2>nul', { stdio: 'ignore' });

// 1. Launch
const start = Date.now();
execSync(`start "" "${APP}"`, { stdio: 'ignore' });
require('child_process').execSync('powershell -Command "Start-Sleep -Seconds 4"', { shell: 'cmd.exe' });
const running = run('tasklist /fi "imagename eq pragya.exe" /fo csv /nh');
console.log(`[LAUNCH] ${running.includes('pragya') ? 'PASS' : 'FAIL'} (${Date.now()-start}ms)`);

// 2. Health — TTS daemon
const tts = run('curl -s -o /dev/null -w "%{http_code}" http://localhost:8750 2>nul || echo 000');
console.log(`[TTS]   daemon port 8750: ${tts === '200' ? 'PASS' : 'FAIL'} (${tts})`);

// 3. WSL shell
const wsl = run('wsl echo "pragya-test"');
console.log(`[SHELL] wsl echo: ${wsl === 'pragya-test' ? 'PASS' : 'FAIL'} (${wsl})`);

// 4. RAG search
const rag = run('wsl bash -l -c "cd ~/fuche-coder && source venv/bin/activate && python3 search.py search \\"nepal business\\" --top-k 1 2>&1"');
const ragOk = rag.includes('combined=') || rag.includes('Top');
console.log(`[RAG]   search: ${ragOk ? 'PASS' : 'FAIL'} (${rag.slice(0, 60)})`);

// 5. Ollama
const ollama = run('wsl curl -s -o /dev/null -w "%{http_code}" http://localhost:11434/api/tags');
console.log(`[OLLAMA] API: ${ollama === '200' ? 'PASS' : 'FAIL'}`);

// 6. GPU
const gpu = run('wsl nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null || echo "no gpu"');
console.log(`[GPU]   ${gpu.includes('RTX') ? 'PASS' : 'FAIL'} (${gpu.trim()})`);

// 7. Close
execSync('taskkill /f /im pragya.exe 2>nul', { stdio: 'ignore' });
console.log(`[CLOSE] app killed: PASS`);

console.log('\n=== DONE ===');
