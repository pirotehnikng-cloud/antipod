use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::CoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyScheme {
    Http,
    Socks5,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    pub scheme: ProxyScheme,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl ProxyConfig {
    pub fn parse(raw: &str) -> Result<Self, CoreError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(CoreError::Config("Адрес прокси не указан".to_string()));
        }

        // Поддержка короткого формата ip:port:user:pass (по умолчанию HTTP)
        if !trimmed.contains("://") && !trimmed.contains('@') && !trimmed.contains('[') {
            let parts: Vec<&str> = trimmed.splitn(4, ':').collect();
            if parts.len() == 4 && !parts[0].is_empty() {
                if let Ok(port) = parts[1].parse::<u16>() {
                    if port > 0 {
                        return Ok(Self {
                            scheme: ProxyScheme::Http,
                            host: parts[0].to_string(),
                            port,
                            username: Some(parts[2].to_string()),
                            password: Some(parts[3].to_string()),
                        });
                    }
                }
            }
        }

        let (scheme, rest) = if let Some((s, r)) = trimmed.split_once("://") {
            let parsed_scheme = match s.to_ascii_lowercase().as_str() {
                "http" | "https" => ProxyScheme::Http,
                "socks5" | "socks5h" => ProxyScheme::Socks5,
                other => {
                    return Err(CoreError::Config(format!(
                        "Неподдерживаемый протокол прокси '{other}'. Используй http:// или socks5://"
                    )));
                }
            };
            (parsed_scheme, r)
        } else {
            (ProxyScheme::Http, trimmed)
        };

        let (username, password, host_section) = match rest.rsplit_once('@') {
            Some((creds, host_part)) => {
                let (u, p) = match creds.split_once(':') {
                    Some((u, p)) => (percent_decode(u), percent_decode(p)),
                    None => (percent_decode(creds), String::new()),
                };
                (Some(u), Some(p), host_part)
            }
            None => (None, None, rest),
        };

        let clean_authority = host_section.split('/').next().unwrap_or("").trim();
        let (host, port) = parse_host_port(clean_authority, None)?;

        Ok(Self {
            scheme,
            host,
            port,
            username,
            password,
        })
    }

    pub fn has_auth(&self) -> bool {
        self.username.as_deref().is_some_and(|u| !u.is_empty())
    }

    pub fn masked_display(&self) -> String {
        let scheme = match self.scheme {
            ProxyScheme::Http => "http",
            ProxyScheme::Socks5 => "socks5",
        };
        if self.has_auth() {
            format!("{scheme}://{}:{} (auth)", self.host, self.port)
        } else {
            format!("{scheme}://{}:{}", self.host, self.port)
        }
    }

    pub fn direct_url(&self) -> String {
        let scheme = match self.scheme {
            ProxyScheme::Http => "http",
            ProxyScheme::Socks5 => "socks5",
        };
        format!("{scheme}://{}:{}", self.host, self.port)
    }

    fn basic_auth_header(&self) -> Option<String> {
        let user = self.username.as_deref()?;
        if user.is_empty() {
            return None;
        }
        let pass = self.password.as_deref().unwrap_or("");
        let creds = format!("{user}:{pass}");
        Some(format!("Basic {}", base64_encode(creds.as_bytes())))
    }
}

pub fn mask_proxy_str(raw: Option<&str>) -> Option<String> {
    let s = raw?.trim();
    if s.is_empty() {
        return None;
    }
    match ProxyConfig::parse(s) {
        Ok(cfg) => Some(cfg.masked_display()),
        Err(_) => Some("ошибка формата прокси".to_string()),
    }
}

pub struct ProxyBridge {
    local_addr: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ProxyBridge {
    pub fn start(upstream: ProxyConfig) -> Result<Self, CoreError> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| CoreError::Isolation(format!("Не удалось открыть порт локального моста: {e}")))?;
        let local_addr = listener
            .local_addr()
            .map_err(|e| CoreError::Isolation(format!("Не удалось получить адрес моста: {e}")))?;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let upstream_arc = Arc::new(upstream);

