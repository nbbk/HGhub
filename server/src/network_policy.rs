//! Server-only network policy; no relaxed mode or FakeIP exception.
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use url::{Host, Url};
pub fn strict_security() -> bool {
    true
}
fn is_internal_webview_host(name: &str) -> bool {
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    matches!(
        name.as_str(),
        "tauri.localhost" | "hg.localhost" | "ipc.localhost"
    )
}

pub fn allowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || (a == 198 && (b == 18 || b == 19))
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && b == 168)
                || (a == 192 && b == 0 && (c == 0 || c == 2))
                || (a == 192 && b == 88 && c == 99)
                || (a == 198 && b == 51 && c == 100)
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return allowed_ip(IpAddr::V4(v4));
            }
            let s = ip.segments();
            // 仅全球单播；拒绝 ULA、链路本地、回环、组播、NAT64，以及可封装私网 IPv4 的 6to4/Teredo。
            (s[0] & 0xe000) == 0x2000
                && s[0] != 0x2002
                && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}

pub fn parse_target(raw: &str) -> Result<Url, String> {
    parse_target_with_mode(raw, strict_security())
}

pub fn parse_target_with_mode(raw: &str, _ignored: bool) -> Result<Url, String> {
    let strict = true;
    if raw.len() > 8192 || raw.chars().any(char::is_control) {
        return Err("目标地址含控制字符".into());
    }
    let url = Url::parse(raw).map_err(|_| "目标地址无效")?;
    if !matches!(url.scheme(), "https" | "http")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("目标只允许不带认证信息的 HTTP(S) 地址".into());
    }
    match url.host().ok_or("目标缺少主机名")? {
        Host::Ipv4(ip) if strict => check_ip(ip.into())?,
        Host::Ipv6(ip) if strict => check_ip(ip.into())?,
        Host::Ipv4(_) | Host::Ipv6(_) => {}
        Host::Domain(name) => {
            if is_internal_webview_host(name) {
                return Err("禁止访问应用内部协议主机名".into());
            }
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            if strict
                && ["localhost", "local", "internal", "home.arpa"]
                    .iter()
                    .any(|suffix| name == *suffix || name.ends_with(&format!(".{suffix}")))
            {
                return Err("禁止访问本机或内网主机名".into());
            }
        }
    }
    Ok(url)
}

fn check_ip(ip: IpAddr) -> Result<(), String> {
    if allowed_ip(ip) {
        Ok(())
    } else {
        Err("禁止访问内网、回环、FakeIP 或保留地址".into())
    }
}

#[cfg(test)]
pub fn check_addresses(addrs: &[SocketAddr]) -> Result<(), String> {
    check_addresses_with_mode(addrs, strict_security())
}

pub fn check_addresses_with_mode(addrs: &[SocketAddr], _ignored: bool) -> Result<(), String> {
    let strict = true;
    if addrs.is_empty() {
        return Err("目标没有可用的 DNS 地址".into());
    }
    if strict {
        // 混合公私网答案整体拒绝，不能让连接器回退到私网地址。
        for addr in addrs {
            check_ip(addr.ip())?;
        }
    }
    Ok(())
}

/// 与浏览器一致优先 IPv6：连接器以首个地址的协议族为首选，另一族在其失败后（Happy Eyeballs）兜底。
/// 普通排序会把 IPv4 排在前面；共享 IPv4 出口被上游接口限流时，请求会挂起或返回 502/503，
/// 而同一网络下浏览器走 IPv6 一切正常。
fn sort_ipv6_first(addrs: &mut [SocketAddr]) {
    addrs.sort_unstable_by_key(|addr| (addr.is_ipv4(), *addr));
}

pub async fn resolve_target(url: &Url, strict: bool) -> Result<Vec<SocketAddr>, String> {
    let port = url.port_or_known_default().ok_or("目标端口无效")?;
    let mut addrs = match url.host().ok_or("目标缺少主机名")? {
        Host::Ipv4(ip) => vec![SocketAddr::new(ip.into(), port)],
        Host::Ipv6(ip) => vec![SocketAddr::new(ip.into(), port)],
        Host::Domain(host) => tokio::time::timeout(
            Duration::from_secs(10),
            tokio::net::lookup_host((host, port)),
        )
        .await
        .map_err(|_| "DNS 解析超时")?
        .map_err(|_| "DNS 解析失败")?
        .collect(),
    };
    sort_ipv6_first(&mut addrs);
    addrs.dedup();
    check_addresses_with_mode(&addrs, strict)?;
    Ok(addrs)
}

