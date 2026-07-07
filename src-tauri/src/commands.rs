use crate::bridge::{shell, tts, opencode, health};
#[cfg(not(target_os = "android"))]
use crate::bridge::{rag, cascade};
use crate::{AppState, HealthReport, ServiceReport, Settings, chrono_now, update_tray, HEARTBEAT_ACTIVE, save_settings, load_settings, restart_dead_services};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

#[cfg(not(target_os = "android"))]
#[tauri::command]
pub fn shell_exec(command: String) -> Result<shell::ShellOutput, String> {
    shell::execute(&command)
}

#[cfg(target_os = "android")]
#[tauri::command]
pub fn shell_exec(_command: String) -> Result<shell::ShellOutput, String> {
    Err("WSL not available on Android".into())
}

#[cfg(not(target_os = "android"))]
#[tauri::command]
pub async fn tts_speak(text: String) -> tts::TtsResult {
    tts::speak(&text).await
}

#[cfg(target_os = "android")]
#[tauri::command]
pub async fn tts_speak(_text: String) -> tts::TtsResult {
    tts::TtsResult { success: false, message: "TTS not available on Android".into(), elapsed_ms: 0 }
}

#[cfg(not(target_os = "android"))]
#[tauri::command]
pub async fn tts_speak_with(text: String, backend: Option<String>, preset: Option<String>, speed: Option<f64>, fx: Option<String>) -> tts::TtsResult {
    tts::speak_with(&text, &backend.unwrap_or("kitten".into()), &preset.unwrap_or("jarvis".into()), speed.unwrap_or(1.25), &fx.unwrap_or_default()).await
}

#[cfg(target_os = "android")]
#[tauri::command]
pub async fn tts_speak_with(_text: String, _backend: Option<String>, _preset: Option<String>, _speed: Option<f64>, _fx: Option<String>) -> tts::TtsResult {
    tts::TtsResult { success: false, message: "TTS not available on Android".into(), elapsed_ms: 0 }
}

#[tauri::command]
pub fn opencode_status() -> opencode::Status {
    opencode::check_status()
}

#[tauri::command]
pub async fn opencode_query(query: String, app: AppHandle) -> Result<opencode::Session, String> {
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
pub fn get_opencode_history(state: State<'_, AppState>) -> Vec<opencode::HistoryEntry> {
    state.opencode_history.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[tauri::command]
pub fn clear_opencode_history(state: State<'_, AppState>) {
    if let Ok(mut h) = state.opencode_history.lock() { h.clear(); }
}

#[cfg(not(target_os = "android"))]
#[tauri::command]
pub async fn cascade_query(query: String) -> cascade::CascadeResult {
    cascade::run_query(&query).await
}

#[cfg(not(target_os = "android"))]
#[tauri::command]
pub async fn rag_search(query: String, limit: Option<usize>) -> rag::RagResult {
    rag::search(&query, limit).await
}

#[cfg(not(target_os = "android"))]
#[tauri::command]
pub async fn rag_ingest(path: String) -> rag::RagResult {
    rag::ingest(&path).await
}

#[tauri::command]
pub async fn health_check(app: AppHandle) -> Result<HealthReport, String> {
    let dh = health::check_all().await;
    if let Some(state) = app.try_state::<AppState>() {
        update_tray(&app, &state, &dh.overall);
    }
    Ok(HealthReport {
        overall: format!("{:?}", dh.overall),
        services: vec![
            ServiceReport { label: "WSL".into(), status: format!("{:?}", dh.wsl.status), detail: dh.wsl.detail },
            ServiceReport { label: "TTS".into(), status: format!("{:?}", dh.tts.status), detail: dh.tts.detail },
            ServiceReport { label: "Whisper".into(), status: format!("{:?}", dh.whisper.status), detail: dh.whisper.detail },
            ServiceReport { label: "Opencode".into(), status: format!("{:?}", dh.opencode.status), detail: dh.opencode.detail },
            ServiceReport { label: "Cascade".into(), status: format!("{:?}", dh.cascade.status), detail: dh.cascade.detail },
            ServiceReport { label: "Ollama".into(), status: format!("{:?}", dh.ollama.status), detail: dh.ollama.detail },
            ServiceReport { label: "GPU".into(), status: format!("{:?}", dh.gpu.status), detail: format!("{} ({} / {} MB)", dh.gpu.gpu_name, dh.gpu.vram_used_mb, dh.gpu.vram_total_mb) },
            ServiceReport { label: "Disk".into(), status: format!("{:?}", dh.disk.status), detail: format!("{:.1} / {:.1} GB ({:.0}%)", dh.disk.free_gb, dh.disk.total_gb, dh.disk.usage_pct) },
        ],
    })
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn toggle_autostart(app: AppHandle) -> bool {
    if app.autolaunch().is_enabled().unwrap_or(false) {
        let _ = app.autolaunch().disable();
        false
    } else {
        let _ = app.autolaunch().enable();
        true
    }
}

#[tauri::command]
pub fn set_heartbeat(active: bool, app: AppHandle) {
    HEARTBEAT_ACTIVE.store(active, std::sync::atomic::Ordering::Relaxed);
    let mut s = load_settings(&app);
    s.heartbeat_active = active;
    save_settings(&app, &s);
}

#[tauri::command]
pub fn get_heartbeat() -> bool {
    HEARTBEAT_ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
}

#[tauri::command]
pub async fn restart_services(app: AppHandle) -> String {
    restart_dead_services(&app).await;
    "done".into()
}

#[tauri::command]
pub fn get_app_settings(app: AppHandle) -> Settings {
    load_settings(&app)
}

#[tauri::command]
pub fn set_app_settings(settings: Settings, app: AppHandle) {
    HEARTBEAT_ACTIVE.store(settings.heartbeat_active, std::sync::atomic::Ordering::Relaxed);
    save_settings(&app, &settings);
}
