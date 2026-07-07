use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;
use serde::Serialize;

#[derive(Serialize)]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

pub fn execute(command: &str) -> Result<WslOutput, String> {
    exec_internal(&["bash", "-c", command], None)
}

pub fn execute_timeout(args: &[&str], secs: u64) -> Result<WslOutput, String> {
    exec_internal(args, Some(Duration::from_secs(secs)))
}

fn exec_internal(args: &[&str], timeout: Option<Duration>) -> Result<WslOutput, String> {
    let mut child = Command::new(args[0]);
    for a in &args[1..] { child.arg(a); }
    child.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = child.spawn().map_err(|e| format!("spawn failed: {}", e))?;
    let mut stdout = child.stdout.take().ok_or("no stdout")?;
    let mut stderr = child.stderr.take().ok_or("no stderr")?;

    let (tx_stdout, rx_stdout) = std::sync::mpsc::channel();
    let (tx_stderr, rx_stderr) = std::sync::mpsc::channel();
    std::thread::spawn(move || { let mut s = String::new(); stdout.read_to_string(&mut s).ok(); let _ = tx_stdout.send(s); });
    std::thread::spawn(move || { let mut s = String::new(); stderr.read_to_string(&mut s).ok(); let _ = tx_stderr.send(s); });

    let status = if let Some(t) = timeout {
        let start = std::time::Instant::now();
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                break status;
            }
            if start.elapsed() >= t {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("timeout after {}s", t.as_secs()));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    } else {
        child.wait().map_err(|e| format!("wait failed: {}", e))?
    };

    let stdout = rx_stdout.recv().unwrap_or_default();
    let stderr = rx_stderr.recv().unwrap_or_default();

    Ok(WslOutput {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
        success: status.success(),
    })
}

pub fn health_check() -> bool {
    execute("echo ok").map(|o| o.stdout.trim() == "ok").unwrap_or(false)
}
