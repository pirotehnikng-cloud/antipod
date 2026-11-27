use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::CoreError;

pub const DEFAULT_MOSTLOGIN_API_URL: &str = "http://127.0.0.1:30898";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MostLoginProfileInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MostLoginLaunchInfo {
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub ws: Option<String>,
}

pub fn open_mostlogin_profile(
    api_url: Option<&str>,
    api_token: Option<&str>,
    profile_id: &str,
    url: Option<&str>,
) -> Result<MostLoginLaunchInfo, CoreError> {
    let id = profile_id.trim();
    if id.is_empty() {
        return Err(CoreError::Config(
            "Не указан MostLogin profileId для профиля".to_string(),
        ));
    }

    let mut payload = serde_json::json!({
        "profileId": id,
        "ignoreStartUrls": false,
    });
    if let Some(u) = url.map(str::trim).filter(|s| !s.is_empty()) {
        payload["urls"] = serde_json::json!([u]);
    }

    let resp = http_post_json(api_url, api_token, "/api/browser/openBrowser", &payload)?;
    let node = resp
        .get("data")
        .filter(|v| v.is_object())
        .unwrap_or(&resp);

    let pid = node
        .get("pid")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok());
    let ws = node
        .get("ws")
        .or_else(|| node.get("cdpUrl"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let http_url = node
        .get("http")
        .or_else(|| node.get("httpUrl"))
        .and_then(Value::as_str);

    let port = node
        .get("port")
        .and_then(Value::as_u64)
        .and_then(|v| u16::try_from(v).ok())
        .or_else(|| http_url.and_then(extract_port_from_url))
        .or_else(|| ws.as_deref().and_then(extract_port_from_url));

    Ok(MostLoginLaunchInfo { pid, port, ws })
}

pub fn close_mostlogin_profile(
    api_url: Option<&str>,
    api_token: Option<&str>,
    profile_id: &str,
) -> Result<(), CoreError> {
    let id = profile_id.trim();
    if id.is_empty() {
        return Ok(());
    }

    let payload = serde_json::json!({
        "profileIds": [id],
    });
    let _ = http_post_json(api_url, api_token, "/api/browser/closeProfiles", &payload)?;
    Ok(())
}

pub fn list_mostlogin_profiles(
    api_url: Option<&str>,
    api_token: Option<&str>,
) -> Result<Vec<MostLoginProfileInfo>, CoreError> {
    let payload = serde_json::json!({
        "page": 1,
        "pageSize": 100,
    });
    let resp = http_post_json(api_url, api_token, "/api/profile/getProfiles", &payload)?;
    Ok(extract_profiles_from_json(&resp))
}

pub fn open_url_via_cdp(port: u16, url: &str) -> Result<(), CoreError> {
    let clean_url = url.trim().replace(' ', "%20");
    if clean_url.is_empty() || clean_url.contains('\r') || clean_url.contains('\n') {
        return Err(CoreError::Config(
            "Некорректный URL для открытия через CDP".to_string(),
        ));
    }

    let code = send_cdp_new_tab("PUT", port, &clean_url)?;
    if (200..300).contains(&code) {
        return Ok(());
    }
    if code == 405 {
        let fallback_code = send_cdp_new_tab("GET", port, &clean_url)?;
        if (200..300).contains(&fallback_code) {
            return Ok(());
        }
        return Err(CoreError::Process(format!(
            "CDP /json/new вернул HTTP {fallback_code}"
        )));
    }
    Err(CoreError::Process(format!(
        "CDP /json/new вернул HTTP {code}"
    )))
}

fn send_cdp_new_tab(method: &str, port: u16, clean_url: &str) -> Result<u16, CoreError> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .map_err(|e| CoreError::Process(format!("CDP порт {port} недоступен: {e}")))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let req = format!(
        "{method} /json/new?{clean_url} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| CoreError::Process(format!("Ошибка записи в CDP порт {port}: {e}")))?;

    let mut buf = [0u8; 256];
    let n = stream
        .read(&mut buf)
        .map_err(|e| CoreError::Process(format!("Ошибка чтения ответа CDP ({port}): {e}")))?;
    let head = String::from_utf8_lossy(&buf[..n]);
    let status = head
        .split("\r\n")
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .unwrap_or(0);
    Ok(status)
}

fn extract_port_from_url(raw: &str) -> Option<u16> {
    let rest = raw
        .trim()
        .split_once("://")
        .map(|(_, r)| r)
        .unwrap_or(raw.trim());
    let authority = rest.split('/').next()?;
    let (_, port_str) = authority.rsplit_once(':')?;
    port_str.parse::<u16>().ok().filter(|&p| p > 0)
}

fn http_post_json(
    api_url: Option<&str>,
    api_token: Option<&str>,
    endpoint: &str,
    payload: &Value,
) -> Result<Value, CoreError> {
    let base = api_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_MOSTLOGIN_API_URL);
    let (host, port, base_path) = parse_api_base(base)?;
    let full_path = format!("{base_path}{endpoint}");

    let body_bytes = serde_json::to_vec(payload)?;
    let mut stream = connect_local(&host, port)?;

    let mut req = format!(
        "POST {full_path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body_bytes.len()
    );
    if let Some(tok) = api_token.map(str::trim).filter(|s| !s.is_empty()) {
        req.push_str(&format!("Authorization: {tok}\r\n"));
    }
    req.push_str("\r\n");

    stream
        .write_all(req.as_bytes())
        .and_then(|()| stream.write_all(&body_bytes))
        .map_err(|e| {
            CoreError::Process(format!("Ошибка отправки запроса в MostLogin API: {e}"))
        })?;

    let mut raw = Vec::new();
    stream
        .take(2 * 1024 * 1024)
        .read_to_end(&mut raw)
        .map_err(|e| {
            CoreError::Process(format!("Нет ответа от MostLogin API ({base}): {e}"))
        })?;

    let split_pos = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| {
            CoreError::Process("MostLogin API вернул некорректный HTTP-ответ".to_string())
        })?;

    let headers_str = String::from_utf8_lossy(&raw[..split_pos]);
    let raw_body = &raw[split_pos + 4..];

    let status_code = headers_str
        .split("\r\n")
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .unwrap_or(0);

    let is_chunked = headers_str.split("\r\n").skip(1).any(|line| {
        line.split_once(':').is_some_and(|(k, v)| {
            k.trim().eq_ignore_ascii_case("transfer-encoding")
                && v.to_ascii_lowercase().contains("chunked")
        })
    });

    let decoded_body = if is_chunked {
        decode_chunked(raw_body).unwrap_or_else(|| raw_body.to_vec())
    } else {
        raw_body.to_vec()
    };

    let body_text = String::from_utf8_lossy(&decoded_body).trim().to_string();
    let parsed_json: Value = if body_text.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&body_text).unwrap_or(Value::Null)
    };

    if !(200..300).contains(&status_code) {
        let detail = extract_error_message(&parsed_json)
            .or_else(|| (!body_text.is_empty()).then_some(body_text))
            .unwrap_or_else(|| format!("HTTP {status_code}"));
        return Err(CoreError::Process(format!(
            "MostLogin API вернул HTTP {status_code}: {detail}"
        )));
    }

    if let Some(err_msg) = check_logical_api_error(&parsed_json) {
        return Err(CoreError::Process(format!(
            "MostLogin API отклонил запрос: {err_msg}"
        )));
    }

    Ok(parsed_json)
}

