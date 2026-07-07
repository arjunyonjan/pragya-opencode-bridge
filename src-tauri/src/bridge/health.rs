use serde::Serialize;

#[derive(Serialize)]
pub struct DaemonHealth {
    pub wsl: ServiceHealth,
    pub tts: ServiceHealth,
    pub opencode: ServiceHealth,
    pub cascade: ServiceHealth,
    pub ollama: ServiceHealth,
    pub gpu: GpuHealth,
    pub disk: DiskHealth,
    pub overall: HealthStatus,
}

#[derive(Serialize)]
pub struct ServiceHealth {
    pub status: HealthStatus,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub enum HealthStatus { Healthy, Degraded, Down }

#[derive(Serialize)]
pub struct GpuHealth {
    pub status: HealthStatus,
    pub gpu_name: String,
    pub vram_used_mb: u64,
    pub vram_total_mb: u64,
    pub power_capped: bool,
}

#[derive(Serialize)]
pub struct DiskHealth {
    pub status: HealthStatus,
    pub free_gb: f64,
    pub total_gb: f64,
    pub usage_pct: f64,
}

#[cfg(not(target_os = "android"))]
pub async fn check_all() -> DaemonHealth {
    let wsl_ok = super::shell::health_check();
    let tts_ok = super::tts::health();

    let (oc, cc, ol, gpu, disk) = tokio::join!(
        opencode_check(), cascade_check(), ollama_check(), gpu_check(), disk_check()
    );

    let overall = if wsl_ok && tts_ok && oc.status == HealthStatus::Healthy {
        HealthStatus::Healthy
    } else if wsl_ok { HealthStatus::Degraded } else { HealthStatus::Down };

    DaemonHealth {
        wsl: ServiceHealth { status: if wsl_ok { HealthStatus::Healthy } else { HealthStatus::Down }, label: "WSL".into(), detail: if wsl_ok { "Responding".into() } else { "Not reachable".into() } },
        tts: ServiceHealth { status: if tts_ok { HealthStatus::Healthy } else { HealthStatus::Degraded }, label: "TTS".into(), detail: if tts_ok { "fuche-tts found" } else { "Binary not found" }.into() },
        opencode: oc, cascade: cc, ollama: ol, gpu, disk, overall,
    }
}

#[cfg(target_os = "android")]
pub async fn check_all() -> DaemonHealth {
    let (oc, cc, ol, gpu, disk) = tokio::join!(
        opencode_check(), cascade_check(), ollama_check(), gpu_check(), disk_check()
    );

    let overall = if oc.status == HealthStatus::Healthy { HealthStatus::Healthy } else { HealthStatus::Degraded };

    DaemonHealth {
        wsl: ServiceHealth { status: HealthStatus::Down, label: "WSL".into(), detail: "Not available on Android".into() },
        tts: ServiceHealth { status: HealthStatus::Down, label: "TTS".into(), detail: "Not available on Android".into() },
        opencode: oc, cascade: cc, ollama: ol, gpu, disk, overall,
    }
}

async fn opencode_check() -> ServiceHealth {
    let s = super::opencode::check_status();
    ServiceHealth { status: if s.installed { HealthStatus::Healthy } else { HealthStatus::Down }, label: "Opencode".into(), detail: if s.installed { format!("v{}", s.version) } else { "Not installed".into() } }
}

#[cfg(not(target_os = "android"))]
async fn cascade_check() -> ServiceHealth {
    let s = super::cascade::check_status();
    ServiceHealth { status: if s.available { HealthStatus::Healthy } else { HealthStatus::Degraded }, label: "Cascade".into(), detail: if s.available { "cascade.py found" } else { "Not available" }.into() }
}

#[cfg(target_os = "android")]
async fn cascade_check() -> ServiceHealth {
    ServiceHealth { status: HealthStatus::Down, label: "Cascade".into(), detail: "Not available on Android".into() }
}

#[cfg(not(target_os = "android"))]
async fn ollama_check() -> ServiceHealth {
    let r = super::shell::execute("curl -s -o /dev/null -w '%{http_code}' http://localhost:11434/api/tags 2>/dev/null || echo '000'");
    match r {
        Ok(out) if out.stdout.trim() == "200" => ServiceHealth { status: HealthStatus::Healthy, label: "Ollama".into(), detail: "API responding".into() },
        _ => ServiceHealth { status: HealthStatus::Degraded, label: "Ollama".into(), detail: "Not reachable".into() },
    }
}

#[cfg(target_os = "android")]
async fn ollama_check() -> ServiceHealth {
    let r = super::shell::execute("curl -s -o /dev/null -w '%{http_code}' http://localhost:11434/api/tags || echo '000'");
    match r {
        Ok(out) if out.stdout.trim() == "200" => ServiceHealth { status: HealthStatus::Healthy, label: "Ollama".into(), detail: "API responding".into() },
        _ => ServiceHealth { status: HealthStatus::Degraded, label: "Ollama".into(), detail: "Not reachable".into() },
    }
}

#[cfg(not(target_os = "android"))]
async fn gpu_check() -> GpuHealth {
    let r = super::shell::execute("nvidia-smi --query-gpu=name,memory.used,memory.total --format=csv,noheader,nounits 2>/dev/null || echo 'no gpu'");
    match r {
        Ok(out) if out.stdout != "no gpu\n" => {
            let p: Vec<&str> = out.stdout.trim().split(", ").collect();
            GpuHealth { status: HealthStatus::Healthy, gpu_name: p.first().unwrap_or(&"Unknown").to_string(), vram_used_mb: p.get(1).and_then(|s| s.trim().parse().ok()).unwrap_or(0), vram_total_mb: p.get(2).and_then(|s| s.trim().parse().ok()).unwrap_or(0), power_capped: false }
        }
        _ => GpuHealth { status: HealthStatus::Degraded, gpu_name: "N/A".into(), vram_used_mb: 0, vram_total_mb: 0, power_capped: false },
    }
}

#[cfg(target_os = "android")]
async fn gpu_check() -> GpuHealth {
    GpuHealth { status: HealthStatus::Degraded, gpu_name: "N/A".into(), vram_used_mb: 0, vram_total_mb: 0, power_capped: false }
}

#[cfg(not(target_os = "android"))]
async fn disk_check() -> DiskHealth {
    let r = super::shell::execute("df -h / | tail -1 | awk '{print $2, $4, $5}'");
    match r {
        Ok(out) => {
            let p: Vec<&str> = out.stdout.trim().split_whitespace().collect();
            let total = p.first().and_then(|s| parse_size(s)).unwrap_or(0.0);
            let free = p.get(1).and_then(|s| parse_size(s)).unwrap_or(0.0);
            let usage = p.get(2).and_then(|s| s.trim_end_matches('%').parse().ok()).unwrap_or(0.0);
            DiskHealth { status: if usage > 90.0 { HealthStatus::Degraded } else { HealthStatus::Healthy }, free_gb: free, total_gb: total, usage_pct: usage }
        }
        _ => DiskHealth { status: HealthStatus::Degraded, free_gb: 0.0, total_gb: 0.0, usage_pct: 0.0 },
    }
}

#[cfg(target_os = "android")]
async fn disk_check() -> DiskHealth {
    let r = super::shell::execute("df -h /data | tail -1 | awk '{print $2, $4, $5}'");
    match r {
        Ok(out) => {
            let p: Vec<&str> = out.stdout.trim().split_whitespace().collect();
            let total = p.first().and_then(|s| parse_size(s)).unwrap_or(0.0);
            let free = p.get(1).and_then(|s| parse_size(s)).unwrap_or(0.0);
            let usage = p.get(2).and_then(|s| s.trim_end_matches('%').parse().ok()).unwrap_or(0.0);
            DiskHealth { status: if usage > 90.0 { HealthStatus::Degraded } else { HealthStatus::Healthy }, free_gb: free, total_gb: total, usage_pct: usage }
        }
        _ => DiskHealth { status: HealthStatus::Degraded, free_gb: 0.0, total_gb: 0.0, usage_pct: 0.0 },
    }
}

fn parse_size(s: &str) -> Option<f64> {
    if s.ends_with('G') { s.trim_end_matches('G').parse().ok() }
    else if s.ends_with('T') { s.trim_end_matches('T').parse().ok().map(|v: f64| v * 1024.0) }
    else if s.ends_with('M') { s.trim_end_matches('M').parse().ok().map(|v: f64| v / 1024.0) }
    else { s.parse().ok() }
}
