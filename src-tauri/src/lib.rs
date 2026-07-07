mod bridge;

use bridge::{wsl, tts, opencode, rag, cascade, health};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use serde::{Deserialize, Serialize};

static HEARTBEAT_ACTIVE: AtomicBool = AtomicBool::new(true);

#[derive(Serialize, Deserialize)]
struct Settings {
    heartbeat_active: bool,
}

impl Default for Settings {
    fn default() -> Self { Self { heartbeat_active: true } }
}

fn settings_path(app: &AppHandle) -> PathBuf {
    app.path().app_config_dir().unwrap_or_default().join("settings.json")
}

fn load_settings(app: &AppHandle) -> Settings {
    let path = settings_path(app);
    std::fs::read_to_string(path).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_settings(app: &AppHandle, settings: &Settings) {
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

            let h = app.handle().clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    if !HEARTBEAT_ACTIVE.load(Ordering::Relaxed) { continue; }
                    rt.block_on(heartbeat_loop(&h));
                }
            });

            let h2 = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    let _ = h2.emit("tts-tick", 0u32);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            wsl_exec,
            tts_speak,
            tts_speak_with,
            opencode_status,
            opencode_query,
            get_opencode_history,
            clear_opencode_history,
            cascade_query,
            rag_search,
            rag_ingest,
            health_check,
            get_autostart,
            toggle_autostart,
            set_heartbeat,
            get_heartbeat,
        ])
        .run(tauri::generate_context!())
        .expect("error while running opencode-bridge");
}

// ── Commands ──

#[tauri::command]
fn wsl_exec(command: String) -> Result<wsl::WslOutput, String> {
    wsl::execute(&command)
}

#[tauri::command]
async fn tts_speak(text: String) -> tts::TtsResult {
    tts::speak(&text).await
}

#[tauri::command]
async fn tts_speak_with(text: String, backend: Option<String>, preset: Option<String>, speed: Option<f64>, fx: Option<String>) -> tts::TtsResult {
    tts::speak_with(&text, &backend.unwrap_or("kitten".into()), &preset.unwrap_or("jarvis".into()), speed.unwrap_or(1.25), &fx.unwrap_or_default()).await
}

#[tauri::command]
fn opencode_status() -> opencode::Status {
    opencode::check_status()
}

#[tauri::command]
async fn opencode_query(query: String, app: AppHandle) -> Result<opencode::Session, String> {
    let session = opencode::run_query(&query).await;
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut hist) = state.opencode_history.lock() {
            hist.push(opencode::HistoryEntry {
                query: query.clone(),
                stdout: session.stdout.clone(),
                stderr: session.stderr.clone(),
                success: session.success,
                timestamp: chrono_now(),
            });
        }
    }
    Ok(session)
}

#[tauri::command]
fn get_opencode_history(state: State<'_, AppState>) -> Vec<opencode::HistoryEntry> {
    state.opencode_history.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[tauri::command]
fn clear_opencode_history(state: State<'_, AppState>) {
    if let Ok(mut h) = state.opencode_history.lock() { h.clear(); }
}

#[tauri::command]
async fn cascade_query(query: String) -> cascade::CascadeResult {
    cascade::run_query(&query).await
}

#[tauri::command]
async fn rag_search(query: String, limit: Option<usize>) -> rag::RagResult {
    rag::search(&query, limit).await
}

#[tauri::command]
async fn rag_ingest(path: String) -> rag::RagResult {
    rag::ingest(&path).await
}

#[tauri::command]
async fn health_check(app: AppHandle) -> Result<HealthReport, String> {
    let dh = health::check_all().await;
    if let Some(state) = app.try_state::<AppState>() {
        update_tray(&app, &state, &dh.overall);
    }
    Ok(HealthReport {
        overall: format!("{:?}", dh.overall),
        services: vec![
            ServiceReport { label: "WSL".into(), status: format!("{:?}", dh.wsl.status), detail: dh.wsl.detail },
            ServiceReport { label: "TTS".into(), status: format!("{:?}", dh.tts.status), detail: dh.tts.detail },
            ServiceReport { label: "Opencode".into(), status: format!("{:?}", dh.opencode.status), detail: dh.opencode.detail },
            ServiceReport { label: "Cascade".into(), status: format!("{:?}", dh.cascade.status), detail: dh.cascade.detail },
            ServiceReport { label: "Ollama".into(), status: format!("{:?}", dh.ollama.status), detail: dh.ollama.detail },
            ServiceReport { label: "GPU".into(), status: format!("{:?}", dh.gpu.status), detail: format!("{} ({} / {} MB)", dh.gpu.gpu_name, dh.gpu.vram_used_mb, dh.gpu.vram_total_mb) },
            ServiceReport { label: "Disk".into(), status: format!("{:?}", dh.disk.status), detail: format!("{:.1} / {:.1} GB ({:.0}%)", dh.disk.free_gb, dh.disk.total_gb, dh.disk.usage_pct) },
        ],
    })
}

#[tauri::command]
fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn toggle_autostart(app: AppHandle) -> bool {
    if app.autolaunch().is_enabled().unwrap_or(false) {
        let _ = app.autolaunch().disable();
        false
    } else {
        let _ = app.autolaunch().enable();
        true
    }
}

#[tauri::command]
fn set_heartbeat(active: bool, app: AppHandle) {
    HEARTBEAT_ACTIVE.store(active, Ordering::Relaxed);
    save_settings(&app, &Settings { heartbeat_active: active });
}

#[tauri::command]
fn get_heartbeat() -> bool {
    HEARTBEAT_ACTIVE.load(Ordering::Relaxed)
}

// ── Helpers ──

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{}", d.as_secs())
}

fn load_tray_png(path: std::path::PathBuf) -> tauri::image::Image<'static> {
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

fn update_tray(handle: &AppHandle, state: &State<AppState>, overall: &health::HealthStatus) {
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
