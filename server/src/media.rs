//! Server media core adapted from native media.rs; native client is unchanged.
//! No shared cache of authenticated resources or entire-segment buffering.
use aes::cipher::{BlockDecrypt, KeyInit};
use aes::Aes128;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use std::sync::Arc;
const IMG_KEY: &[u8] = b"f5d965df75336270";
const IMG_IV: &[u8] = b"97b60394abc2fbe1";
pub const MAX_IMAGE: usize = 8 * 1024 * 1024;
pub const MAX_PLAYLIST: usize = 2 * 1024 * 1024;
fn aes_cbc_decrypt(data: &[u8]) -> Option<Vec<u8>> {
    if data.is_empty() || data.len() % 16 != 0 {
        return None;
    }
    let cipher = Aes128::new_from_slice(IMG_KEY).ok()?;
    let mut prev = [0u8; 16];
    prev.copy_from_slice(IMG_IV);
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks(16) {
        let mut ct = [0u8; 16];
        ct.copy_from_slice(chunk);
        let mut block = ct;
        cipher.decrypt_block((&mut block).into());
        for i in 0..16 {
            out.push(block[i] ^ prev[i]);
        }
        prev = ct;
    }
    Some(out)
}

fn strip_padding(mut buf: Vec<u8>) -> Vec<u8> {
    let pad = *buf.last().unwrap_or(&0) as usize;
    if pad > 0 && pad <= 16 && buf.len() >= pad {
        if buf[buf.len() - pad..].iter().all(|b| *b as usize == pad) {
            buf.truncate(buf.len() - pad);
        }
    }
    buf
}

