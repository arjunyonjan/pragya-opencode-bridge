import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

let ocrCount = 0;

document.addEventListener('DOMContentLoaded', () => {
  // Close button
  document.getElementById('close-btn')?.addEventListener('click', () => {
    try { getCurrentWindow().close(); } catch(e) {}
  });

  initTTS();
  initOpencode();
  initCascade();
  initRag();
  initWsl();
  initHealth();
  initHeartbeat();
  initAutostart();
  initOcr();

  setInterval(() => {
    const now = new Date();
    document.getElementById('footer-time').textContent = now.toLocaleTimeString();
  }, 1000);

  refreshHealth(false);
});

// ── OCR panel ──

function initOcr() {
  listen('ocr-result', (e) => {
    const r = e.payload;
    ocrCount++;
    document.getElementById('ocr-count').textContent = ocrCount;
    const list = document.getElementById('ocr-list');
    const entry = document.createElement('div');
    entry.className = 'ocr-entry';
    entry.innerHTML = `<div class="ocr-file">${r.file}</div>
      <div class="ocr-text">${r.ocr_text ? r.ocr_text.slice(0, 120) : '<span class="dim">no text</span>'}</div>
      <div class="ocr-moon dim">${r.moondream ? r.moondream.slice(0, 120) : ''}</div>`;
    list.prepend(entry);
  });
}

// ── TTS panel ──

function initTTS() {
  const input = document.getElementById('tts-input');
  const btn = document.getElementById('tts-speak');
  const status = document.getElementById('tts-status');
  const backend = document.getElementById('tts-backend');
  const preset = document.getElementById('tts-preset');
  const speed = document.getElementById('tts-speed');
  const fx = document.getElementById('tts-fx');

  function speak() {
    const text = input.value.trim();
    if (!text) return;
    status.textContent = 'speaking...';
    invoke('tts_speak_with', {
      text,
      backend: backend.value,
      preset: preset.value,
      speed: parseFloat(speed.value),
      fx: fx.value,
    }).then(r => {
      status.textContent = r.success ? `done (${r.elapsed}s)` : `error: ${r.error}`;
    });
  }

  btn.addEventListener('click', speak);
  input.addEventListener('keydown', e => { if (e.key === 'Enter') speak(); });
  input.focus();
}

// ── Opencode panel ──

function initOpencode() {
  const input = document.getElementById('opencode-input');
  const btn = document.getElementById('opencode-send');
  const output = document.getElementById('opencode-output');
  const indicator = document.getElementById('stream-indicator');
  const historyDiv = document.getElementById('opencode-history');
  const historyToggle = document.getElementById('history-toggle');
  const clearBtn = document.getElementById('clear-history-btn');
  const streamToggle = document.getElementById('stream-toggle');

  let streaming = false;

  listen('opencode-stream-line', (e) => {
    if (streamToggle.checked && streaming) {
      output.textContent += e.payload;
      output.scrollTop = output.scrollHeight;
    }
  });

  listen('opencode-stream-done', () => {
    streaming = false;
    indicator.textContent = '';
  });

  async function query() {
    const q = input.value.trim();
    if (!q || streaming) return;
    streaming = true;
    indicator.textContent = 'streaming...';
    output.textContent = '';

    try {
      const r = await invoke('opencode_query', { query: q });
      if (!streamToggle.checked) {
        output.textContent = r.stdout || r.stderr || '(empty)';
      }
    } catch (err) {
      output.textContent = `Error: ${err}`;
    }
    streaming = false;
    indicator.textContent = '';
  }

  btn.addEventListener('click', query);
  input.addEventListener('keydown', e => { if (e.key === 'Enter') query(); });

  clearBtn.addEventListener('click', () => {
    invoke('clear_opencode_history');
    historyDiv.innerHTML = '';
  });

  historyToggle.addEventListener('change', () => {
    if (historyToggle.checked) {
      invoke('get_opencode_history').then(h => {
        historyDiv.innerHTML = h.map(e =>
          `<div class="hist-entry"><b>${e.query}</b> <span class="dim">${e.timestamp}</span><pre>${e.stdout.slice(0, 200)}</pre></div>`
        ).join('');
      });
      historyDiv.style.display = 'block';
      output.style.display = 'none';
    } else {
      historyDiv.style.display = 'none';
      output.style.display = 'block';
    }
  });
}

// ── Cascade panel ──

function initCascade() {
  const input = document.getElementById('cascade-input');
  const btn = document.getElementById('cascade-send');
  const output = document.getElementById('cascade-output');
  const status = document.getElementById('cascade-status');

  async function query() {
    const q = input.value.trim();
    if (!q) return;
    status.textContent = 'querying...';
    output.textContent = '';
    try {
      const r = await invoke('cascade_query', { query: q });
      output.textContent = r.output || r.error || '(empty)';
    } catch (err) {
      output.textContent = 'Not available on this platform';
    }
    status.textContent = '';
  }

  btn.addEventListener('click', query);
  input.addEventListener('keydown', e => { if (e.key === 'Enter') query(); });
}

// ── RAG panel ──