        let handle = thread::Builder::new()
            .name(format!("antipod-proxy-{}", local_addr.port()))
            .spawn(move || {
                for incoming in listener.incoming() {
                    if stop_flag.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(stream) = incoming {
                        if stop_flag.load(Ordering::SeqCst) {
                            break;
                        }
                        let cfg = Arc::clone(&upstream_arc);
                        let _ = thread::spawn(move || {
                            let _ = handle_client(stream, &cfg);
                        });
                    }
                }
            })
            .map_err(|e| CoreError::Isolation(format!("Не удалось запустить поток прокси-моста: {e}")))?;

        Ok(Self {
            local_addr,
            stop,
            handle: Some(handle),
        })
    }

    pub fn local_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.local_addr.port())
    }
}

impl Drop for ProxyBridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&self.local_addr, Duration::from_millis(200));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn handle_client(mut client: TcpStream, upstream: &ProxyConfig) -> io::Result<()> {
    let handshake_timeout = Duration::from_secs(15);
    let _ = client.set_read_timeout(Some(handshake_timeout));
    let _ = client.set_write_timeout(Some(handshake_timeout));
    let _ = client.set_nodelay(true);

    let (head, leftover) = read_http_head(&mut client)?;
    let head_str = String::from_utf8_lossy(&head);
    let first_line = head_str
        .split("\r\n")
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty request line"))?;

    let mut parts = first_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing HTTP method"))?;
    let target = parts
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request target"))?;
    let version = parts.next().unwrap_or("HTTP/1.1");

    if method.eq_ignore_ascii_case("CONNECT") {
        handle_connect(client, target, &leftover, upstream)
    } else {
        handle_plain_http(client, method, target, version, &head_str, &leftover, upstream)
    }
}

fn handle_connect(
    mut client: TcpStream,
    target: &str,
    client_leftover: &[u8],
    upstream: &ProxyConfig,
) -> io::Result<()> {
    let (target_host, target_port) = parse_host_port(target, Some(443))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    match upstream.scheme {
        ProxyScheme::Http => {
            let mut up = match connect_upstream(upstream) {
                Ok(s) => s,
                Err(e) => {
                    let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n");
                    return Err(e);
                }
            };

            let mut req = format!(
                "CONNECT {target} HTTP/1.1\r\nHost: {target}\r\nProxy-Connection: Keep-Alive\r\n"
            );
            if let Some(auth) = upstream.basic_auth_header() {
                req.push_str(&format!("Proxy-Authorization: {auth}\r\n"));
            }
            req.push_str("\r\n");
            up.write_all(req.as_bytes())?;

            let (resp_head, resp_leftover) = read_http_head(&mut up)?;
            client.write_all(&resp_head)?;

            let resp_first = String::from_utf8_lossy(&resp_head);
            let is_ok = resp_first
                .split("\r\n")
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .is_some_and(|code| code == "200");

            if !is_ok {
                return Ok(());
            }

            clear_timeouts(&client, &up);
            tunnel_bidirectional(client, up, client_leftover, &resp_leftover)
        }
        ProxyScheme::Socks5 => {
            let up = match connect_socks5(upstream, &target_host, target_port) {
                Ok(s) => s,
                Err(e) => {
                    let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n");
                    return Err(e);
                }
            };

            client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")?;
            clear_timeouts(&client, &up);
            tunnel_bidirectional(client, up, client_leftover, &[])
        }
    }
}