pub fn redirect_target(current: &Url, location: &str) -> Result<Url, String> {
    redirect_target_with_mode(current, location, strict_security())
}

pub fn redirect_target_with_mode(
    current: &Url,
    location: &str,
    _ignored: bool,
) -> Result<Url, String> {
    let strict = true;
    let next = current.join(location).map_err(|_| "重定向地址无效")?;
    let next = parse_target_with_mode(next.as_str(), strict)?;
    if strict && current.scheme() == "https" && next.scheme() != "https" {
        return Err("禁止 HTTPS 降级重定向".into());
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolved_addresses_prefer_ipv6() {
        let mut addrs: Vec<SocketAddr> = [
            "18.65.214.45:443",
            "[2600:9000:221c:5000:3:fdd6:fcc0:93a1]:443",
            "18.65.214.3:443",
            "[2600:9000:221c:4000:3:fdd6:fcc0:93a1]:443",
        ]
        .iter()
        .map(|s| s.parse().unwrap())
        .collect();
        sort_ipv6_first(&mut addrs);
        assert!(addrs[..2].iter().all(SocketAddr::is_ipv6), "{addrs:?}");
        assert!(addrs[2..].iter().all(SocketAddr::is_ipv4), "{addrs:?}");
        assert_eq!(addrs[2], "18.65.214.3:443".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn public_addresses_are_allowed() {
        for ip in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            assert!(allowed_ip(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn private_and_special_addresses_are_blocked() {
        for ip in [
            "198.18.0.0",
            "198.19.255.255",
            "::ffff:198.18.0.1",
            "0.0.0.0",
            "10.0.0.1",
            "100.64.0.1",
            "100.127.255.254",
            "127.0.0.1",
            "169.254.169.254",
            "172.16.0.1",
            "172.31.255.254",
            "192.168.1.1",
            "192.0.0.1",
            "192.0.2.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "fc00::1",
            "fe80::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::ffff:192.168.1.1",
            "64:ff9b::c0a8:101",
            "2002:c0a8:101::1",
            "2001::1",
            "2001:db8::1",
            "3fff::1",
        ] {
            assert!(!allowed_ip(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn reserved_ipv6_subranges_are_covered_by_the_existing_block() {
        for raw in [
            "2001:2::1",
            "2001:2:ffff:ffff:ffff:ffff:ffff:ffff",
            "2001:10::1",
            "2001:1f:ffff:ffff:ffff:ffff:ffff:ffff",
        ] {
            assert!(!allowed_ip(raw.parse().unwrap()), "{raw}");
        }
    }

    #[test]
    fn alternate_spellings_and_unsafe_urls_are_blocked() {
        for raw in [
            "http://127.1/",
            "http://2130706433/",
            "http://0x7f000001/",
            "http://[::ffff:192.168.0.1]/",
            "http://localhost./",
            "http://foo.local/",
            "file:///etc/passwd",
            "https://user:secret@example.com/",
            "https://example.com/\n",
        ] {
            assert!(parse_target(raw).is_err(), "{raw}");
        }
        assert!(parse_target("http://198.18.0.1/video").is_err());
    }

    #[test]
    fn mixed_dns_and_redirect_bypasses_are_blocked() {
        let addrs = [
            "1.1.1.1:443".parse().unwrap(),
            "192.168.1.1:443".parse().unwrap(),
        ];
        assert!(check_addresses(&addrs).is_err());
        assert!(check_addresses(&[]).is_err());
        let current = Url::parse("https://example.com/media").unwrap();
        assert!(redirect_target(&current, "//127.0.0.1/private").is_err());
        assert!(redirect_target(&current, "http://example.com/media").is_err());
        assert!(redirect_target(&current, "https://198.19.0.1/media").is_err());
        assert_eq!(
            redirect_target(&current, "/next").unwrap().as_str(),
            "https://example.com/next"
        );
    }

    #[test]
    fn cannot_disable_server_policy() {
        assert!(parse_target_with_mode("http://127.0.0.1", false).is_err());
        assert!(check_addresses_with_mode(&["127.0.0.1:80".parse().unwrap()], false).is_err());
    }
}
