use serde::Serialize;

#[derive(Serialize)]
pub struct RagResult {
    pub query: String,
    pub results: Vec<Entry>,
    pub success: bool,
    pub error: String,
}

#[derive(Serialize)]
pub struct Entry {
    pub score: f64,
    pub source: String,
    pub snippet: String,
}

pub async fn search(query: &str, limit: Option<usize>) -> RagResult {
    let limit = limit.unwrap_or(5);
    let safe = query.replace('\'', "'\\''");
    let cmd = format!("fuche search --collection default --limit {} '{}'", limit, safe);
    let result = tokio::task::spawn_blocking(move || super::wsl::execute(&cmd)).await;

    match result {
        Ok(Ok(out)) if out.success => RagResult {
            query: query.to_string(), results: parse(&out.stdout), success: true, error: String::new(),
        },
        Ok(Ok(out)) => RagResult { query: query.to_string(), results: vec![], success: false, error: out.stderr },
        Ok(Err(e)) => RagResult { query: query.to_string(), results: vec![], success: false, error: e },
        Err(e) => RagResult { query: query.to_string(), results: vec![], success: false, error: format!("task panic: {}", e) },
    }
}

pub async fn ingest(path: &str) -> RagResult {
    let safe = path.replace('\'', "'\\''");
    let cmd = format!("fuche ingest '{}'", safe);
    let result = tokio::task::spawn_blocking(move || super::wsl::execute(&cmd)).await;

    match result {
        Ok(Ok(out)) => RagResult { query: path.to_string(), results: vec![], success: out.success, error: if out.success { String::new() } else { out.stderr } },
        Ok(Err(e)) => RagResult { query: path.to_string(), results: vec![], success: false, error: e },
        Err(e) => RagResult { query: path.to_string(), results: vec![], success: false, error: format!("task panic: {}", e) },
    }
}

fn parse(output: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut src = String::new();
    let mut score = 0.0;
    let mut snippet = String::new();

    for line in output.lines() {
        let t = line.trim();
        if t.to_lowercase().starts_with("score:") {
            if !src.is_empty() { entries.push(Entry { score, source: src.clone(), snippet: snippet.clone() }); }
            score = t.split(':').nth(1).and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
            src.clear(); snippet.clear();
        } else if t.to_lowercase().starts_with("source:") {
            src = t.split(':').nth(1).map(|s| s.trim().to_string()).unwrap_or_default();
        } else if !src.is_empty() {
            if !snippet.is_empty() { snippet.push('\n'); }
            snippet.push_str(t);
        }
    }
    if !src.is_empty() { entries.push(Entry { score, source: src, snippet }); }
    entries
}
