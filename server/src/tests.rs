use super::*;
use http_body_util::BodyExt;
use tower::ServiceExt;
fn app() -> App {
    configuration("test-password-long-enough", "https://watch.example.com").unwrap()
}
fn req(method: &str, path: &str, body: &str) -> axum::http::Request<Body> {
    axum::http::Request::builder()
        .method(method)
        .uri(path)
        .header("host", "watch.example.com")
        .header("origin", "https://watch.example.com")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
async fn log_in(a: App) -> String {
    let r = router(a, "/tmp/no-static-fixture")
        .oneshot(req(
            "POST",
            "/auth/login",
            r#"{"password":"test-password-long-enough"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let c = r.headers()[header::SET_COOKIE].to_str().unwrap();
    for f in ["HttpOnly", "SameSite=Strict", "Secure", "Max-Age=43200"] {
        assert!(c.contains(f));
    }
    c.split(';').next().unwrap().to_string()
}
#[test]
fn insecure_config_rejected() {
    assert!(configuration("short", "https://example.com").is_err());
    for u in [
        "http://example.com",
        "https://user:pass@example.com",
        "https://example.com/path",
        "https://example.com?x=1",
        "file:///tmp",
    ] {
        assert!(
            configuration("test-password-long-enough", u).is_err(),
            "{u}"
        );
    }
    assert!(configuration("test-password-long-enough", "http://localhost:8080").is_ok());
}
#[tokio::test]
async fn auth_required_for_api_and_media() {
    for (m, p, b) in [
        ("POST", "/api/api_feed", "{}"),
        ("GET", "/media/seg?u=http://127.0.0.1", ""),
        ("HEAD", "/media/hls?u=https://example.com", ""),
    ] {
        assert_eq!(
            router(app(), "/tmp/no-static-fixture")
                .oneshot(req(m, p, b))
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
}
#[tokio::test]
async fn host_and_csrf_guard() {
    let mut r = req(
        "POST",
        "/auth/login",
        r#"{"password":"test-password-long-enough"}"#,
    );
    r.headers_mut()
        .insert("origin", HeaderValue::from_static("https://evil.example"));
    assert_eq!(
        router(app(), "/tmp").oneshot(r).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let mut r = req("GET", "/auth/status", "");
    r.headers_mut()
        .insert("host", HeaderValue::from_static("evil.example"));
    r.headers_mut().insert(
        "x-forwarded-host",
        HeaderValue::from_static("watch.example.com"),
    );
    assert_eq!(
        router(app(), "/tmp").oneshot(r).await.unwrap().status(),
        StatusCode::MISDIRECTED_REQUEST
    );
}
#[tokio::test]
async fn session_logout_and_expiry() {
    let a = app();
    let c = log_in(a.clone()).await;
    let mut r = req("POST", "/api/app_boot", "{}");
    r.headers_mut().insert(header::COOKIE, c.parse().unwrap());
    let r = router(a.clone(), "/tmp").oneshot(r).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let v: Value =
        serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(v["strict_security"], true);
    assert_eq!(v["logged_in"], false);
    let mut r = req("POST", "/auth/logout", "{}");
    r.headers_mut().insert(header::COOKIE, c.parse().unwrap());
    assert_eq!(
        router(a.clone(), "/tmp").oneshot(r).await.unwrap().status(),
        StatusCode::OK
    );
    let mut r = req("GET", "/media/seg?u=https://example.com", "");
    r.headers_mut().insert(header::COOKIE, c.parse().unwrap());
    assert_eq!(
        router(a.clone(), "/tmp").oneshot(r).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let c = log_in(a.clone()).await;
    a.sessions
        .lock()
        .unwrap()
        .values_mut()
        .for_each(|v| *v = Instant::now() - Duration::from_secs(1));
    let mut r = req("GET", "/media/seg?u=https://example.com", "");
    r.headers_mut().insert(header::COOKIE, c.parse().unwrap());
    assert_eq!(
        router(a, "/tmp").oneshot(r).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
async fn rate_and_body_limits() {
    let a = app();
    for _ in 0..12 {
        a.attempts.lock().unwrap().push(Instant::now());
    }
    assert_eq!(
        router(a, "/tmp")
            .oneshot(req("POST", "/auth/login", r#"{"password":"wrong"}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    let huge = json!({"password":"a".repeat(17000)}).to_string();
    assert_eq!(
        router(app(), "/tmp")
            .oneshot(req("POST", "/auth/login", &huge))
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
}
#[tokio::test]
async fn explicit_command_allowlist_and_limits() {
    for cmd in [
        "debug_log",
        "net_diag",
        "security_set",
        "api_login",
        "line_set",
        "api_register",
    ] {
        assert!(dispatch(cmd, json!({})).await.is_err());
    }
    assert!(dispatch("api_feed", json!({"size":1000000})).await.is_err());
    assert!(dispatch("api_search", json!({"q":"a".repeat(600)}))
        .await
        .is_err());
    assert!(dispatch("api_category", json!({"cate":".."}))
        .await
        .is_err());
}
