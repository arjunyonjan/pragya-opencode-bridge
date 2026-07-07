use std::process::{Command, Output};
use serde::Serialize;

#[derive(Serialize)]
pub struct WslOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

pub fn execute(command: &str) -> Result<WslOutput, String> {
    let output: Output = Command::new("wsl")
        .args(["--", command])
        .output()
        .map_err(|e| format!("WSL exec failed: {}", e))?;

    Ok(WslOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
        success: output.status.success(),
    })
}

pub fn health_check() -> bool {
    execute("echo 'ok'").map(|o| o.stdout.trim() == "ok").unwrap_or(false)
}
