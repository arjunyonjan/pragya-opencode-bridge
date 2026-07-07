use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct HistoryEntry {
    pub query: String,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub timestamp: String,
}

#[derive(Serialize)]
pub struct Session {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

#[derive(Serialize)]
pub struct Status {
    pub installed: bool,
    pub version: String,
    pub config_path: String,
}

pub fn check_status() -> Status {
    let version = super::wsl::execute("opencode --version")
        .map(|o| o.stdout.trim().to_string())
        .unwrap_or_default();
    let config = super::wsl::execute("echo $HOME/.config/opencode/config.json")
        .map(|o| o.stdout.trim().to_string())
        .unwrap_or_default();
    Status { installed: !version.is_empty(), version, config_path: config }
}

pub async fn run_query(query: &str) -> Session {
    let command = format!("cd ~ && opencode '{}'", query.replace('\'', "'\\''"));
    let c = command.clone();
    let result = tokio::task::spawn_blocking(move || super::wsl::execute(&c))
        .await.unwrap_or_else(|_| Err("task panic".into()));

    match result {
        Ok(out) => Session { command, stdout: out.stdout, stderr: out.stderr, success: out.success },
        Err(e) => Session { command, stdout: String::new(), stderr: e, success: false },
    }
}
