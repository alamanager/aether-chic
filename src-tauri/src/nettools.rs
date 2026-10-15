//! Through-tunnel speed test + DoH resolver ranking.
//!
//! Both run on demand from the UI (never in the background): the speed
//! test moves real megabytes, the DNS scan fires a handful of HTTPS
//! queries. Timeouts everywhere; any failure is a message, not a hang.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SpeedResult {
    pub down_mbps: f64,
    pub up_mbps: f64,
}

/// Download 8MB + upload 2MB through the given SOCKS proxy, timed.
/// `socks` looks like "127.0.0.1:1870".
#[tauri::command]
pub async fn speed_test(socks: String) -> Result<SpeedResult, String> {
    let proxy =
        reqwest::Proxy::all(format!("socks5h://{socks}")).map_err(|e| format!("proxy: {e}"))?;
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(std::time::Duration::from_secs(90))
        .user_agent("AetherGUI-speed/0.7")
        .build()
        .map_err(|e| format!("http: {e}"))?;

    let t = std::time::Instant::now();
    let down = client
        .get("https://speed.cloudflare.com/__down?bytes=8000000")
        .send()
        .await
        .map_err(|e| format!("down: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("down read: {e}"))?;
    let down_ms = t.elapsed().as_millis().max(1) as f64;

    let body: Vec<u8> = vec![0x41; 2_000_000];
    let t = std::time::Instant::now();
    client
        .post("https://speed.cloudflare.com/__up")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("up: {e}"))?;
    let up_ms = t.elapsed().as_millis().max(1) as f64;

    Ok(SpeedResult {
        down_mbps: (down.len() as f64 * 8.0) / (down_ms / 1000.0) / 1_000_000.0,
        up_mbps: (2_000_000f64 * 8.0) / (up_ms / 1000.0) / 1_000_000.0,
    })
}

const DOH_TARGETS: &[(&str, &str)] = &[
    ("Cloudflare", "https://1.1.1.1/dns-query?name=example.com&type=A"),
    ("Google", "https://8.8.8.8/resolve?name=example.com&type=A"),
    ("Quad9", "https://dns.quad9.net:5053/dns-query?name=example.com&type=A"),
    ("AdGuard", "https://dns.adguard-dns.com/dns-query?name=example.com&type=A"),
    ("Shecan", "https://doh.shecan.ir/dns-query?name=example.com&type=A"),
];

#[derive(Debug, Clone, Serialize)]
pub struct DnsRank {
    pub name: String,
    pub url: String,
    pub ms: Option<u64>,
}

/// Time one DoH query per well-known resolver (parallel, 6s each).
/// "Use fastest" pastes the winner into the ECH resolver field.
#[tauri::command]
pub async fn dns_scan() -> Vec<DnsRank> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .user_agent("AetherGUI-dnsscan/0.7")
        .build();
    let client = match client {
        Ok(c) => c,
        Err(_) => {
            return DOH_TARGETS
                .iter()
                .map(|(name, url)| DnsRank { name: name.to_string(), url: url.to_string(), ms: None })
                .collect()
        }
    };
    let mut handles = Vec::new();
    for (name, url) in DOH_TARGETS {
        let client = client.clone();
        let name = name.to_string();
        let url = url.to_string();
        handles.push(tauri::async_runtime::spawn(async move {
            let t = std::time::Instant::now();
            let ms = client
                .get(&url)
                .header("Accept", "application/dns-json")
                .send()
                .await
                .ok()
                .filter(|r| r.status().is_success())
                .map(|_| t.elapsed().as_millis().min(u64::MAX as u128) as u64);
            DnsRank { name, url, ms }
        }));
    }
    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        match h.await {
            Ok(r) => out.push(r),
            Err(_) => out.push(DnsRank { name: "?".into(), url: String::new(), ms: None }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mbps_math() {
        // 8 Mbit in 1s = 8 Mbps.
        let mbps = (1_000_000f64 * 8.0) / (1000.0 / 1000.0) / 1_000_000.0;
        assert!((mbps - 8.0).abs() < 1e-9);
    }
}
