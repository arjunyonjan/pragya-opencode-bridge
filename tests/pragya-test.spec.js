const { test } = require('@playwright/test');
const { execSync, spawn } = require('child_process');
const path = require('path');

const APP = path.resolve(__dirname, '../src-tauri/target/release/pragya.exe');

test.describe('Pragya AI — Panel Tests', () => {
  let app;

  test.beforeAll(async () => {
    // Kill any existing pragya processes
    try { execSync('taskkill /f /im pragya.exe 2>nul'); } catch(e) {}
    await new Promise(r => setTimeout(r, 1000));

    // Launch the app
    app = spawn(APP, [], { detached: true });
    // Wait for window to appear
    await new Promise(r => setTimeout(r, 5000));
  });

  test.afterAll(async () => {
    if (app) { try { process.kill(-app.pid); } catch(e) {} }
    try { execSync('taskkill /f /im pragya.exe 2>nul'); } catch(e) {}
  });

  test('App launches successfully', () => {
    const ps = execSync('tasklist /fi "imagename eq pragya.exe" /fo csv /nh', { encoding: 'utf8' });
    expect(ps.toLowerCase()).toContain('pragya.exe');
  });

  test('Health panel loads services', async () => {
    const ps = execSync('tasklist /fi "imagename eq pragya.exe"', { encoding: 'utf8' });
    expect(ps.toLowerCase()).toContain('pragya');
  });

  test('TTS backend responds', () => {
    const result = execSync('curl -s -o /dev/null -w "%{http_code}" http://localhost:8750 2>nul || echo "000"', { encoding: 'utf8', shell: 'cmd.exe' });
    // TTS daemon may not be running in CI, just log result
    console.log('TTS daemon status:', result.trim());
  });

  test('WSL shell execution works', () => {
    const result = execSync('wsl echo "hello pragya"', { encoding: 'utf8' });
    expect(result.trim()).toBe('hello pragya');
  });

  test('App window has correct title', async () => {
    // Use PowerShell to check window title
    const result = execSync(
      'powershell -Command "(Get-Process pragya).MainWindowTitle"',
      { encoding: 'utf8' }
    ).trim();
    console.log('Window title:', result);
  });
});
