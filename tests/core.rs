use mokawa::{
    api::Api,
    auth::{Registration, Session, now},
    error::Error,
    files::ConfirmedDeletion,
    models::{File, TransferPhase, TransferRecord},
    state::BrowserState,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path, query_param},
};
fn session() -> Session {
    Session {
        access_token: "secret-access".into(),
        refresh_token: "secret-refresh".into(),
        captcha_token: "secret-captcha".into(),
        device_id: "device".into(),
        expires_at: now() + 3600,
    }
}
fn file(id: &str, name: &str) -> File {
    File {
        id: id.into(),
        name: name.into(),
        kind: "drive#file".into(),
        ..Default::default()
    }
}
#[test]
fn model_size_and_secret_redaction() {
    let f: File = serde_json::from_str(
        r#"{"id":"a","name":"x","kind":"drive#file","size":"9223372036854775808"}"#,
    )
    .unwrap();
    assert_eq!(f.size, 9223372036854775808);
    assert!(
        serde_json::from_str::<File>(r#"{"id":"a","name":"x","kind":"drive#file","size":-1}"#)
            .is_err()
    );
    assert!(!format!("{:?}", session()).contains("secret"));
}
#[test]
fn deletion_requires_confirmation_and_valid_ids() {
    assert!(matches!(
        ConfirmedDeletion::confirm(vec!["a".into()], false),
        Err(Error::ConfirmationRequired)
    ));
    assert!(ConfirmedDeletion::confirm(vec!["../x".into()], true).is_err());
    assert!(ConfirmedDeletion::confirm(vec![], true).is_err());
}
#[test]
fn stale_results_never_replace_new_folder() {
    let mut s = BrowserState::default();
    let old = s.begin_navigation();
    let new = s.begin_navigation();
    assert!(!s.apply(old, vec![file("a", "Old")]));
    assert!(s.apply(new, vec![file("b", "New")]));
    assert_eq!(s.visible("new", false)[0].id, "b");
}
#[tokio::test]
async fn paginated_files_include_auth_and_filters() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/drive/v1/files"))
        .and(query_param("parent_id", "folder"))
        .and(header("authorization", "Bearer secret-access"))
        .and(query_param("page_token", "next"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"files":[{"id":"b","name":"B","kind":"drive#folder"}]}),
        ))
        .with_priority(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/drive/v1/files")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"files":[{"id":"a","name":"A","kind":"drive#file"}],"next_page_token":"next"}))).with_priority(2).expect(1).mount(&server).await;
    let files = Api::for_mock(&server.uri())
        .unwrap()
        .list_files(&session(), "folder", false)
        .await
        .unwrap();
    assert_eq!(files.len(), 2);
}
#[tokio::test]
async fn expired_session_does_not_send_request() {
    let server = MockServer::start().await;
    let mut s = session();
    s.expires_at = 0;
    assert!(matches!(
        Api::for_mock(&server.uri())
            .unwrap()
            .list_files(&s, "", false)
            .await,
        Err(Error::Unauthorized)
    ));
    assert_eq!(server.received_requests().await.unwrap().len(), 0);
}
#[tokio::test]
async fn errors_are_bounded_and_redacted() {
    for (status, expected) in [(401, "expired"), (429, "rate limiting"), (500, "HTTP 500")] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(status).set_body_string("secret-token"))
            .expect(1)
            .mount(&server)
            .await;
        let err = Api::for_mock(&server.uri())
            .unwrap()
            .list_files(&session(), "", false)
            .await
            .unwrap_err();
        assert!(err.to_string().contains(expected));
        assert!(!format!("{err:?}").contains("secret-token"));
    }
}
#[tokio::test]
async fn login_captcha_stops_before_password_submission() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/shield/captcha/init"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"url":"https://example.test/challenge","captcha_token":"challenge"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    let reg = Registration {
        client_id: "mokawa-test".into(),
        client_secret: "test".into(),
    };
    assert!(matches!(
        Api::for_mock(&server.uri())
            .unwrap()
            .login(&reg, "account", &"password".into(), "device")
            .await,
        Err(Error::VerificationRequired)
    ));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}