fn connect_local(host: &str, port: u16) -> Result<TcpStream, CoreError> {
    let connect_timeout = Duration::from_secs(5);
    let addrs: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|e| {
            CoreError::Config(format!(
                "Некорректный адрес MostLogin API ({host}:{port}): {e}"
            ))
        })?
        .collect();

    let mut last_err = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, connect_timeout) {
            Ok(stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
            Err(e) => last_err = Some(e),
        }
    }

    let detail = last_err
        .map(|e| e.to_string())
        .unwrap_or_else(|| "адрес недоступен".to_string());
    Err(CoreError::Process(format!(
        "Не удалось подключиться к MostLogin Local API ({host}:{port}). Убедись, что клиент MostLogin запущен и Local API включён ({detail})"
    )))
}

fn parse_api_base(input: &str) -> Result<(String, u16, String), CoreError> {
    let trimmed = input.trim().trim_end_matches('/');
    let rest = if let Some(r) = trimmed.strip_prefix("http://") {
        r
    } else if trimmed.starts_with("https://") {
        return Err(CoreError::Config(
            "MostLogin Local API работает по http:// (например, http://127.0.0.1:30898)"
                .to_string(),
        ));
    } else {
        trimmed
    };

    let (authority, base_path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], rest[idx..].trim_end_matches('/').to_string()),
        None => (rest, String::new()),
    };

    let (host, port) = if let Some((h, p)) = authority.rsplit_once(':') {
        let parsed_port = p
            .parse::<u16>()
            .map_err(|_| CoreError::Config(format!("Некорректный порт MostLogin API: {p}")))?;
        (h.to_string(), parsed_port)
    } else {
        (authority.to_string(), 30898)
    };

    if host.is_empty() || port == 0 {
        return Err(CoreError::Config(format!(
            "Некорректный URL MostLogin API: {input}"
        )));
    }

    Ok((host, port, base_path))
}