fn handle_plain_http(
    mut client: TcpStream,
    method: &str,
    target: &str,
    version: &str,
    head_str: &str,
    client_leftover: &[u8],
    upstream: &ProxyConfig,
) -> io::Result<()> {
    match upstream.scheme {
        ProxyScheme::Http => {
            let mut up = match connect_upstream(upstream) {
                Ok(s) => s,
                Err(e) => {
                    let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n");
                    return Err(e);
                }
            };

            let mut rebuilt = format!("{method} {target} {version}\r\n");
            for line in head_str.split("\r\n").skip(1) {
                if line.is_empty() {
                    break;
                }
                if is_hop_by_hop_or_proxy_header(line) {
                    continue;
                }
                rebuilt.push_str(line);
                rebuilt.push_str("\r\n");
            }
            if let Some(auth) = upstream.basic_auth_header() {
                rebuilt.push_str(&format!("Proxy-Authorization: {auth}\r\n"));
            }
            rebuilt.push_str("Connection: close\r\nProxy-Connection: close\r\n\r\n");

            up.write_all(rebuilt.as_bytes())?;
            clear_timeouts(&client, &up);
            tunnel_bidirectional(client, up, client_leftover, &[])
        }
        ProxyScheme::Socks5 => {
            let (host, port, path, authority) = split_http_url(target, head_str)?;
            let mut up = match connect_socks5(upstream, &host, port) {
                Ok(s) => s,
                Err(e) => {
                    let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n");
                    return Err(e);
                }
            };

            let mut rebuilt = format!("{method} {path} {version}\r\n");
            let mut has_host = false;
            for line in head_str.split("\r\n").skip(1) {
                if line.is_empty() {
                    break;
                }
                if is_hop_by_hop_or_proxy_header(line) {
                    continue;
                }
                if line
                    .split_once(':')
                    .is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case("host"))
                {
                    has_host = true;
                }
                rebuilt.push_str(line);
                rebuilt.push_str("\r\n");
            }
            if !has_host {
                rebuilt.push_str(&format!("Host: {authority}\r\n"));
            }
            rebuilt.push_str("Connection: close\r\n\r\n");

            up.write_all(rebuilt.as_bytes())?;
            clear_timeouts(&client, &up);
            tunnel_bidirectional(client, up, client_leftover, &[])
        }
    }
}

fn is_hop_by_hop_or_proxy_header(line: &str) -> bool {
    let Some((name, _)) = line.split_once(':') else {
        return false;
    };
    let n = name.trim();
    n.eq_ignore_ascii_case("proxy-authorization")
        || n.eq_ignore_ascii_case("proxy-connection")
        || n.eq_ignore_ascii_case("connection")
}

fn connect_upstream(upstream: &ProxyConfig) -> io::Result<TcpStream> {
    let timeout = Duration::from_secs(15);
    let addrs: Vec<SocketAddr> = (upstream.host.as_str(), upstream.port)
        .to_socket_addrs()?
        .collect();

    let mut last_err = io::Error::new(
        io::ErrorKind::AddrNotAvailable,
        "не удалось разрешить адрес прокси",
    );
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => {
                let _ = stream.set_read_timeout(Some(timeout));
                let _ = stream.set_write_timeout(Some(timeout));
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

fn connect_socks5(
    upstream: &ProxyConfig,
    target_host: &str,
    target_port: u16,
) -> io::Result<TcpStream> {
    let mut stream = connect_upstream(upstream)?;

    // 1. Приветствие SOCKS5 (RFC 1928)
    if upstream.has_auth() {
        stream.write_all(&[0x05, 0x02, 0x00, 0x02])?;
    } else {
        stream.write_all(&[0x05, 0x01, 0x00])?;
    }

    let mut method_resp = [0u8; 2];
    stream.read_exact(&mut method_resp)?;
    if method_resp[0] != 0x05 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "некорректная версия SOCKS5",
        ));
    }

    // 2. Авторизация логин/пароль (RFC 1929)
    match method_resp[1] {
        0x00 => {}
        0x02 => {
            let user = upstream.username.as_deref().unwrap_or("").as_bytes();
            let pass = upstream.password.as_deref().unwrap_or("").as_bytes();
            if user.len() > 255 || pass.len() > 255 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "логин или пароль SOCKS5 длиннее 255 байт",
                ));
            }
            let mut auth_req = Vec::with_capacity(3 + user.len() + pass.len());
            auth_req.push(0x01);
            auth_req.push(user.len() as u8);
            auth_req.extend_from_slice(user);
            auth_req.push(pass.len() as u8);
            auth_req.extend_from_slice(pass);
            stream.write_all(&auth_req)?;

            let mut auth_resp = [0u8; 2];
            stream.read_exact(&mut auth_resp)?;
            if auth_resp[1] != 0x00 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "SOCKS5 отклонил логин или пароль",
                ));
            }
        }
        0xFF => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "SOCKS5 не принял метод авторизации",
            ));
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("неподдерживаемый метод SOCKS5: {other}"),
            ));
        }
    }

    // 3. Команда CONNECT (удалённое разрешение домена для защиты от утечек DNS)
    let mut req = Vec::with_capacity(64);
    req.extend_from_slice(&[0x05, 0x01, 0x00]);

    let clean_host = target_host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(v4) = clean_host.parse::<Ipv4Addr>() {
        req.push(0x01);
        req.extend_from_slice(&v4.octets());
    } else if let Ok(v6) = clean_host.parse::<Ipv6Addr>() {
        req.push(0x04);
        req.extend_from_slice(&v6.octets());
    } else {
        let domain = clean_host.as_bytes();
        if domain.len() > 255 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "имя хоста длиннее 255 байт",
            ));
        }
        req.push(0x03);
        req.push(domain.len() as u8);
        req.extend_from_slice(domain);
    }
    req.extend_from_slice(&target_port.to_be_bytes());
    stream.write_all(&req)?;

    // 4. Чтение ответа SOCKS5
    let mut hdr = [0u8; 4];
    stream.read_exact(&mut hdr)?;
    if hdr[0] != 0x05 || hdr[1] != 0x00 {
        return Err(io::Error::other(format!(
            "SOCKS5 CONNECT отклонён с кодом {}",
            hdr[1]
        )));
    }

    match hdr[3] {
        0x01 => {
            let mut skip = [0u8; 6];
            stream.read_exact(&mut skip)?;
        }
        0x04 => {
            let mut skip = [0u8; 18];
            stream.read_exact(&mut skip)?;
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len)?;
            let mut skip = vec![0u8; len[0] as usize + 2];
            stream.read_exact(&mut skip)?;
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "неизвестный тип адреса в ответе SOCKS5",
            ));
        }
    }

    Ok(stream)
}

