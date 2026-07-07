use serde::Serialize;

#[derive(Serialize)]
pub struct TtsResult {
    pub success: bool,
    pub message: String,
    pub elapsed_ms: u64,
}

pub async fn speak(text: &str) -> TtsResult {
    speak_with(text, "kitten", "jarvis", 1.25, "").await
}

pub async fn speak_with(text: &str, backend: &str, preset: &str, speed: f64, fx: &str) -> TtsResult {
    let start = std::time::Instant::now();
    let escaped = text.replace('\'', "'\\''").replace('"', "\\\"");
    let fx_flag = if fx.is_empty() { String::new() } else { format!("--fx {}", fx) };
    let cmd = format!(
        "bash -l -c \"fuche-tts '{}' --speed {} --preset {} --backend {} {}\"",
        escaped, speed, preset, backend, fx_flag
    );

    let result = super::wsl::execute(&cmd);
    let elapsed = start.elapsed().as_millis() as u64;

    match result {
        Ok(out) => TtsResult {
            success: out.success,
            message: if out.success { "OK".into() } else { out.stderr },
            elapsed_ms: elapsed,
        },
        Err(e) => TtsResult { success: false, message: e, elapsed_ms: elapsed },
    }
}

pub fn health() -> bool {
    super::wsl::execute("bash -l -c \"fuche-tts --version\" 2>/dev/null")
        .map(|o| o.success)
        .unwrap_or(false)
}
