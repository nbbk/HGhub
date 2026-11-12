//! Server transport: direct, DNS-pinned, bounded, and credential-free by design.
use crate::network_policy;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use bytes::{Bytes, BytesMut};
use serde_json::{json, Value};
use std::time::Duration;
pub const DEFAULT_BASE: &str = "https://huangguoai.com";
pub fn base() -> String {
    std::env::var("HGHUB_WEB_UPSTREAM")
        .ok()
        .and_then(|s| normalize_base(&s))
        .unwrap_or_else(|| DEFAULT_BASE.into())
}
pub fn normalize_base(raw: &str) -> Option<String> {
    let u = network_policy::parse_target(raw).ok()?;
    (u.scheme() == "https" && u.path() == "/" && u.query().is_none() && u.fragment().is_none())
        .then(|| u.origin().ascii_serialization())
}
// Every request/hop pins all validated DNS answers. No proxy, automatic redirect
// or cached connection can bypass a fresh DNS-rebinding decision.
async fn client_for(raw: &str) -> Result<(url::Url, reqwest::Client), String> {
    let target = network_policy::parse_target(raw)?;
    let addrs = network_policy::resolve_target(&target, true).await?;
    let host = target.host_str().ok_or("目标缺少主机名")?;
    let http = reqwest::Client::builder()
        .no_proxy().no_gzip().no_brotli().no_deflate().no_zstd()
        .redirect(reqwest::redirect::Policy::none()).referer(false)
        .connect_timeout(Duration::from_secs(10)).read_timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .resolve_to_addrs(host, &addrs).build().map_err(|_| "无法创建 HTTP 客户端")?;
    Ok((target, http))
}
/// Only one RFC 7233 byte range. Reject multi-range and header injection.
pub fn valid_range(raw: &str) -> bool {
    if raw.len() > 64 {
        return false;
    }
    let Some(v) = raw.strip_prefix("bytes=") else {
        return false;
    };
    let Some((a, b)) = v.split_once('-') else {
        return false;
    };
    if a.is_empty() {
        return b.parse::<u64>().is_ok_and(|n| n > 0);
    }
    let Ok(start) = a.parse::<u64>() else {
        return false;
    };
    b.is_empty() || b.parse::<u64>().is_ok_and(|end| end >= start)
}
pub async fn get_range(
    raw: &str,
    range: Option<&str>,
    head: bool,
) -> Result<reqwest::Response, String> {
    if range.is_some_and(|r| !valid_range(r)) {
        return Err("无效的单段 Range".into());
    }
    let mut next = network_policy::parse_target(raw)?;
    for hop in 0..=5 {
        let (target, http) = client_for(next.as_str()).await?;
        let mut request = if head {
            http.head(target.clone())
        } else {
            http.get(target.clone())
        };
        request = request
            .header("Accept-Encoding", "identity")
            .header("Referer", format!("{}/", base()));
        if let Some(r) = range {
            request = request.header("Range", r);
        }
        let response = request.send().await.map_err(|_| "上游连接失败或超时")?;
        if !matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
            return Ok(response);
        }
        if hop == 5 {
            return Err("重定向次数超过限制".into());
        }
        let loc = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or("无效重定向")?;
        next = network_policy::redirect_target(&target, loc)?;
    }
    Err("重定向次数超过限制".into())
}
/// Enforce declared and actual byte bounds; transparent decompression disabled.
pub async fn bounded_bytes(response: reqwest::Response, max: usize) -> Result<Bytes, String> {
    bounded_remaining_bytes(response, max, 0).await
}
/// `consumed` accounts for bytes already inspected for an opaque HLS URL.
pub async fn bounded_remaining_bytes(
    mut response: reqwest::Response,
    max: usize,
    consumed: usize,
) -> Result<Bytes, String> {
    if response
        .content_length()
        .is_some_and(|n| n.saturating_sub(consumed as u64) > max as u64)
    {
        return Err("上游响应超过大小限制".into());
    }
    let mut buf = BytesMut::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "上游响应读取失败")? {
        if chunk.len() > max.saturating_sub(buf.len()) {
            return Err("上游响应超过大小限制".into());
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf.freeze())
}
pub async fn fetch_bounded(
    raw: &str,
    max: usize,
) -> Result<(String, Option<String>, Vec<u8>), String> {
    let response = get_range(raw, None, false).await?;
    if !response.status().is_success() {
        return Err(format!("上游 HTTP {}", response.status().as_u16()));
    }
    let final_url = response.url().to_string();
    let ctype = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = bounded_bytes(response, max).await?;
    Ok((final_url, ctype, bytes.to_vec()))
}
/// Same native edge envelope. Web v1 never transmits upstream account secrets.
pub async fn edge(
    origin: &str,
    method: &str,
    path: &str,
    body: Option<Value>,
    token: Option<&str>,
) -> Result<Value, String> {
    if token.is_some() || path.starts_with("/api/account/") {
        return Err("网页版仅支持游客浏览".into());
    }
    if !matches!(method, "GET" | "POST") || !path.starts_with("/api/") || path.len() > 4096 {
        return Err("不支持的接口".into());
    }
    let origin = normalize_base(origin).ok_or("上游必须是 HTTPS 站点")?;
    let envelope = json!({"method":method,"path":path,"body":body.as_ref().map(|v|B64.encode(serde_json::to_vec(v).unwrap())).unwrap_or_default(),"content_type":if body.is_some(){"application/json"}else{""}});
    let mut next = network_policy::parse_target(&format!("{origin}/api/edge/request"))?;
    for hop in 0..=5 {
        let (target, http) = client_for(next.as_str()).await?;
        let response = http
            .post(target.clone())
            .timeout(Duration::from_secs(30))
            .header("Accept", "application/json")
            .header("Accept-Encoding", "identity")
            .header("Referer", format!("{origin}/"))
            .json(&envelope)
            .send()
            .await
            .map_err(|_| "上游接口连接失败或超时")?;
        let status = response.status();
        if status.is_redirection() {
            if method != "GET"
                || body.is_some()
                || !matches!(status.as_u16(), 301 | 302 | 303)
                || hop == 5
            {
                return Err("业务接口重定向已拒绝".into());
            }
            let loc = response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or("无效重定向")?;
            next = network_policy::redirect_target(&target, loc)?;
            continue;
        }
        if !status.is_success() {
            return Err(format!("上游 HTTP {}", status.as_u16()));
        }
        let bytes = bounded_bytes(response, 8 * 1024 * 1024).await?;
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| "上游未返回有效 JSON")?;
        if value["status"].as_i64() == Some(1) {
            return Ok(value.get("data").cloned().unwrap_or(Value::Null));
        }
        return Err(value["msg"]
            .as_str()
            .unwrap_or("接口返回失败")
            .chars()
            .take(200)
            .collect());
    }
    Err("重定向次数超过限制".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_is_strict_and_single() {
        for r in ["bytes=0-1", "bytes=0-", "bytes=-100"] {
            assert!(valid_range(r));
        }
        for r in [
            "bytes=",
            "bytes=-",
            "bytes=2-1",
            "bytes=0-1,5-9",
            "bytes=0-1\r\nx:1",
            "bytes=-0",
            "items=0-1",
        ] {
            assert!(!valid_range(r), "{r}");
        }
    }
    #[tokio::test]
    async fn private_targets_fail_before_request() {
        for u in [
            "http://127.0.0.1/",
            "http://169.254.169.254/",
            "http://198.18.0.1/",
        ] {
            assert!(get_range(u, None, false).await.is_err());
        }
    }
    fn fixture_response(raw: &'static str) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let task = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0u8; 4096];
            let _ = socket.read(&mut request);
            let _ = socket.write_all(raw.as_bytes());
        });
        (format!("http://{addr}/"), task)
    }
    #[tokio::test]
    async fn actual_and_declared_lengths_are_bounded() {
        // Decoder unit fixture only. Production client_for rejects this address.
        for wire in [
            "HTTP/1.1 200 OK\r\nContent-Length: 999999\r\n\r\n",
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n6\r\n123456\r\n0\r\n\r\n",
        ] {
            let (url, thread) = fixture_response(wire);
            let response = reqwest::Client::builder()
                .no_proxy()
                .build()
                .unwrap()
                .get(url)
                .send()
                .await
                .unwrap();
            assert!(bounded_bytes(response, 5).await.is_err());
            thread.join().unwrap();
        }
        let (url, thread) = fixture_response("HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n12345");
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(url)
            .send()
            .await
            .unwrap();
        assert_eq!(&bounded_bytes(response, 5).await.unwrap()[..], b"12345");
        thread.join().unwrap();
    }
    #[tokio::test]
    async fn already_peeked_bytes_are_not_counted_twice() {
        let (url, thread) = fixture_response("HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n12345");
        let mut response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(url)
            .send()
            .await
            .unwrap();
        let prefix = response.chunk().await.unwrap().unwrap();
        let tail = bounded_remaining_bytes(response, 5 - prefix.len(), prefix.len())
            .await
            .unwrap();
        assert_eq!(prefix.len() + tail.len(), 5);
        thread.join().unwrap();
    }
    #[tokio::test]
    #[ignore = "read-only live upstream availability; requires internet"]
    async fn live_guest_feed_and_media() {
        let feed = edge(&base(), "GET", "/api/videos?page=1&size=1", None, None)
            .await
            .expect("guest feed");
        let item = feed["items"]
            .as_array()
            .and_then(|a| a.first())
            .expect("feed item");
        let id = item["id"].as_i64().expect("video id");
        let play = edge(
            &base(),
            "POST",
            &format!("/api/videos/{id}/episodes/1/playback"),
            Some(json!({})),
            None,
        )
        .await
        .expect("playback metadata");
        let url = play["streams"]["main"]
            .as_str()
            .or_else(|| play["video_url"].as_str())
            .expect("play URL");
        let playlist = crate::media::load_playlist(url, "/media")
            .await
            .expect("playlist");
        assert!(playlist.starts_with("#EXTM3U"));
        assert!(playlist.contains("/media/seg?u="));
        eprintln!("live guest feed, playback metadata and playlist rewrite succeeded");
    }
}