function initRag() {
  const input = document.getElementById('rag-input');
  const btn = document.getElementById('rag-search-btn');
  const results = document.getElementById('rag-results');
  const ingestInput = document.getElementById('rag-ingest-input');
  const ingestBtn = document.getElementById('rag-ingest-btn');

  btn.addEventListener('click', async () => {
    const q = input.value.trim();
    if (!q) return;
    results.innerHTML = '<div class="loading">searching...</div>';
    try {
      const r = await invoke('rag_search', { query: q, limit: 5 });
      results.innerHTML = '';
      if (r.results) {
        r.results.forEach(res => {
          const div = document.createElement('div');
          div.className = 'rag-result';
          div.innerHTML = `<div class="rag-score">${(res.score * 100).toFixed(0)}%</div>
            <div class="rag-snippet">${res.snippet}</div>
            <div class="rag-source dim">${res.source || ''}</div>`;
          results.appendChild(div);
        });
      } else {
        results.innerHTML = `<div class="dim">${r.error || 'no results'}</div>`;
      }
    } catch (err) {
      results.innerHTML = '<div class="dim">Not available on this platform</div>';
    }
  });

  ingestBtn.addEventListener('click', async () => {
    const p = ingestInput.value.trim();
    if (!p) return;
    ingestBtn.textContent = 'ingesting...';
    try {
      const r = await invoke('rag_ingest', { path: p });
      ingestBtn.textContent = r.success ? 'done' : `error: ${r.error}`;
    } catch (err) {
      ingestBtn.textContent = 'Not available on this platform';
    }
    setTimeout(() => { ingestBtn.textContent = 'INGEST'; }, 2000);
  });
}

// ── WSL panel ──

function initWsl() {
  const input = document.getElementById('wsl-input');
  const btn = document.getElementById('wsl-exec-btn');
  const output = document.getElementById('wsl-output');

  async function run() {
    const cmd = input.value.trim();
    if (!cmd) return;
    output.textContent = '$ ' + cmd + '\n';
    try {
      const r = await invoke('shell_exec', { command: cmd });
      output.textContent += r.stdout || r.stderr || '(empty)';
    } catch (err) {
      output.textContent += `Error: ${err}`;
    }
  }

  btn.addEventListener('click', run);
  input.addEventListener('keydown', e => { if (e.key === 'Enter') run(); });
}

// ── Health panel ──

function initHealth() {
  const refreshBtn = document.getElementById('refresh-health');
  refreshBtn.addEventListener('click', () => refreshHealth(true));
}

function initHeartbeat() {
  const toggle = document.getElementById('heartbeat-toggle');
  invoke('get_heartbeat').then(active => toggle.checked = active);
  toggle.addEventListener('change', () => {
    invoke('set_heartbeat', { active: toggle.checked });
  });
}

function initAutostart() {
  const btn = document.getElementById('autostart-btn');
  invoke('get_autostart').then(enabled => {
    btn.textContent = `Autostart: ${enabled ? 'ON' : 'OFF'}`;
  });
  btn.addEventListener('click', () => {
    invoke('toggle_autostart').then(enabled => {
      btn.textContent = `Autostart: ${enabled ? 'ON' : 'OFF'}`;
    });
  });

  // Restart button
  document.getElementById('restart-btn').addEventListener('click', () => {
    invoke('restart_services').then(r => {
      refreshHealth(false);
    });
  });

  // Settings button + modal
  const overlay = document.getElementById('settings-overlay');
  document.getElementById('settings-btn').addEventListener('click', () => {
    invoke('get_app_settings').then(s => {
      document.getElementById('set-heartbeat').checked = s.heartbeat_active;
      document.getElementById('set-hb-interval').value = s.heartbeat_interval_secs;
      document.getElementById('set-rec-interval').value = s.recovery_interval_secs;
      document.getElementById('set-fuche-dir').value = s.fuche_coder_dir;
      document.getElementById('set-ss-dir').value = s.screenshot_dir;
      document.getElementById('set-ollama-port').value = s.ollama_port;
      document.getElementById('set-tts-port').value = s.tts_port;
      document.getElementById('set-whisper-port').value = s.whisper_port;
    });
    overlay.style.display = 'flex';
  });

  document.getElementById('settings-close').addEventListener('click', () => {
    overlay.style.display = 'none';
  });

  document.getElementById('settings-save').addEventListener('click', () => {
    const settings = {
      heartbeat_active: document.getElementById('set-heartbeat').checked,
      heartbeat_interval_secs: parseInt(document.getElementById('set-hb-interval').value),
      recovery_interval_secs: parseInt(document.getElementById('set-rec-interval').value),
      fuche_coder_dir: document.getElementById('set-fuche-dir').value,
      screenshot_dir: document.getElementById('set-ss-dir').value,
      ollama_port: parseInt(document.getElementById('set-ollama-port').value),
      tts_port: parseInt(document.getElementById('set-tts-port').value),
      whisper_port: parseInt(document.getElementById('set-whisper-port').value),
    };
    invoke('set_app_settings', { settings }).then(() => {
      document.getElementById('settings-status').textContent = 'saved';
      setTimeout(() => { document.getElementById('settings-status').textContent = ''; }, 2000);
    });
  });
}

async function refreshHealth(showLoading) {
  const body = document.getElementById('health-body');
  const dot = document.getElementById('status-dot');
  const text = document.getElementById('status-text');
  if (showLoading) body.innerHTML = '<div class="loading">scanning services...</div>';
  try {
    const r = await invoke('health_check');
    body.innerHTML = '';
    r.services.forEach(s => {
      const div = document.createElement('div');
      div.className = 'health-row';
      div.innerHTML = `<span class="health-dot ${s.status.toLowerCase()}"></span>
        <span class="health-label">${s.label}</span>
        <span class="health-detail">${s.detail}</span>`;
      body.appendChild(div);
    });
    dot.className = `status-dot ${r.overall.toLowerCase()}`;
    text.textContent = r.overall;
  } catch (err) {
    body.innerHTML = `<div class="dim">Error: ${err}</div>`;
  }
}
