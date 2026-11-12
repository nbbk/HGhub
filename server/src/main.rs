//! Private guest-only web adapter; does not change the native client.
mod media;
mod net;
mod network_policy;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use tokio::sync::Semaphore;
use tower_http::services::{ServeDir, ServeFile};
#[derive(Clone)]
struct App {
    hash: [u8; 32],
    salt: [u8; 32],
    origin: String,
    host: String,
    secure: bool,
    sessions: Arc<Mutex<HashMap<String, Instant>>>,
    attempts: Arc<Mutex<Vec<Instant>>>,
    slots: Arc<Semaphore>,
}
fn err(code: StatusCode, msg: &str) -> Response {
    (code, Json(json!({"error":msg}))).into_response()
}
fn digest(salt: &[u8; 32], p: &str) -> [u8; 32] {
    let mut d = Sha256::new();
    d.update(salt);
    d.update(p.as_bytes());
    d.finalize().into()
}
fn token(h: &HeaderMap) -> Option<&str> {
    h.get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|s| s.trim().strip_prefix("hghub_session="))
}
fn authed(a: &App, h: &HeaderMap) -> bool {
    let Some(t) = token(h) else { return false };
    let mut s = a.sessions.lock().unwrap();
    s.retain(|_, v| *v > Instant::now());
    s.contains_key(t)
}
async fn guard(State(a): State<App>, r: Request, next: Next) -> Response {
    if r.uri().path() != "/healthz"
        && r.headers().get(header::HOST).and_then(|v| v.to_str().ok()) != Some(&a.host)
    {
        return err(
            StatusCode::MISDIRECTED_REQUEST,
            "访问域名与 HGHUB_WEB_ORIGIN 不一致",
        );
    }
    if r.uri().path().len() + r.uri().query().map_or(0, str::len) > 16384 {
        return err(StatusCode::URI_TOO_LONG, "请求地址过长");
    }
    if !matches!(*r.method(), Method::GET | Method::HEAD)
        && r.headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            != Some(&a.origin)
    {
        return err(StatusCode::FORBIDDEN, "请求来源不匹配");
    }
    if (r.uri().path().starts_with("/api/") || r.uri().path().starts_with("/media/"))
        && !authed(&a, r.headers())
    {
        return err(
            StatusCode::UNAUTHORIZED,
            "访问会话已过期，请重新输入访问密码",
        );
    }
    let mut out = next.run(r).await;
    let h = out.headers_mut();
    for(k,v)in[("cache-control","no-store"),("x-content-type-options","nosniff"),("referrer-policy","no-referrer"),("x-frame-options","DENY"),("content-security-policy","default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; media-src 'self' blob:; connect-src 'self'; worker-src 'self' blob:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'")]{h.insert(k,HeaderValue::from_static(v));}
    if a.secure {
        h.insert(
            "strict-transport-security",
            HeaderValue::from_static("max-age=31536000"),
        );
    }
    out
}
#[derive(Deserialize)]
struct Login {
    password: String,
}
async fn login(State(a): State<App>, Json(v): Json<Login>) -> Response {
    {
        let mut t = a.attempts.lock().unwrap();
        t.retain(|v| v.elapsed() < Duration::from_secs(60));
        if t.len() >= 12 {
            return err(
                StatusCode::TOO_MANY_REQUESTS,
                "尝试过于频繁，请一分钟后再试",
            );
        }
        t.push(Instant::now());
    }
    if v.password.len() > 1024 || !bool::from(digest(&a.salt, &v.password).ct_eq(&a.hash)) {
        tokio::time::sleep(Duration::from_millis(200)).await;
        return err(StatusCode::UNAUTHORIZED, "访问密码不正确");
    }
    let t = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
    {
        let mut s = a.sessions.lock().unwrap();
        s.retain(|_, v| *v > Instant::now());
        if s.len() >= 32 {
            return err(
                StatusCode::TOO_MANY_REQUESTS,
                "活跃会话过多，请先退出其他浏览器",
            );
        }
        s.insert(t.clone(), Instant::now() + Duration::from_secs(43200));
    }
    (
        [(
            header::SET_COOKIE,
            format!(
                "hghub_session={t}; Path=/; HttpOnly; SameSite=Strict; Max-Age=43200{}",
                if a.secure { "; Secure" } else { "" }
            ),
        )],
        Json(json!({"authenticated":true})),
    )
        .into_response()
}
async fn logout(State(a): State<App>, h: HeaderMap) -> Response {
    if let Some(t) = token(&h) {
        a.sessions.lock().unwrap().remove(t);
    }
    (
        [(
            header::SET_COOKIE,
            format!(
                "hghub_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{}",
                if a.secure { "; Secure" } else { "" }
            ),
        )],
        Json(json!({"authenticated":false})),
    )
        .into_response()
}
async fn status(State(a): State<App>, h: HeaderMap) -> Json<Value> {
    Json(json!({"authenticated":authed(&a,&h)}))
}
fn n(v: &Value, k: &str, d: u64, max: u64) -> Result<u64, String> {
    let x = match v.get(k) {
        None => d,
        Some(x) => x.as_u64().ok_or("参数必须为正整数")?,
    };
    if x == 0 || x > max {
        Err("参数超出范围".into())
    } else {
        Ok(x)
    }
}
fn s<'a>(v: &'a Value, k: &str, max: usize) -> Result<&'a str, String> {
    let x = v.get(k).and_then(Value::as_str).ok_or("缺少字符串参数")?;
    if x.len() > max || x.chars().any(char::is_control) {
        Err("字符串参数无效".into())
    } else {
        Ok(x)
    }
}
fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
fn play_normalize(v: &mut Value) {
    for (k, t) in [("video_url", "main"), ("video_url_hevc", "hevc")] {
        if v.get(k).and_then(Value::as_str).unwrap_or("").is_empty() {
            if let Some(u) = v
                .get("streams")
                .and_then(|x| x.get(t))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
            {
                v[k] = json!(u);
            }
        }
    }
}
async fn dispatch(cmd: &str, v: Value) -> Result<Value, String> {
    if !v.is_object() {
        return Err("参数必须为对象".into());
    }
    match cmd {
        "app_boot" => {
            return Ok(
                json!({"user":null,"logged_in":false,"base":net::base(),"strict_security":true,"cli_relaxed":false}),
            )
        }
        "line_state" => {
            return Ok(
                json!({"base":net::base(),"default":net::base(),"fixed":[{"name":"服务器配置线路","url":net::base()}]}),
            )
        }
        "api_logout" | "cache_clear" => return Ok(json!({"ok":true})),
        _ => {}
    }
    let p = || n(&v, "page", 1, 10000);
    let z = || n(&v, "size", 20, 100);
    let id = || n(&v, "id", 1, i64::MAX as u64);
    let ep = || n(&v, "ep", 1, 100000);
    let path = match cmd {
        "api_feed" => format!("/api/videos?page={}&size={}", p()?, z()?),
        "api_category" => {
            let cate = s(&v, "cate", 128)?;
            if cate.is_empty()
                || !cate
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                return Err("无效分类".into());
            }
            format!(
                "/api/videos/category/{}?page={}&size={}",
                enc(cate),
                p()?,
                z()?
            )
        }
        "api_rank" => {
            let k = s(&v, "kind", 20)?;
            if !["hot", "potential", "recommend"].contains(&k) {
                return Err("无效榜单".into());
            }
            format!("/api/ranks/{k}")
        }
        "api_search" => format!(
            "/api/search?q={}&page={}&size={}",
            enc(s(&v, "q", 512)?),
            p()?,
            z()?
        ),
        "api_posts" => format!("/api/posts?page={}&size={}", p()?, z()?),
        "api_post" => format!("/api/posts/{}", id()?),
        "api_video" => format!("/api/videos/{}", id()?),
        "api_comments" => format!(
            "/api/videos/{}/comments?ep={}&page={}&size={}",
            id()?,
            ep()?,
            p()?,
            z()?
        ),
        "api_post_comments" => format!("/api/posts/{}/comments?page={}&size={}", id()?, p()?, z()?),
        "api_play" => {
            let i = id()?;
            let e = ep()?;
            let primary = net::edge(
                &net::base(),
                "POST",
                &format!("/api/videos/{i}/episodes/{e}/playback"),
                Some(json!({})),
                None,
            )
            .await
            .map(|mut v| {
                play_normalize(&mut v);
                v
            });
            if primary
                .as_ref()
                .ok()
                .and_then(|v| v.get("video_url"))
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty())
            {
                return primary;
            }
            return net::edge(
                &net::base(),
                "GET",
                &format!("/api/videos/{i}/play?ep={e}&page_texts=0"),
                None,
                None,
            )
            .await
            .map(|mut v| {
                play_normalize(&mut v);
                v
            })
            .or(primary);
        }
        _ => return Err("个人网页版仅支持游客浏览；上游账号与原生功能请使用客户端".into()),
    };
    net::edge(&net::base(), "GET", &path, None, None).await
}
async fn api(State(a): State<App>, Path(cmd): Path<String>, Json(v): Json<Value>) -> Response {
    let Ok(_p) = a.slots.clone().try_acquire_owned() else {
        return err(StatusCode::TOO_MANY_REQUESTS, "请求过多");
    };
    match tokio::time::timeout(Duration::from_secs(65), dispatch(&cmd, v)).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => err(StatusCode::BAD_GATEWAY, &e),
        Err(_) => err(StatusCode::GATEWAY_TIMEOUT, "上游请求超时"),
    }
}
#[derive(Deserialize)]
struct MediaQuery {
    u: String,
}
async fn media_route(
    State(a): State<App>,
    Path(kind): Path<String>,
    Query(q): Query<MediaQuery>,
    method: Method,
    h: HeaderMap,
) -> Response {
    if q.u.len() > 8192 {
        return err(StatusCode::BAD_REQUEST, "媒体地址过长");
    }
    let Ok(permit) = a.slots.clone().try_acquire_owned() else {
        return err(StatusCode::TOO_MANY_REQUESTS, "媒体请求过多");
    };
    let head = method == Method::HEAD;
    let result:Result<Response,String>=async{
 if kind=="img"{let(ct,b)=media::load_image(&q.u).await?;return Ok(([(header::CONTENT_TYPE,ct)],if head{Body::empty()}else{Body::from(b.as_ref().clone())}).into_response())}
 if kind=="hls"{let b=media::load_playlist(&q.u,"/media").await?;return Ok(([(header::CONTENT_TYPE,"application/vnd.apple.mpegurl")],if head{Body::empty()}else{Body::from(b.as_ref().clone())}).into_response())}
 if kind!="seg"{return Err("未知媒体类型".into())}
 let range=h.get(header::RANGE).and_then(|v|v.to_str().ok());let mut r=net::get_range(&q.u,range,head).await?;let status=r.status();
 if !status.is_success()&&status!=StatusCode::RANGE_NOT_SATISFIABLE{return Err("媒体上游错误".into())}
 let ct=r.headers().get(header::CONTENT_TYPE).and_then(|v|v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
 let mut prefix=bytes::BytesMut::new();
 if !head&&range.is_none()&&status.is_success(){while prefix.len()<32{match r.chunk().await.map_err(|_|"媒体读取失败")?{Some(c)=>{if c.len()>media::MAX_PLAYLIST{return Err("首块过大".into())}prefix.extend_from_slice(&c)},None=>break}}}
 let looks_hls=std::str::from_utf8(&prefix[..prefix.len().min(16)]).unwrap_or("").trim_start_matches('\u{feff}').starts_with("#EXTM3U");
 if !head&&range.is_none()&&(looks_hls||ct.contains("mpegurl")){let u=r.url().to_string();let rest=net::bounded_bytes(r,media::MAX_PLAYLIST.saturating_sub(prefix.len())).await?;prefix.extend_from_slice(&rest);let b=media::rewrite_playlist(&u,std::str::from_utf8(&prefix).map_err(|_|"播放列表不是 UTF-8")?,"/media")?;return Ok(([(header::CONTENT_TYPE,"application/vnd.apple.mpegurl")],b).into_response())}
 let mut out=Response::builder().status(status);
 for k in [header::CONTENT_RANGE,header::ACCEPT_RANGES,header::CONTENT_LENGTH]{if k==header::CONTENT_LENGTH&&status==StatusCode::RANGE_NOT_SATISFIABLE&&!head{continue}if let Some(v)=r.headers().get(&k){out=out.header(k,v)}}
 let mime=if ct.starts_with("video/")||ct.starts_with("audio/")||ct.starts_with("text/vtt"){ct.as_str()}else{"application/octet-stream"};out=out.header(header::CONTENT_TYPE,mime);
 if head||status==StatusCode::RANGE_NOT_SATISFIABLE{return Ok(out.body(Body::empty()).unwrap())}
 if r.content_length().is_some_and(|n|n>512*1024*1024){return Err("媒体对象超过 512 MiB 上限".into())}
 let stream=async_stream::stream! {
   let _permit=permit; let mut count=prefix.len();
   if !prefix.is_empty(){yield Ok::<bytes::Bytes,std::io::Error>(prefix.freeze());}
   loop { match tokio::time::timeout(Duration::from_secs(30),r.chunk()).await {
     Ok(Ok(Some(c)))=>{count+=c.len();if count>512*1024*1024{yield Err(std::io::Error::other("media size limit"));break;}yield Ok(c);},
     Ok(Ok(None))=>break,
     _=>{yield Err(std::io::Error::other("media stream failed"));break;}
   }}
 };
 Ok(out.body(Body::from_stream(stream)).unwrap())
 }.await;
    result.unwrap_or_else(|_| {
        err(
            StatusCode::BAD_GATEWAY,
            "媒体加载失败：请检查上游、格式及服务器网络",
        )
    })
}
fn router(a: App, dir: &str) -> Router {
    Router::new()
        .route("/healthz", get(|| async { Json(json!({"ok":true})) }))
        .route("/auth/status", get(status))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/api/{cmd}", post(api))
        .route("/media/{kind}", get(media_route))
        .fallback_service(
            ServeDir::new(dir).not_found_service(ServeFile::new(format!("{dir}/index.html"))),
        )
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(a.clone(), guard))
        .with_state(a)
}
fn configuration(p: &str, origin: &str) -> Result<App, String> {
    if p.len() < 16 || p.len() > 1024 {
        return Err("访问密码须为 16–1024 字节".into());
    }
    let u = url::Url::parse(origin).map_err(|_| "无效 HGHUB_WEB_ORIGIN")?;
    if u.path() != "/"
        || u.query().is_some()
        || u.fragment().is_some()
        || !u.username().is_empty()
        || u.password().is_some()
    {
        return Err("HGHUB_WEB_ORIGIN 须为纯 origin".into());
    }
    let local = matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if u.scheme() != "https" && !(u.scheme() == "http" && local) {
        return Err("公网必须使用 HTTPS，HTTP 只限 localhost/127.0.0.1".into());
    }
    let origin = u.origin().ascii_serialization();
    let host = origin.split_once("://").unwrap().1.to_string();
    let salt = rand::random();
    Ok(App {
        hash: digest(&salt, p),
        salt,
        origin,
        host,
        secure: u.scheme() == "https",
        sessions: Default::default(),
        attempts: Default::default(),
        slots: Arc::new(Semaphore::new(24)),
    })
}
#[tokio::main]
async fn main() {
    let p = match std::env::var("HGHUB_WEB_PASSWORD_FILE") {
        Ok(f) => std::fs::read_to_string(f)
            .expect("无法读取密码文件")
            .trim_end_matches(['\r', '\n'])
            .to_string(),
        Err(_) => std::env::var("HGHUB_WEB_PASSWORD").expect("请设置 HGHUB_WEB_PASSWORD_FILE"),
    };
    let origin =
        std::env::var("HGHUB_WEB_ORIGIN").unwrap_or_else(|_| "http://localhost:8080".into());
    let a = configuration(&p, &origin).unwrap_or_else(|e| panic!("{e}"));
    drop(p);
    let bind = std::env::var("HGHUB_WEB_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let dir = std::env::var("HGHUB_WEB_STATIC_DIR").unwrap_or_else(|_| "dist".into());
    let l = tokio::net::TcpListener::bind(&bind)
        .await
        .expect("监听失败");
    eprintln!("HGhub personal web: {bind}, origin {origin}, guest-only");
    axum::serve(l, router(a, &dir))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("HTTP 服务退出");
}
#[cfg(test)]
mod tests;
