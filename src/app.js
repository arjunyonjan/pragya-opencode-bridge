import { initTTS } from './panels/tts.js';
import { initOpencode } from './panels/opencode.js';
import { initCascade } from './panels/cascade.js';
import { initRag } from './panels/rag.js';
import { initWsl } from './panels/wsl.js';
import { initHealth, initHeartbeat, initAutostart, refreshHealth } from './panels/health.js';

document.addEventListener('DOMContentLoaded', () => {
  initTTS();
  initOpencode();
  initCascade();
  initRag();
  initWsl();
  initHealth();
  initHeartbeat();
  initAutostart();

  setInterval(() => {
    const now = new Date();
    document.getElementById('footer-time').textContent = now.toLocaleTimeString();
  }, 1000);

  refreshHealth(false);
});