fn read_http_head(stream: &mut TcpStream) -> io::Result<(Vec<u8>, Vec<u8>)> {
    const MAX_HEAD_BYTES: usize = 64 * 1024;
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];

    loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let leftover = buf.split_off(pos + 4);
            return Ok((buf, leftover));
        }
        if buf.len() >= MAX_HEAD_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HTTP заголовки превышают лимит 64 КБ",
            ));
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "соединение закрыто при чтении HTTP заголовков",
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

fn clear_timeouts(a: &TcpStream, b: &TcpStream) {
    let _ = a.set_read_timeout(None);
    let _ = a.set_write_timeout(None);
    let _ = b.set_read_timeout(None);
    let _ = b.set_write_timeout(None);
}

fn tunnel_bidirectional(
    mut client: TcpStream,
    mut upstream: TcpStream,
    client_leftover: &[u8],
    upstream_leftover: &[u8],
) -> io::Result<()> {
    if !client_leftover.is_empty() {
        upstream.write_all(client_leftover)?;
    }
    if !upstream_leftover.is_empty() {
        client.write_all(upstream_leftover)?;
    }

    let mut client_read = client.try_clone()?;
    let mut upstream_write = upstream.try_clone()?;

    let c2u = thread::spawn(move || {
        let _ = io::copy(&mut client_read, &mut upstream_write);
        let _ = upstream_write.shutdown(Shutdown::Write);
    });

    let _ = io::copy(&mut upstream, &mut client);
    let _ = client.shutdown(Shutdown::Write);
    let _ = c2u.join();
    Ok(())
}

