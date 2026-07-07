use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct OcrResult {
    pub file: String,
    pub ocr_text: String,
    pub moondream: String,
    pub timestamp: String,
    pub success: bool,
}

fn win_to_wsl(path: &str) -> String {
    let p = path.replace('\\', "/");
    if let Some(rest) = p.strip_prefix("C:") {
        format!("/mnt/c{}", rest)
    } else if let Some(rest) = p.strip_prefix("D:") {
        format!("/mnt/d{}", rest)
    } else {
        p
    }
}

pub fn run_tesseract(path: &str) -> String {
    let wsl = win_to_wsl(path);
    match super::wsl::execute_timeout(&["tesseract", &wsl, "stdout", "-l", "eng"], 10) {
        Ok(out) => {
            let lines: Vec<&str> = out.stdout.lines().filter(|l| !l.trim().is_empty()).collect();
            if lines.is_empty() { return String::new(); }
            let text = lines.join(" ");
            if text.len() > 500 { text[..500].to_string() } else { text }
        }
        Err(e) => { println!("Tesseract timeout: {e}"); String::new() }
    }
}

pub fn run_moondream(path: &str) -> String {
    let wsl = win_to_wsl(path);
    let start = std::time::Instant::now();
    let cmd = format!("ollama run moondream 'describe {}' 2>/dev/null", wsl);
    let result = match super::wsl::execute_timeout(&["bash", "-l", "-c", &cmd], 10) {
        Ok(out) => {
            let cleaned = strip_ansi(&out.stdout);
            let desc = cleaned.trim().to_string();
            if desc.len() > 300 { desc[..300].to_string() } else { desc }
        }
        Err(e) => { println!("Moondream timeout: {e}"); String::new() }
    };
    println!("moondream took {:?}", start.elapsed());
    result
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            while let Some(&n) = chars.peek() {
                if n.is_ascii_alphabetic() || n == '?' || n == 'K' || n == 'h' || n == 'l' {
                    chars.next();
                    break;
                }
                let _ = chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn log_result(file: &str, ocr: &str, moondream: &str) {
    let log_path = r"C:\Users\ACER\OneDrive\Obsidian Vault\system\auto-ocr.md";
    let ts = chrono_now();
    let ocr_safe = ocr.replace('|', "/").replace('\n', " ");
    let moon_safe = moondream.replace('|', "/").replace('\n', " ");
    let line = format!("| {} | {} | {} | {} |\n", ts, file, ocr_safe, moon_safe);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
        use std::io::Write;
        let _ = f.write_all(line.as_bytes());
    }
}

pub fn process_screenshot(path: &str) -> OcrResult {
    let file = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let start = std::time::Instant::now();
    let ocr_text = run_tesseract(path);
    let moondream = run_moondream(path);
    let elapsed = start.elapsed().as_secs();
    let timestamp = chrono_now();
    let success = !ocr_text.is_empty() || !moondream.is_empty();

    println!("Processed {} in {}s — OCR:{}, Moon:{}", file, elapsed, !ocr_text.is_empty(), !moondream.is_empty());

    log_result(&file, &ocr_text, &moondream);

    OcrResult { file, ocr_text, moondream, timestamp, success }
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs();
    // days since epoch
    let days = secs / 86400;
    // compute year/month/day from days since 1970-01-01
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year { break; }
        remaining -= days_in_year;
        y += 1;
    }
    let month_days = if is_leap(y) { &LEAP_MONTH_DAYS[..] } else { &NORM_MONTH_DAYS[..] };
    let mut m = 0;
    for &md in month_days {
        if remaining < md as i64 { break; }
        remaining -= md as i64;
        m += 1;
    }
    let day = remaining + 1;
    let time = secs % 86400;
    let h = time / 3600;
    let min = (time % 3600) / 60;
    let s = time % 60;
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m + 1, day, h, min, s)
}

const NORM_MONTH_DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const LEAP_MONTH_DAYS: [u32; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}
