#[cfg(not(target_os = "android"))]
pub mod shell;
#[cfg(target_os = "android")]
mod termux;
#[cfg(target_os = "android")]
pub use termux as shell;

#[cfg(not(target_os = "android"))]
pub mod tts;
#[cfg(target_os = "android")]
pub mod tts {
    use serde::Serialize;
    #[derive(Serialize)]
    pub struct TtsResult { pub success: bool, pub message: String, pub elapsed_ms: u64 }
    pub async fn speak(_text: &str) -> TtsResult { TtsResult { success: false, message: "no TTS on Android".into(), elapsed_ms: 0 } }
    pub async fn speak_with(_text: &str, _backend: &str, _preset: &str, _speed: f64, _fx: &str) -> TtsResult { TtsResult { success: false, message: "no TTS on Android".into(), elapsed_ms: 0 } }
    pub fn health() -> bool { false }
}
pub mod opencode;
#[cfg(not(target_os = "android"))]
pub mod rag;
#[cfg(not(target_os = "android"))]
pub mod cascade;
pub mod health;
pub mod ocr;
