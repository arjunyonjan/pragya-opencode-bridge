mod bridge;
mod commands;

use bridge::{opencode, health, ocr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

struct OcrSem(Mutex<u32>);
impl OcrSem {
    fn new(n: u32) -> Self { OcrSem(Mutex::new(n)) }
    fn acquire(&self) -> OcrGuard<'_> {
        loop {
            let mut count = self.0.lock().unwrap();
            if *count > 0 { *count -= 1; return OcrGuard(&self.0); }
            drop(count);
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
struct OcrGuard<'a>(&'a Mutex<u32>);
impl Drop for OcrGuard<'_> {
    fn drop(&mut self) { *self.0.lock().unwrap() += 1; }
}
use tauri::{AppHandle, Emitter, Manager, State};
// ManagerExt used in commands.rs
use serde::{Deserialize, Serialize};

pub static HEARTBEAT_ACTIVE: AtomicBool = AtomicBool::new(true);

#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub heartbeat_active: bool,
    pub heartbeat_interval_secs: u64,
    pub recovery_interval_secs: u64,
    pub fuche_coder_dir: String,
    pub screenshot_dir: String,
    pub ollama_port: u16,
    pub tts_port: u16,
    pub whisper_port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            heartbeat_active: true,
            heartbeat_interval_secs: 30,
            recovery_interval_secs: 60,
            fuche_coder_dir: "~/fuche-coder".into(),
            screenshot_dir: r"C:\Users\ACER\OneDrive\ai-screenshots".into(),
            ollama_port: 11434,
            tts_port: 8750,
            whisper_port: 8760,
        }
    }
}

pub fn settings_path(app: &AppHandle) -> PathBuf {
    app.path().app_config_dir().unwrap_or_default().join("settings.json")
}

pub fn load_settings(app: &AppHandle) -> Settings {
    let path = settings_path(app);
    std::fs::read_to_string(path).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_settings(app: &AppHandle, settings: &Settings) {
    if let Ok(s) = serde_json::to_string_pretty(settings) {
        let path = settings_path(app);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, s);
    }
}

pub struct AppState {
    pub tray_icon: Mutex<tauri::tray::TrayIcon>,
    pub opencode_history: Mutex<Vec<opencode::HistoryEntry>>,
}

#[derive(Serialize)]
pub struct HealthReport {
    pub overall: String,
    pub services: Vec<ServiceReport>,
}

#[derive(Serialize)]
pub struct ServiceReport {
    pub label: String,
    pub status: String,
    pub detail: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let settings = load_settings(app.handle());
            HEARTBEAT_ACTIVE.store(settings.heartbeat_active, Ordering::Relaxed);

