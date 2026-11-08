use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use lemonfiber_contract::adapter::{about_path, ready_path, About, REQUEST_KEY_FILE};
use lemonfiber_contract::capabilities::library;
use lemonfiber_fixtures::http::{Answer, Fake};
use tower::ServiceExt as _;

use adapter_kit_servarr::curator::Curator;

use super::{about, kit, sonarr, DIGEST, RELEASE, UPSTREAM};

fn a_key() -> String {
    ('e'..='t').collect()
}

fn the_gate_s_key() -> String {
    ('G'..='W').collect()
}

fn configuration() -> &'static Path {
    static WRITTEN: OnceLock<PathBuf> = OnceLock::new();
    WRITTEN.get_or_init(|| {
        let directory =
            std::env::temp_dir().join(format!("sonarr-adapter-tests-{}", std::process::id()));
        let written = std::fs::create_dir_all(&directory).and_then(|()| {
            std::fs::write(
                directory.join(REQUEST_KEY_FILE),
                format!("{}\n", the_gate_s_key()),
            )
        });
        assert!(written.is_ok(), "{written:?}");
        directory
    })
}

const BASE: &str = "http://sonarr:8989";

fn curator(fake: &Arc<Fake>) -> Arc<Curator> {
    Arc::new(sonarr(
        fake.clone(),
        BASE,
        Arc::new(('a'..='p').collect::<String>()),
    ))
}

async fn answered_by(
    fake: &Arc<Fake>,
    method: &str,
    path: &str,
    key: Option<&str>,
    body: &str,
) -> (StatusCode, Vec<u8>) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(key) = key {
        request = request.header(header::AUTHORIZATION, format!("Bearer {key}"));
    }
    let Ok(request) = request.body(Body::from(body.to_owned())) else {
        return (StatusCode::IM_A_TEAPOT, Vec::new());
    };
    let Ok(response) = kit(a_key(), configuration(), curator(fake))
        .router()
        .oneshot(request)
        .await;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map(|bytes| bytes.to_vec())
        .unwrap_or_default();
    (status, bytes)
}

async fn answered(
    method: &str,
    path: &str,
    key: Option<&str>,
    body: &str,
) -> (StatusCode, Vec<u8>) {
    answered_by(&Fake::silent(), method, path, key, body).await
}

#[test]
fn it_speaks_the_curator_contract_and_names_the_release_its_recordings_were_taken_from() {
    let said = about();
    assert_eq!(said.speaks, ["library.curate@1"]);
    assert_eq!(said.upstream, UPSTREAM);
    assert_eq!(
        said.releases
            .iter()
            .map(|release| (release.version.as_str(), release.digest.as_str()))
            .collect::<Vec<_>>(),
        [(RELEASE, DIGEST)]
    );
}

#[tokio::test]
async fn about_is_answered_under_the_plugin_s_key() {
    let key = a_key();
    let (status, body) = answered("GET", &about_path(), Some(&key), "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(serde_json::from_slice::<About>(&body).ok(), Some(about()));
}

#[tokio::test]
async fn nothing_is_answered_without_the_key_or_under_another() {
    let operation = "/lemonfiber/library.curate/v1/identity";
    for key in [None, Some("not-the-plugin-s-key")] {
        assert_eq!(
            answered("POST", operation, key, "{}").await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            answered("GET", &about_path(), key, "").await.0,
            StatusCode::UNAUTHORIZED
        );
    }
}

#[tokio::test]
async fn an_unreachable_sonarr_is_answered_as_the_refusal_the_contract_declares() {
    let key = a_key();
    let (status, body) = answered(
        "POST",
        "/lemonfiber/library.curate/v1/identity",
        Some(&key),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let refusal: Option<serde_json::Value> = serde_json::from_slice(&body).ok();
    assert_eq!(
        refusal.and_then(|refusal| refusal.get("type").cloned()),
        Some(serde_json::json!("unavailable"))
    );
}

#[tokio::test]
async fn an_operation_is_answered_from_what_sonarr_says() {
    let key = a_key();
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"appName":"Sonarr","version":"4.0.20"}"#,
    ));
    let (status, body) = answered_by(
        &fake,
        "POST",
        "/lemonfiber/library.curate/v1/identity",
        Some(&key),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).ok(),
        Some(serde_json::json!({ "name": "Sonarr", "version": "4.0.20" }))
    );
    assert!(fake
        .requests()
        .iter()
        .all(|asked| asked.url == format!("{BASE}/api/v3/system/status")));
}