fn decode_chunked(input: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;

    while i < input.len() {
        let rel_end = input[i..].windows(2).position(|w| w == b"\r\n")?;
        let size_line = std::str::from_utf8(&input[i..i + rel_end]).ok()?;
        let size_hex = size_line.split(';').next()?.trim();
        let chunk_len = usize::from_str_radix(size_hex, 16).ok()?;
        i += rel_end + 2;

        if chunk_len == 0 {
            return Some(out);
        }
        if i + chunk_len > input.len() {
            return None;
        }
        out.extend_from_slice(&input[i..i + chunk_len]);
        i += chunk_len;

        if input.get(i..i + 2) == Some(b"\r\n") {
            i += 2;
        }
    }

    Some(out)
}

fn extract_error_message(val: &Value) -> Option<String> {
    for key in ["message", "msg", "error"] {
        if let Some(s) = val
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
        {
            return Some(s.to_string());
        }
    }
    None
}

fn check_logical_api_error(val: &Value) -> Option<String> {
    if val.get("success").and_then(Value::as_bool) == Some(false) {
        return Some(
            extract_error_message(val).unwrap_or_else(|| "операция не выполнена".to_string()),
        );
    }
    if let Some(code) = val.get("code").and_then(Value::as_i64) {
        if code != 0 && code != 200 {
            let msg = extract_error_message(val).unwrap_or_else(|| format!("код ошибки {code}"));
            return Some(msg);
        }
    }
    None
}

fn extract_profiles_from_json(val: &Value) -> Vec<MostLoginProfileInfo> {
    let candidates = [
        val.get("data").and_then(|d| d.get("list")),
        val.get("data").and_then(|d| d.get("records")),
        val.get("data").and_then(|d| d.get("items")),
        val.get("data").and_then(|d| d.get("profiles")),
        val.get("list"),
        val.get("records"),
        val.get("profiles"),
        val.get("data"),
        Some(val),
    ];

    let Some(arr) = candidates.into_iter().flatten().find_map(Value::as_array) else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|item| {
            let id = item
                .get("profileId")
                .or_else(|| item.get("id"))
                .or_else(|| item.get("profile_id"))
                .and_then(|v| {
                    v.as_str()
                        .map(ToString::to_string)
                        .or_else(|| v.as_i64().map(|n| n.to_string()))
                })?;

            let name = item
                .get("name")
                .or_else(|| item.get("profileName"))
                .or_else(|| item.get("title"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| id.clone());

            Some(MostLoginProfileInfo { id, name })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_api_base_url() {
        let (h, p, path) = parse_api_base("http://127.0.0.1:30898/").expect("base");
        assert_eq!(h, "127.0.0.1");
        assert_eq!(p, 30898);
        assert_eq!(path, "");

        let (h2, p2, _) = parse_api_base("127.0.0.1:31000").expect("short");
        assert_eq!(h2, "127.0.0.1");
        assert_eq!(p2, 31000);
    }

    #[test]
    fn extracts_cdp_port_from_urls() {
        assert_eq!(
            extract_port_from_url("ws://127.0.0.1:9222/devtools/browser/abc"),
            Some(9222)
        );
        assert_eq!(extract_port_from_url("http://127.0.0.1:41234"), Some(41234));
        assert_eq!(extract_port_from_url("http://127.0.0.1"), None);
    }

    #[test]
    fn decodes_chunked_transfer() {
        let raw = b"7\r\n{\"pid\":\r\n6\r\n12345}\r\n0\r\n\r\n";
        let out = decode_chunked(raw).expect("chunked");
        assert_eq!(out, b"{\"pid\":12345}");
    }

    #[test]
    fn extracts_mostlogin_profiles_list() {
        let sample = serde_json::json!({
            "code": 0,
            "data": {
                "list": [
                    { "profileId": "d878e732552a422e87942b2a4938265c", "name": "Google US 1" },
                    { "id": "abc123", "profileName": "Work EU" }
                ]
            }
        });
        let items = extract_profiles_from_json(&sample);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "d878e732552a422e87942b2a4938265c");
        assert_eq!(items[0].name, "Google US 1");
        assert_eq!(items[1].id, "abc123");
        assert_eq!(items[1].name, "Work EU");
    }
}