#[tokio::test]
async fn refresh_retains_rotates_and_expires() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/auth/token"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"access_token":"new","refresh_token":"rotated","expires_in":3600}),
        ))
        .mount(&server)
        .await;
    let reg = Registration {
        client_id: "mokawa-test".into(),
        client_secret: "test".into(),
    };
    let result = Api::for_mock(&server.uri())
        .unwrap()
        .refresh(&reg, &session())
        .await
        .unwrap();
    assert!(!result.expired());
    use secrecy::ExposeSecret;
    assert_eq!(result.refresh_token.expose_secret(), "rotated");
}
#[tokio::test]
async fn interrupted_queue_reloads_without_claiming_resume() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("queue.json");
    let records = vec![TransferRecord {
        id: "1".into(),
        file_id: "a".into(),
        file_name: "file".into(),
        completed: 12,
        total: Some(30),
        phase: TransferPhase::Running,
    }];
    mokawa::download::save_queue(&path, &records).await.unwrap();
    let loaded = mokawa::download::load_queue(&path).await.unwrap();
    assert_eq!(loaded[0].phase, TransferPhase::Interrupted);
    assert_eq!(loaded[0].completed, 12);
}
#[tokio::test]
async fn cyclic_pagination_is_rejected() {
    let server = MockServer::start().await;
    Mock::given(path("/drive/v1/files"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"next_page_token":"same"})),
        )
        .expect(2)
        .mount(&server)
        .await;
    assert!(matches!(
        Api::for_mock(&server.uri())
            .unwrap()
            .list_files(&session(), "", false)
            .await,
        Err(Error::Protocol)
    ));
}

#[tokio::test]
async fn queue_rejects_insecure_url_and_persists_failure_without_secrets() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("downloads.json");
    let center = mokawa::transfers::TransferCenter::new(path.clone());
    center.restore().await.unwrap();
    let record = TransferRecord {
        id: "test-download".into(),
        file_id: "a".into(),
        file_name: "safe.txt".into(),
        completed: 0,
        total: None,
        phase: TransferPhase::Queued,
    };
    center
        .run(
            record,
            "http://example.invalid/signed?secret=should-not-persist".into(),
            directory.path().to_path_buf(),
        )
        .await;
    let snapshot = center.snapshot();
    assert_eq!(snapshot.records[0].phase, TransferPhase::Failed);
    assert!(snapshot.errors["test-download"].contains("HTTPS"));
    assert!(
        !std::fs::read_to_string(&path)
            .unwrap()
            .contains("should-not-persist")
    );
    assert!(!directory.path().join("safe.txt").exists());
    assert!(!directory.path().join("safe.txt.mokawa.part").exists());
}

#[tokio::test]
async fn preferences_round_trip_and_corruption_is_reported() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("prefs.json");
    let defaults = mokawa::settings::Preferences::load(&path).await.unwrap();
    assert!(!defaults.hide_details);
    mokawa::settings::Preferences {
        compact_rows: true,
        hide_details: true,
    }
    .save(&path)
    .await
    .unwrap();
    let saved = mokawa::settings::Preferences::load(&path).await.unwrap();
    assert!(saved.compact_rows && saved.hide_details);
    std::fs::write(&path, "broken").unwrap();
    assert!(mokawa::settings::Preferences::load(&path).await.is_err());
}

#[test]
fn sign_out_invalidates_pending_session_writes() {
    let old = mokawa::credentials::generation();
    mokawa::credentials::invalidate_pending_writes();
    assert!(mokawa::credentials::generation() > old);
    assert!(matches!(
        mokawa::credentials::save_if_current(&session(), old),
        Err(Error::Cancelled)
    ));
}