#[tokio::test]
async fn a_capability_it_does_not_speak_is_refused_as_unknown() {
    let key = a_key();
    let (status, _) = answered(
        "POST",
        "/lemonfiber/download.usenet/v1/transfers",
        Some(&key),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn it_is_ready_while_sonarr_answers_its_ping() {
    let key = a_key();
    let up = Fake::always(Answer::reply(200, r#"{"status":"OK"}"#));
    let (status, body) = answered_by(&up, "GET", &ready_path(), Some(&key), "").await;
    assert_eq!(
        (status, body.as_slice()),
        (StatusCode::OK, b"true".as_slice())
    );
    assert!(up
        .requests()
        .iter()
        .all(|asked| asked.url == format!("{BASE}/ping")));
    for down in [Fake::silent(), Fake::always(Answer::reply(503, ""))] {
        let (status, body) = answered_by(&down, "GET", &ready_path(), Some(&key), "").await;
        assert_eq!(
            (status, body.as_slice()),
            (StatusCode::OK, b"false".as_slice())
        );
    }
}

fn refused_as(body: &[u8]) -> Option<serde_json::Value> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|refusal| refusal.get("type").cloned())
}

#[tokio::test]
async fn the_request_gate_s_key_opens_each_operation_it_may_ask() {
    let gate = the_gate_s_key();
    let fake = Fake::always(Answer::reply(200, "[]"));
    let (status, body) = answered_by(
        &fake,
        "POST",
        "/lemonfiber/library.curate/v1/titles",
        Some(&gate),
        r#"{"kind":"tv"}"#,
    )
    .await;
    assert_eq!(
        (status, body.as_slice()),
        (StatusCode::OK, b"[]".as_slice())
    );
    assert!(fake
        .requests()
        .iter()
        .all(|asked| asked.url == format!("{BASE}/api/v3/series")));
    for operation in library::REQUESTED {
        let (status, body) = answered(
            "POST",
            &format!("/lemonfiber/library.curate/v1/{operation}"),
            Some(&gate),
            "{}",
        )
        .await;
        assert_ne!(status, StatusCode::UNAUTHORIZED, "{operation}");
        assert_ne!(
            refused_as(&body),
            Some(serde_json::json!("disallowed")),
            "{operation}"
        );
    }
}

#[tokio::test]
async fn the_request_gate_s_key_is_disallowed_everything_else() {
    let gate = the_gate_s_key();
    for (method, path) in [
        ("POST", "/lemonfiber/library.curate/v1/identity".to_owned()),
        ("POST", "/lemonfiber/library.curate/v1/carry".to_owned()),
        ("GET", about_path()),
        ("GET", ready_path()),
    ] {
        let fake = Fake::always(Answer::reply(200, "{}"));
        let (status, body) = answered_by(&fake, method, &path, Some(&gate), "{}").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(
            refused_as(&body),
            Some(serde_json::json!("disallowed")),
            "{path}"
        );
        assert!(fake.requests().is_empty(), "{path}");
    }
}

#[tokio::test]
async fn the_request_gate_s_key_opens_neither_unqueue_nor_grab() {
    let gate = the_gate_s_key();
    for (operation, body) in [
        ("unqueue", r#"{"kind":"tv","id":51,"blocklist":true}"#),
        ("grab", r#"{"kind":"tv","id":4,"release":"0000"}"#),
    ] {
        let fake = Fake::always(Answer::reply(200, "{}"));
        let (status, said) = answered_by(
            &fake,
            "POST",
            &format!("/lemonfiber/library.curate/v1/{operation}"),
            Some(&gate),
            body,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{operation}");
        assert_eq!(
            refused_as(&said),
            Some(serde_json::json!("disallowed")),
            "{operation}"
        );
        assert!(fake.requests().is_empty(), "{operation}");
        let (status, _) = answered_by(
            &fake,
            "POST",
            &format!("/lemonfiber/library.curate/v1/{operation}"),
            Some(&a_key()),
            body,
        )
        .await;
        assert_ne!(status, StatusCode::BAD_REQUEST, "{operation}");
        assert!(!fake.requests().is_empty(), "{operation}");
    }
}
