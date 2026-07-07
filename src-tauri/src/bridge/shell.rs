use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;
use serde::Serialize;
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Serialize)]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

pub fn execute(command: &str) -> Result<ShellOutput, String> {
    exec_args_internal(&["bash", "-l", "-c", command], None)
}

pub fn execute_timeout(args: &[&str], secs: u64) -> Result<ShellOutput, String> {
    exec_args_internal(args, Some(Duration::from_secs(secs)))
}

fn exec_args_internal(args: &[&str], timeout: Option<Duration>) -> Result<ShellOutput, String> {
    let wsl_path = r"C:\Windows\System32\wsl.exe";
    let mut child = Command::new(wsl_path);
    child.creation_flags(CREATE_NO_WINDOW);
    child.arg("--");
    for a in args { child.arg(a); }
    child.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = child.spawn().map_err(|e| format!("WSL spawn failed: {}", e))?;
    let mut stdout = child.stdout.take().ok_or("no stdout")?;
    let mut stderr = child.stderr.take().ok_or("no stderr")?;

    // Read stdout/stderr in threads so pipes don't block
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
                return Err(format!("WSL timeout after {}s", t.as_secs()));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    } else {
        child.wait().map_err(|e| format!("WSL wait failed: {}", e))?
    };

    let stdout = rx_stdout.recv().unwrap_or_default();
    let stderr = rx_stderr.recv().unwrap_or_default();

    Ok(ShellOutput {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
        success: status.success(),
    })
}

pub fn health_check() -> bool {
    execute("echo ok").map(|o| o.stdout.trim() == "ok").unwrap_or(false)
}
