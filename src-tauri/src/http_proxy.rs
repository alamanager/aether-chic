//! Minimal HTTP→SOCKS5 bridge (std only, no new crates).
//!
//! Why: Windows system proxy and most apps only speak HTTP proxy, while
//! Aether exposes SOCKS5 only — the same reason v2rayN runs a separate HTTP
//! inbound next to SOCKS. The bridge listens on its own port (SOCKS port + 1)
//! and forwards both CONNECT tunnels and plain-HTTP requests into the
//! SOCKS endpoint. Hostnames are handed to SOCKS (ATYP DOMAIN) for remote
//! resolution, never resolved locally, so nothing leaks around the tunnel.
//! ponytail: thread-per-connection + blocking io::copy — fine for a
//! single-user desktop proxy, replace with async if it ever isn't.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

pub struct HttpProxyHandle {
    shutdown: Arc<AtomicBool>,
    /// "127.0.0.1:1820" — shown in the UI and handed to the system-proxy switch.
    pub addr: String,
}

impl HttpProxyHandle {
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

const HEAD_LIMIT: usize = 32 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(15);

pub fn start(socks_addr: SocketAddr, http_addr: SocketAddr) -> Result<HttpProxyHandle, String> {
    let listener = TcpListener::bind(http_addr)
        .map_err(|e| format!("cannot listen on {http_addr}: {e}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("cannot set nonblocking: {e}"))?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = shutdown.clone();
    std::thread::spawn(move || loop {
        if flag.load(Ordering::Relaxed) {
            break;
        }
        match listener.accept() {
            Ok((client, _)) => {
                std::thread::spawn(move || serve_conn(client, socks_addr));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    });
    Ok(HttpProxyHandle {
        shutdown,
        addr: http_addr.to_string(),
    })
}

fn read_exact(stream: &mut TcpStream, n: usize) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; n];
    stream
        .read_exact(&mut buf)
        .map_err(|e| format!("read failed: {e}"))?;
    Ok(buf)
}

/// Reads one HTTP request head (up to the blank line), with a timeout so a
/// half-open client can't park a thread forever.
fn read_head(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 1024];
    loop {
        let n = stream.read(&mut tmp).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("client closed connection".into());
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() > HEAD_LIMIT {
            return Err("request head too large".into());
        }
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    stream
        .set_read_timeout(None)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

/// SOCKS5 CONNECT through the tunnel. Returns a stream ready for relay.
fn socks5_connect(socks: SocketAddr, host: &str, port: u16) -> Result<TcpStream, String> {
    let mut s = TcpStream::connect_timeout(&socks, Duration::from_secs(10))
        .map_err(|e| format!("SOCKS unreachable: {e}"))?;
    s.set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|e| e.to_string())?;
    // Greeting: version 5, 1 method, no-auth.
    s.write_all(&[0x05, 0x01, 0x00])
        .map_err(|e| e.to_string())?;
    let resp = read_exact(&mut s, 2)?;
    if resp != [0x05, 0x00] {
        return Err("SOCKS5: no-auth rejected".into());
    }
    // Request: VER CMD RSV + ATYP + ADDR + PORT.
    let mut req = vec![0x05, 0x01, 0x00];
    if let Ok(v4) = host.parse::<std::net::Ipv4Addr>() {
        req.push(0x01);
        req.extend_from_slice(&v4.octets());
    } else if let Ok(v6) = host.parse::<std::net::Ipv6Addr>() {
        req.push(0x04);
        req.extend_from_slice(&v6.octets());
    } else {
        if host.len() > 255 {
            return Err("hostname too long".into());
        }
        req.push(0x03);
        req.push(host.len() as u8);
        req.extend_from_slice(host.as_bytes());
    }
    req.extend_from_slice(&port.to_be_bytes());
    s.write_all(&req).map_err(|e| e.to_string())?;
    // Reply: VER REP RSV ATYP + BND.ADDR + BND.PORT.
    let hdr = read_exact(&mut s, 4)?;
    if hdr[0] != 0x05 {
        return Err("SOCKS5: bad reply version".into());
    }
    if hdr[1] != 0x00 {
        let why = match hdr[1] {
            2 => "not allowed",
            3 => "network unreachable",
            4 => "host unreachable",
            5 => "connection refused",
            6 => "TTL expired",
            7 => "command not supported",
            8 => "address not supported",
            _ => "general failure",
        };
        return Err(format!("SOCKS5 connect failed: {why}"));
    }
    let skip = match hdr[3] {
        0x01 => 4 + 2,
        0x04 => 16 + 2,
        0x03 => {
            let l = read_exact(&mut s, 1)?;
            (l[0] as usize) + 2
        }
        _ => 0,
    };
    if skip > 0 {
        read_exact(&mut s, skip)?;
    }
    s.set_read_timeout(None).map_err(|e| e.to_string())?;
    Ok(s)
}

/// Splits "host:port" (IPv6 bracket-aware); falls back to `default_port`.
fn parse_authority(auth: &str, default_port: u16) -> (String, u16) {
    match auth.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && !h.ends_with(':') => match p.parse::<u16>() {
            Ok(port) => (h.trim_matches(|c| c == '[' || c == ']').to_string(), port),
            Err(_) => (auth.to_string(), default_port),
        },
        _ => (auth.to_string(), default_port),
    }
}

fn serve_conn(mut client: TcpStream, socks: SocketAddr) {
    let head = match read_head(&mut client) {
        Ok(h) => h,
        Err(_) => return,
    };
    let head_str = String::from_utf8_lossy(&head);
    let request_line = head_str.split("\r\n").next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let (method, target) = match (parts.next(), parts.next()) {
        (Some(m), Some(t)) => (m, t),
        _ => {
            let _ =
                client.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n");
            return;
        }
    };
    let is_connect = method.eq_ignore_ascii_case("CONNECT");
    // CONNECT carries authority-form ("host:port"); plain-HTTP through a
    // proxy carries absolute-form ("http://host/path").
    let authority: String = if is_connect {
        target.to_string()
    } else if let Some(rest) = target.strip_prefix("http://") {
        let end = rest.find('/').unwrap_or(rest.len());
        rest[..end].to_string()
    } else {
        let _ = client.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n");
        return;
    };
    let (host, port) = parse_authority(&authority, if is_connect { 443 } else { 80 });
    match socks5_connect(socks, &host, port) {
        Ok(server) => {
            if is_connect {
                if client
                    .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                    .is_err()
                {
                    return;
                }
            } else if client.write_all(&head).is_err() {
                // Absolute-URI form is valid HTTP to the origin server.
                return;
            }
            relay(client, server);
        }
        Err(_) => {
            let _ =
                client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n");
        }
    }
}

/// Bidirectional copy; whichever direction ends first shuts both sockets
/// down so the other direction can't idle forever.
fn relay(client: TcpStream, server: TcpStream) {
    use std::net::Shutdown;
    let c_up = match client.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    };
    let s_down = match server.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let t = std::thread::spawn(move || {
        let mut r = c_up;
        let mut w = server;
        let _ = std::io::copy(&mut r, &mut w);
        let _ = r.shutdown(Shutdown::Both);
        let _ = w.shutdown(Shutdown::Both);
    });
    {
        let mut r = s_down;
        let mut w = client;
        let _ = std::io::copy(&mut r, &mut w);
        let _ = r.shutdown(Shutdown::Both);
        let _ = w.shutdown(Shutdown::Both);
    }
    let _ = t.join();
}