            let icon = load_tray_png(app.path().resource_dir().unwrap_or_default().join("icons/tray-green.png"));
            let tray = app.tray_by_id("main").expect("tray icon not found in config");
            let tray_clone = tray.clone();
            tray.set_icon(Some(icon)).ok();
            tray.on_tray_icon_event(|tray, event| {
                if let tauri::tray::TrayIconEvent::Click { .. } = event {
                    if let Some(window) = tray.app_handle().get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
            });

            app.manage(AppState {
                tray_icon: Mutex::new(tray_clone),
                opencode_history: Mutex::new(Vec::new()),
            });

            // Tray menu
            use tauri::menu::{MenuBuilder, MenuItemBuilder};
            if let Ok(menu) = MenuBuilder::new(app.handle())
                .item(&MenuItemBuilder::with_id("restart", "Restart Services").build(app.handle())?)
                .separator()
                .item(&MenuItemBuilder::with_id("show", "Show/Hide Window").build(app.handle())?)
                .separator()
                .item(&MenuItemBuilder::with_id("quit", "Quit").build(app.handle())?)
                .build()
            {
                let _ = tray.set_menu(Some(menu));
            }
            let app_handle = app.handle().clone();
            tray.on_menu_event(move |_app, event| {
                match event.id.as_ref() {
                    "restart" => { restart_dead_services_sync(&app_handle); }
                    "show" => {
                        if let Some(w) = _app.get_webview_window("main") {
                            if w.is_visible().unwrap_or(false) { let _ = w.hide(); }
                            else { let _ = w.show(); let _ = w.set_focus(); }
                        }
                    }
                    "quit" => { _app.exit(0); }
                    _ => {}
                }
            });

            // Heartbeat loop with auto-recovery
            let h = app.handle().clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    if !HEARTBEAT_ACTIVE.load(Ordering::Relaxed) { continue; }
                    rt.block_on(heartbeat_loop(&h));
                }
            });

            // Auto-recovery loop — restarts dead services
            let h_auto = app.handle().clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                loop {
                    let settings = load_settings(&h_auto);
                    std::thread::sleep(std::time::Duration::from_secs(settings.recovery_interval_secs));
                    rt.block_on(auto_recovery(&settings));
                }
            });

            // TTS tick loop
            let h2 = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    let _ = h2.emit("tts-tick", 0u32);
                }
            });

            // Screenshot watcher — lazy 10s loop, max 2 concurrent OCR tasks
            let h3 = app.handle().clone();
            let semaphore = std::sync::Arc::new(OcrSem::new(2));
            std::thread::spawn(move || {
                #[cfg(target_os = "android")]
                let ss_dir = "/sdcard/Pictures/Screenshots";
                #[cfg(not(target_os = "android"))]
                let ss_dir = r"C:\Users\ACER\OneDrive\ai-screenshots";
                #[cfg(target_os = "android")]
                let log_path = "/storage/emulated/0/Android/data/com.fuche.pragya/files/auto-ocr.md";
                #[cfg(not(target_os = "android"))]
                let log_path = r"C:\Users\ACER\OneDrive\Obsidian Vault\system\auto-ocr.md";
                let mut processed: std::collections::HashSet<String> = std::fs::read_to_string(log_path)
                    .ok().map(|s| s.lines().filter_map(|l| l.split('|').nth(1)).map(|s| s.trim().to_string()).collect())
                    .unwrap_or_default();
                println!("OCR: {} files known", processed.len());
                loop {
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        for entry in std::fs::read_dir(ss_dir).into_iter().flatten().flatten() {
                            let path = entry.path();
                            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                            if !["jpg", "jpeg", "png"].contains(&ext.as_str()) { continue; }
                            let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                            let fname2 = fname.clone();
                            if !processed.insert(fname) { continue; }
                            let h = h3.clone();
                            let p = path.to_string_lossy().to_string();
                            let sem = semaphore.clone();
                            std::thread::spawn(move || {
                                let _g = sem.acquire();
                                let r = ocr::process_screenshot(&p);
                                let _ = h.emit("ocr-result", &r);
                                if r.success { println!("OCR done: {fname2}"); }
                            });
                        }
                    }));
                    if let Err(e) = r {
                        eprintln!("OCR watcher crashed: {:?}, restarting", e);
                    }
                    std::thread::sleep(std::time::Duration::from_secs(10));
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::shell_exec,
            commands::tts_speak,
            commands::tts_speak_with,
            commands::opencode_status,
            commands::opencode_query,
            commands::get_opencode_history,
            commands::clear_opencode_history,
            #[cfg(not(target_os = "android"))]
            commands::cascade_query,
            #[cfg(not(target_os = "android"))]
            commands::rag_search,
            #[cfg(not(target_os = "android"))]
            commands::rag_ingest,
            commands::health_check,
            commands::get_autostart,
            commands::toggle_autostart,
            commands::set_heartbeat,
            commands::get_heartbeat,
            commands::restart_services,
            commands::get_app_settings,
            commands::set_app_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PRAGYA");
}

// ── Helpers ──

pub fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs();
    let days = secs / 86400;
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year { break; }
        remaining -= days_in_year;
        y += 1;
    }
    let month_days = if is_leap(y) { &LEAP_MONTH_DAYS[..] } else { &NORM_MONTH_DAYS[..] };
    let mut m = 0;
    for &md in month_days {
        if remaining < md as i64 { break; }
        remaining -= md as i64;
        m += 1;
    }
    let day = remaining + 1;
    let time = secs % 86400;
    let h = time / 3600;
    let min = (time % 3600) / 60;
    let s = time % 60;
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m + 1, day, h, min, s)
}