pub fn image_kind(buf: &[u8]) -> Option<&'static str> {
    if buf.len() < 12 {
        return None;
    }
    if buf[0] == 0xFF && buf[1] == 0xD8 {
        return Some("image/jpeg");
    }
    if buf.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Some("image/png");
    }
    if buf.starts_with(b"GIF8") {
        return Some("image/gif");
    }
    if buf.starts_with(b"RIFF") && &buf[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

pub async fn load_image(url: &str) -> Result<(String, Arc<Vec<u8>>), String> {
    let (_, _, bytes) = crate::net::fetch_bounded(url, MAX_IMAGE).await?;
    let plain = if image_kind(&bytes).is_some() {
        bytes
    } else {
        strip_padding(aes_cbc_decrypt(&bytes).ok_or("封面解密失败")?)
    };
    let kind = image_kind(&plain).ok_or("上游未返回有效图片")?.to_string();
    Ok((kind, Arc::new(plain)))
}
pub async fn load_playlist(url: &str, prefix: &str) -> Result<Arc<String>, String> {
    let (final_url, _, bytes) = crate::net::fetch_bounded(url, MAX_PLAYLIST).await?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "播放列表不是 UTF-8 文本")?;
    Ok(Arc::new(rewrite_playlist(&final_url, text, prefix)?))
}
fn enc(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

fn absolutize(base: &str, target: &str) -> String {
    if target.starts_with("http://") || target.starts_with("https://") {
        return target.to_string();
    }
    if let Ok(b) = url::Url::parse(base) {
        if let Ok(j) = b.join(target) {
            return j.to_string();
        }
    }
    target.to_string()
}

/// 属性列表中的引号内可含逗号，不能直接 split(',')。
fn hls_attributes(line: &str) -> Option<(&str, Vec<&str>)> {
    let (tag, rest) = line.split_once(':')?;
    let mut attributes = Vec::new();
    let mut quoted = false;
    let mut start = 0;
    for (i, ch) in rest.char_indices() {
        if ch == '"' {
            quoted = !quoted;
        }
        if ch == ',' && !quoted {
            attributes.push(&rest[start..i]);
            start = i + 1;
        }
    }
    attributes.push(&rest[start..]);
    Some((tag, attributes))
}

fn routed(base: &str, uri: &str) -> Result<String, String> {
    let absolute = absolutize(base, uri);
    crate::network_policy::parse_target(&absolute)?;
    Ok(format!("/media/seg?u={}", enc(&absolute)))
}
/// Strictly same-origin URIs. The server supplies a constant prefix, never Host.
pub fn rewrite_playlist(base: &str, text: &str, prefix: &str) -> Result<String, String> {
    if prefix != "/media" {
        return Err("无效媒体前缀".into());
    }
    if text.len() > MAX_PLAYLIST || !text.trim_start_matches('\u{feff}').starts_with("#EXTM3U") {
        return Err("响应不是有效大小的 HLS 列表".into());
    }
    let mut out = String::with_capacity(text.len() + 512);
    for raw in text.trim_start_matches('\u{feff}').lines() {
        if raw.len() > 16384 {
            return Err("播放列表行过长".into());
        }
        let line = raw.trim();
        if line.starts_with("#EXT-X-DEFINE:") {
            return Err("暂不支持变量播放列表".into());
        }
        if line.starts_with("#EXT-X-CONTENT-STEERING:") {
            continue;
        }
        if line.starts_with('#') {
            if let Some((tag, attrs)) = hls_attributes(line) {
                let mut mapped = Vec::new();
                for attr in attrs {
                    let Some((key, value)) = attr.split_once('=') else {
                        mapped.push(attr.to_owned());
                        continue;
                    };
                    let key = key.trim();
                    if tag == "#EXT-X-DATERANGE"
                        && (key.to_ascii_uppercase().contains("URI") || key == "X-ASSET-LIST")
                    {
                        continue;
                    }
                    if key == "URI" {
                        let uri = value
                            .trim()
                            .strip_prefix('"')
                            .and_then(|v| v.strip_suffix('"'))
                            .ok_or("无效 HLS URI 属性")?;
                        mapped.push(format!("{key}=\"{}\"", routed(base, uri)?));
                    } else {
                        mapped.push(attr.to_owned());
                    }
                }
                out.push_str(&format!("{tag}:{}", mapped.join(",")));
            } else {
                out.push_str(line);
            }
        } else if !line.is_empty() {
            out.push_str(&routed(base, line)?);
        }
        out.push('\n');
        if out.len() > 8 * MAX_PLAYLIST {
            return Err("改写后的播放列表过大".into());
        }
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_uris_are_same_origin() {
        let p=rewrite_playlist("https://cdn.example/master.m3u8", "#EXTM3U\n#EXT-X-MEDIA:NAME=\"audio,main\",URI=\"audio/a.m3u8\"\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXT-X-PART:URI=\"part.ts\"\n#EXT-X-PRELOAD-HINT:URI=\"next.ts\"\nchild.m3u8\n", "/media").unwrap();
        assert_eq!(p.matches("/media/seg?u=").count(), 6);
        assert!(p.contains("NAME=\"audio,main\""));
        assert!(p.contains("https%3A%2F%2Fcdn%2Eexample%2Faudio%2Fa%2Em3u8"));
    }
    #[test]
    fn unsafe_or_unsupported_playlists_fail_closed() {
        for p in [
            "<html>wrong",
            "#EXTM3U\nhttp://127.0.0.1/file\n",
            "#EXTM3U\n#EXT-X-KEY:URI=\"file:///etc/passwd\"\n",
            "#EXTM3U\n#EXT-X-DEFINE:NAME=\"path\",VALUE=\"evil\"\n",
            "#EXTM3U\n#EXT-X-KEY:URI=http://example.com/key\n",
        ] {
            assert!(rewrite_playlist("https://cdn.example/list.m3u8", p, "/media").is_err());
        }
        assert!(rewrite_playlist(
            "https://cdn.example/",
            "#EXTM3U\nx.ts",
            "https://evil.example"
        )
        .is_err());
        assert!(rewrite_playlist(
            "https://cdn.example/",
            &format!("#EXTM3U\n{}", "a".repeat(17000)),
            "/media"
        )
        .is_err());
    }
    #[test]
    fn metadata_cannot_fetch_external_assets() {
        let p=rewrite_playlist("https://cdn.example/list.m3u8", "#EXTM3U\n#EXT-X-CONTENT-STEERING:SERVER-URI=\"https://evil.example\"\n#EXT-X-DATERANGE:ID=\"a\",DURATION=4,X-ASSET-URI=\"https://evil.example\"\nx.ts", "/media").unwrap();
        assert!(!p.contains("evil") && !p.contains("STEERING"));
        assert!(p.contains("DURATION=4"));
    }
    #[test]
    fn cover_decrypt_roundtrip() {
        use aes::cipher::BlockEncrypt;

        let plain = b"\xFF\xD8\xFF\xE0cover-payload-for-test".to_vec();
        let mut buf = plain.clone();
        let pad = 16 - plain.len() % 16;
        buf.extend(std::iter::repeat_n(pad as u8, pad));

        let cipher = Aes128::new_from_slice(IMG_KEY).expect("key");
        let mut prev = [0u8; 16];
        prev.copy_from_slice(IMG_IV);
        let mut encrypted = Vec::with_capacity(buf.len());
        for chunk in buf.chunks(16) {
            let mut block = [0u8; 16];
            block.copy_from_slice(chunk);
            for i in 0..16 {
                block[i] ^= prev[i];
            }
            let mut out_block = block;
            cipher.encrypt_block((&mut out_block).into());
            encrypted.extend_from_slice(&out_block);
            prev = out_block;
        }

        let decrypted = aes_cbc_decrypt(&encrypted).expect("decrypt");
        let stripped = strip_padding(decrypted);
        assert_eq!(stripped, plain);
        assert_eq!(image_kind(&stripped), Some("image/jpeg"));
    }
}
