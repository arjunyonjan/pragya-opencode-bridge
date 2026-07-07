use std::process::{Command, Output};
use std::time::Duration;
use serde::Serialize;

#[derive(Serialize)]
pub struct WslOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

pub fn execute(command: &str) -> Result<WslOutput, String> {
    exec_args_internal(&["bash", "-l", "-c", command], None)
}

pub fn execute_timeout(args: &[&str], secs: u64) -> Result<WslOutput, String> {
    exec_args_internal(args, Some(Duration::from_secs(secs)))
}

fn exec_args_internal(args: &[&str], timeout: Option<Duration>) -> Result<WslOutput, String> {
    let mut child = Command::new("wsl");
    child.arg("--");
    for a in args {
        child.arg(a);
    }
    let mut child = child.spawn().map_err(|e| format!("WSL spawn failed: {}", e))?;

    let elapsed = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().unwrap_or_else(|_| Output {
                    stdout: vec![],
                    stderr: vec![],
                    status: std::process::ExitStatus::default(),
                });
                return Ok(WslOutput {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    exit_code: status.code().unwrap_or(-1),
                    success: status.success(),
                });
            }
            Ok(None) => {
                if let Some(t) = timeout {
                    if elapsed.elapsed() >= t {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(format!("WSL timeout after {}s", t.as_secs()));
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(format!("WSL wait failed: {}", e)),
        }
    }
}

pub fn health_check() -> bool {
    execute("echo ok").map(|o| o.stdout.trim() == "ok").unwrap_or(false)
}
