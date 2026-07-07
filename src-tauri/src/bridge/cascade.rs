use serde::Serialize;

#[derive(Serialize)]
pub struct CascadeResult {
    pub query: String,
    pub response: String,
    pub model_info: String,
    pub success: bool,
}

#[derive(Serialize)]
pub struct Status {
    pub available: bool,
}

pub async fn run_query(query: &str) -> CascadeResult {
    let safe = query.replace('\'', "'\\''");
    let cmd = format!("python3 /home/arjun/fuche-coder/cascade.py '{}'", safe);
    let q = query.to_string();
    let result = tokio::task::spawn_blocking(move || super::wsl::execute(&cmd))
        .await.unwrap_or_else(|_| Err("task panic".into()));

    match result {
        Ok(out) => CascadeResult {
            query: q, response: out.stdout, model_info: out.stderr.trim().to_string(), success: out.success,
        },
        Err(e) => CascadeResult { query: q, response: String::new(), model_info: e, success: false },
    }
}

pub fn check_status() -> Status {
    let ok = super::wsl::execute("test -f /home/arjun/fuche-coder/cascade.py && echo 'ok'")
        .map(|o| o.stdout.trim() == "ok").unwrap_or(false);
    Status { available: ok }
}