fn split_http_url(target: &str, head_str: &str) -> io::Result<(String, u16, String, String)> {
    if target.starts_with('/') {
        let host_val = head_str
            .split("\r\n")
            .skip(1)
            .find_map(|line| {
                let (k, v) = line.split_once(':')?;
                if k.trim().eq_ignore_ascii_case("host") {
                    Some(v.trim().to_string())
                } else {
                    None
                }
            })
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "отсутствует заголовок Host"))?;
        let (host, port) = parse_host_port(&host_val, Some(80))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        return Ok((host, port, target.to_string(), host_val));
    }

    let without_scheme = if target.get(..7).is_some_and(|s| s.eq_ignore_ascii_case("http://")) {
        &target[7..]
    } else if target.get(..8).is_some_and(|s| s.eq_ignore_ascii_case("https://")) {
        &target[8..]
    } else {
        target
    };

    let (authority, path) = match without_scheme.find('/') {
        Some(idx) => (&without_scheme[..idx], without_scheme[idx..].to_string()),
        None => (without_scheme, "/".to_string()),
    };

    let (host, port) = parse_host_port(authority, Some(80))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    Ok((host, port, path, authority.to_string()))
}

fn parse_host_port(input: &str, default_port: Option<u16>) -> Result<(String, u16), CoreError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(CoreError::Config("Пустой хост прокси".to_string()));
    }

    let (host, port) = if let Some(rest) = s.strip_prefix('[') {
        let (h, tail) = rest
            .split_once(']')
            .ok_or_else(|| CoreError::Config(format!("Некорректный IPv6 адрес: {s}")))?;
        let p = if let Some(port_str) = tail.strip_prefix(':') {
            port_str
                .parse::<u16>()
                .map_err(|_| CoreError::Config(format!("Некорректный порт прокси: {port_str}")))?
        } else {
            default_port.ok_or_else(|| CoreError::Config(format!("Не указан порт прокси: {s}")))?
        };
        (h.to_string(), p)
    } else if let Some((h, port_str)) = s.rsplit_once(':') {
        let p = port_str
            .parse::<u16>()
            .map_err(|_| CoreError::Config(format!("Некорректный порт прокси: {port_str}")))?;
        (h.to_string(), p)
    } else if let Some(p) = default_port {
        (s.to_string(), p)
    } else {
        return Err(CoreError::Config(format!(
            "Укажи порт прокси в формате host:port (получено: {s})"
        )));
    };

    if host.is_empty() || port == 0 {
        return Err(CoreError::Config(format!("Некорректный адрес прокси: {s}")));
    }
    Ok((host, port))
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok());
            if let Some(val) = hex {
                out.push(val);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_http_and_socks5_variants() {
        let p1 = ProxyConfig::parse("http://alice:p%40ss@1.2.3.4:8080").expect("http auth");
        assert_eq!(p1.scheme, ProxyScheme::Http);
        assert_eq!(p1.host, "1.2.3.4");
        assert_eq!(p1.port, 8080);
        assert_eq!(p1.username.as_deref(), Some("alice"));
        assert_eq!(p1.password.as_deref(), Some("p@ss"));
        assert_eq!(p1.masked_display(), "http://1.2.3.4:8080 (auth)");

        let p2 = ProxyConfig::parse("socks5://bob:secret@proxy.example.com:1080").expect("socks5");
        assert_eq!(p2.scheme, ProxyScheme::Socks5);
        assert_eq!(p2.port, 1080);
        assert_eq!(p2.masked_display(), "socks5://proxy.example.com:1080 (auth)");

        let p3 = ProxyConfig::parse("10.0.0.5:3128:user1:pass:2").expect("shorthand");
        assert_eq!(p3.scheme, ProxyScheme::Http);
        assert_eq!(p3.host, "10.0.0.5");
        assert_eq!(p3.port, 3128);
        assert_eq!(p3.username.as_deref(), Some("user1"));
        assert_eq!(p3.password.as_deref(), Some("pass:2"));
    }

    #[test]
    fn base64_matches_standard() {
        assert_eq!(base64_encode(b"user:pass"), "dXNlcjpwYXNz");
        assert_eq!(base64_encode(b"a"), "YQ==");
        assert_eq!(base64_encode(b"ab"), "YWI=");
    }

    #[test]
    fn local_bridge_starts_and_drops() {
        let cfg = ProxyConfig::parse("http://127.0.0.1:9999").expect("parse");
        let bridge = ProxyBridge::start(cfg).expect("start bridge");
        assert!(bridge.local_url().starts_with("http://127.0.0.1:"));
        drop(bridge);
    }
}
