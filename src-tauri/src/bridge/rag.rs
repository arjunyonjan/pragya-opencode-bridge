#[cfg(not(target_os = "android"))]
mod inner {
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
        let result = tokio::task::spawn_blocking(move || super::super::shell::execute(&cmd)).await;

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
        let result = tokio::task::spawn_blocking(move || super::super::shell::execute(&cmd)).await;

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
            if t.is_empty() { continue; }
            // Match fuche search output: "1.  combined=0.520 faiss=0.600 kw=0.400  source-file.md"
            if let Some(cap) = t.strip_prefix(|c: char| c.is_ascii_digit()).and_then(|s| s.strip_prefix(". ")) {
                if !src.is_empty() { entries.push(Entry { score, source: src.clone(), snippet: snippet.clone() }); }
                score = 0.5; // default
                src = cap.split_whitespace().last().unwrap_or("").to_string();
                snippet.clear();
            } else {
                if !snippet.is_empty() { snippet.push('\n'); }
                snippet.push_str(t);
            }
        }
        if !src.is_empty() { entries.push(Entry { score, source: src, snippet }); }
        entries
    }
}

pub use inner::*;