const NORM_MONTH_DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const LEAP_MONTH_DAYS: [u32; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

pub fn load_tray_png(path: std::path::PathBuf) -> tauri::image::Image<'static> {
    match std::fs::read(&path) {
        Ok(bytes) => {
            if let Ok(img) = image::load_from_memory(&bytes) {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let pixels: &'static [u8] = rgba.into_vec().leak();
                tauri::image::Image::new(pixels, w, h)
            } else {
                tauri::image::Image::new(&[0u8; 4], 1, 1)
            }
        }
        Err(_) => tauri::image::Image::new(&[0u8; 4], 1, 1),
    }
}

pub fn update_tray(handle: &AppHandle, state: &State<AppState>, overall: &health::HealthStatus) {
    let name = match overall {
        health::HealthStatus::Healthy => "tray-green.png",
        health::HealthStatus::Degraded => "tray-amber.png",
        health::HealthStatus::Down => "tray-red.png",
    };
    let res = handle.path().resource_dir().unwrap_or_default().join(format!("icons/{}", name));
    let img = load_tray_png(res);
    if let Ok(tray) = state.tray_icon.lock() {
        tray.set_icon(Some(img)).ok();
    }
}

async fn heartbeat_loop(handle: &AppHandle) {
    let dh = health::check_all().await;
    if let Some(state) = handle.try_state::<AppState>() {
        update_tray(handle, &state, &dh.overall);
    }
    let label = match dh.overall {
        health::HealthStatus::Healthy => "\u{1f7e2}",
        health::HealthStatus::Degraded => "\u{1f7e1}",
        health::HealthStatus::Down => "\u{1f534}",
    };
    let _ = handle.emit("heartbeat", label);
}

/// Auto-recovery: restarts dead services
async fn auto_recovery(settings: &Settings) {
    // Ollama
    let ollama_up = bridge::shell::execute(&format!("curl -s -o /dev/null -w '%{{http_code}}' http://localhost:{}/api/tags 2>/dev/null || echo '000'", settings.ollama_port))
        .map(|o| o.stdout.trim() == "200").unwrap_or(false);
    if !ollama_up {
        println!("Auto-recovery: starting Ollama...");
        let _ = bridge::shell::execute("bash -l -c 'ollama serve > /dev/null 2>&1 &'");
    }

    // Search daemon
    let daemon_up = bridge::shell::execute("test -S /tmp/search-daemon.sock && echo ok || echo no")
        .map(|o| o.stdout.trim() == "ok").unwrap_or(false);
    if !daemon_up {
        println!("Auto-recovery: starting search daemon...");
        let _ = bridge::shell::execute(&format!("bash -l -c 'cd {} && source venv/bin/activate && setsid python3 -u search.py --daemon > /dev/null 2>&1 &'", settings.fuche_coder_dir));
    }

    // TTS daemon
    let tts_up = bridge::shell::execute(&format!("curl -s -o /dev/null -w '%{{http_code}}' http://localhost:{} 2>/dev/null || echo '000'", settings.tts_port))
        .map(|o| o.stdout.trim() == "200").unwrap_or(false);
    if !tts_up {
        println!("Auto-recovery: TTS daemon not responding (OK if not needed)");
    }

    // Whisper.cpp
    let whisper_up = bridge::shell::execute(&format!("curl -s -o /dev/null -w '%{{http_code}}' http://localhost:{} 2>/dev/null || echo '000'", settings.whisper_port))
        .map(|o| o.stdout.trim() == "200").unwrap_or(false);
    if !whisper_up {
        println!("Auto-recovery: whisper.cpp not responding (OK if not needed)");
    }
}

/// Sync helper for tray menu — spawns a thread with its own tokio runtime
fn restart_dead_services_sync(handle: &tauri::AppHandle) {
    let h = handle.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let settings = load_settings(&h);
        rt.block_on(auto_recovery(&settings));
        println!("Auto-recovery: manual restart triggered");
    });
}

/// Public async entry for the restart_services Tauri command
pub async fn restart_dead_services(app: &tauri::AppHandle) {
    let settings = load_settings(app);
    auto_recovery(&settings).await;
}
