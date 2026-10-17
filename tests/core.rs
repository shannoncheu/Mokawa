use mokawa::{
    api::Api,
    auth::{AuthMethod, Registration, Session, now},
    error::Error,
    files::ConfirmedDeletion,
    models::{File, TransferPhase, TransferRecord},
    state::BrowserState,
};
use wiremock::matchers::body_json;
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
        auth_method: AuthMethod::OAuth,
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
    let legacy = r#"{"access_token":"a","refresh_token":"r","captcha_token":"c","device_id":"d","expires_at":99}"#;
    let restored: Session = serde_json::from_str(legacy).unwrap();
    assert_eq!(restored.auth_method, AuthMethod::OAuth);
}
#[test]
fn drive_quota_accepts_server_byte_counts_without_inference() {
    use mokawa::models::DriveAbout;
    let about: DriveAbout =
        serde_json::from_str(r#"{"quota":{"limit":"1099511627776","usage":536870912}}"#).unwrap();
    assert_eq!(about.quota.unwrap().limit, Some(1_099_511_627_776));
    let unknown: DriveAbout = serde_json::from_str(r#"{"quota":{}}"#).unwrap();
    assert_eq!(unknown.quota.unwrap().usage, None);
    assert!(serde_json::from_str::<DriveAbout>(r#"{"quota":{"usage":-1}}"#).is_err());
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
async fn offline_submission_uses_pikpak_url_task_endpoint_and_cloud_destination() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/drive/v1/files"))
        .and(header("authorization", "Bearer secret-access"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "kind":"drive#file", "upload_type":"UPLOAD_TYPE_URL",
            "url":{"url":"magnet:?xt=urn:btih:0123456789012345678901234567890123456789"},
            "parent_id":"folder", "folder_type":""
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "task":{"id":"task-1","phase":"PHASE_TYPE_PENDING"}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let task = Api::for_mock(&server.uri())
        .unwrap()
        .create_offline_task(
            &session(),
            "magnet:?xt=urn:btih:0123456789012345678901234567890123456789",
            "folder",
        )
        .await
        .unwrap();
    assert_eq!(task.id, "task-1");
}

#[tokio::test]
async fn offline_retry_checks_remote_phase_and_task_deletion_preserves_files() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/drive/v1/tasks/task-1"))
        .and(query_param("type", "offline"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "task":{"id":"task-1","phase":"PHASE_TYPE_ERROR"}
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/drive/v1/task"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "type":"offline","create_type":"RETRY","id":"task-1"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/drive/v1/tasks"))
        .and(query_param("task_ids", "task-1,task-2"))
        .and(query_param("delete_files", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;
    let api = Api::for_mock(&server.uri()).unwrap();
    api.retry_offline_task(&session(), "task-1").await.unwrap();
    api.delete_offline_tasks(&session(), &["task-1".into(), "task-2".into()])
        .await
        .unwrap();
}

#[tokio::test]
async fn storage_quota_uses_authenticated_drive_about_endpoint() {
    use mokawa::models::DriveAbout;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/drive/v1/about"))
        .and(header("authorization", "Bearer secret-access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "quota": { "limit": "1000", "usage": "250" }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let about: DriveAbout = Api::for_mock(&server.uri())
        .unwrap()
        .drive_about(&session())
        .await
        .unwrap();
    let quota = about.quota.unwrap();
    assert_eq!(quota.limit, Some(1000));
    assert_eq!(quota.usage, Some(250));
}

#[tokio::test]
async fn personal_access_token_is_verified_as_bearer_before_it_can_be_used() {
    use secrecy::ExposeSecret;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/drive/v1/files"))
        .and(query_param("parent_id", ""))
        .and(query_param("limit", "1"))
        .and(header("authorization", "Bearer user-pat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"files":[]})))
        .expect(1)
        .mount(&server)
        .await;

    let session = Session::from_personal_access_token("user-pat".into()).unwrap();
    assert_eq!(session.auth_method, AuthMethod::PersonalAccessToken);
    assert!(!session.expired());
    assert!(!format!("{session:?}").contains("user-pat"));
    assert_eq!(session.access_token.expose_secret(), "user-pat");

    Api::for_mock(&server.uri())
        .unwrap()
        .verify_personal_access_token(&session)
        .await
        .unwrap();
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].headers.contains_key("x-device-id"));
    assert!(!requests[0].headers.contains_key("x-captcha-token"));
}

#[tokio::test]
async fn invalid_or_underprivileged_personal_access_token_is_not_accepted() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/drive/v1/files"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;
    let session = Session::from_personal_access_token("revoked-or-invalid".into()).unwrap();
    assert!(matches!(
        Api::for_mock(&server.uri())
            .unwrap()
            .verify_personal_access_token(&session)
            .await,
        Err(Error::Unauthorized)
    ));
}

#[tokio::test]
async fn personal_access_token_never_uses_oauth_refresh() {
    let server = MockServer::start().await;
    let registration = Registration {
        client_id: "unused-for-pat".into(),
        client_secret: "unused-for-pat".into(),
    };
    let session = Session::from_personal_access_token("user-pat".into()).unwrap();
    assert!(matches!(
        Api::for_mock(&server.uri())
            .unwrap()
            .refresh(&registration, &session)
            .await,
        Err(Error::Unauthorized)
    ));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn personal_access_token_permission_failure_is_actionable() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/drive/v1/files"))
        .respond_with(ResponseTemplate::new(403))
        .expect(1)
        .mount(&server)
        .await;
    let session = Session::from_personal_access_token("limited-pat".into()).unwrap();
    let err = Api::for_mock(&server.uri())
        .unwrap()
        .verify_personal_access_token(&session)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::UnsupportedPermission));
    assert!(err.to_string().contains("令牌不具备"));
}

#[test]
fn personal_access_token_rejects_empty_oversized_and_control_data() {
    assert!(Session::from_personal_access_token("  ".into()).is_err());
    assert!(Session::from_personal_access_token("bad\ntoken".into()).is_err());
    assert!(Session::from_personal_access_token("x".repeat(2049).into()).is_err());
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
    for (status, expected) in [(401, "过期"), (429, "频繁"), (500, "HTTP 500")] {
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
        Err(Error::VerificationRequired(Some(url))) if url == "https://example.test/challenge"
    ));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}
#[tokio::test]
async fn password_login_uses_device_and_registered_client_then_reads_drive() {
    let server = MockServer::start().await;
    let client_id = "registered-client";
    let device_id = "device-123";
    Mock::given(path("/v1/shield/captcha/init"))
        .and(method("POST"))
        .and(query_param("client_id", client_id))
        .and(header("x-device-id", device_id))
        .and(body_json(serde_json::json!({
            "client_id": client_id,
            "device_id": device_id,
            "action": "POST:/v1/auth/signin",
            "meta": {"email": "user@example.test"}
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"captcha_token":"mock-captcha"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/auth/signin"))
        .and(method("POST"))
        .and(query_param("client_id", client_id))
        .and(header("x-device-id", device_id))
        .and(body_json(serde_json::json!({
            "client_id": client_id,
            "client_secret": "registered-secret",
            "grant_type": "password",
            "username": "user@example.test",
            "password": "mock-password",
            "captcha_token": "mock-captcha"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token":"mock-access","refresh_token":"mock-refresh","expires_in":3600
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/drive/v1/files"))
        .and(method("GET"))
        .and(header("authorization", "Bearer mock-access"))
        .and(header("x-device-id", device_id))
        .and(header("x-captcha-token", "mock-captcha"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"files":[]})))
        .expect(1)
        .mount(&server)
        .await;

    let registration =
        Registration::from_values(client_id.into(), "registered-secret".into()).unwrap();
    let api = Api::for_mock(&server.uri()).unwrap();
    assert!(!format!("{registration:?}").contains("registered-secret"));
    let session = api
        .login(
            &registration,
            "user@example.test",
            &"mock-password".into(),
            device_id,
        )
        .await
        .unwrap();
    assert_eq!(api.list_files(&session, "", false).await.unwrap().len(), 0);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3);
    assert!(!format!("{session:?}").contains("mock-access"));
}
#[tokio::test]
async fn refresh_retains_rotates_and_expires() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/auth/token"))
        .and(method("POST"))
        .and(query_param("client_id", "mokawa-test"))
        .and(header("x-device-id", "device"))
        .and(body_json(serde_json::json!({
            "client_id":"mokawa-test",
            "client_secret":"test",
            "grant_type":"refresh_token",
            "refresh_token":"secret-refresh"
        })))
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
        local_path: None,
    }];
    mokawa::download::save_queue(&path, &records).await.unwrap();
    let loaded = mokawa::download::load_queue(&path).await.unwrap();
    assert_eq!(loaded[0].phase, TransferPhase::Interrupted);
    assert_eq!(loaded[0].completed, 12);
}
#[tokio::test]
async fn completed_transfer_keeps_its_local_path_for_open_actions() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("queue.json");
    let records = vec![TransferRecord {
        id: "done".into(),
        file_id: "a".into(),
        file_name: "video.mp4".into(),
        completed: 100,
        total: Some(100),
        phase: TransferPhase::Completed,
        local_path: Some("C:/Users/test/Downloads/video.mp4".into()),
    }];
    mokawa::download::save_queue(&path, &records).await.unwrap();
    let loaded = mokawa::download::load_queue(&path).await.unwrap();
    assert_eq!(
        loaded[0].local_path.as_deref(),
        Some("C:/Users/test/Downloads/video.mp4")
    );
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
        local_path: None,
    };
    let _ = center
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
        ..Default::default()
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

#[test]
fn ime_selection_is_relative_to_replacement_with_non_ascii_prefix() {
    use mokawa::state::composition_selection;
    let document = "目录 old suffix";
    let insertion = "目录 ".len();
    let replacement = "界😀";
    let range = composition_selection(replacement, 1..3, insertion);
    let updated = document.replacen("old", replacement, 1);
    assert_eq!(&updated[range], "😀");
    let clamped = composition_selection(replacement, 0..usize::MAX, insertion);
    assert_eq!(&updated[clamped], replacement);
}

#[tokio::test]
async fn sign_out_cancels_download_before_worker_registration() {
    let directory = tempfile::tempdir().unwrap();
    let center = mokawa::transfers::TransferCenter::new(directory.path().join("queue.json"));
    let scope = tokio_util::sync::CancellationToken::new();
    scope.cancel();
    let record = TransferRecord {
        id: "cancelled-before-start".into(),
        file_id: "a".into(),
        file_name: "never-created.txt".into(),
        completed: 0,
        total: Some(42),
        phase: TransferPhase::Queued,
        local_path: None,
    };
    let _ = center
        .run_cancellable(
            record,
            "https://example.invalid/not-requested".into(),
            directory.path().to_path_buf(),
            scope.child_token(),
        )
        .await;
    assert_eq!(center.snapshot().records[0].phase, TransferPhase::Cancelled);
    assert!(!directory.path().join("never-created.txt").exists());
    assert!(
        !directory
            .path()
            .join("never-created.txt.mokawa.part")
            .exists()
    );
    center.wait_idle().await;
}

#[tokio::test]
async fn oversized_history_is_rejected_before_json_decode() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tasks.json");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(mokawa::download::MAX_QUEUE_BYTES + 1).unwrap();
    assert!(matches!(
        mokawa::download::load_queue(&path).await,
        Err(Error::Invalid(_))
    ));
}

#[test]
fn large_file_list_indexes_preserve_filtering_and_folder_order() {
    let browser = mokawa::state::BrowserState {
        files: (0..100_000)
            .map(|index| mokawa::models::File {
                id: index.to_string(),
                name: format!("File {index:06}"),
                kind: if index % 2 == 0 {
                    "drive#folder"
                } else {
                    "drive#file"
                }
                .into(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let start = std::time::Instant::now();
    let indices = browser.visible_indices("File 099", true);
    eprintln!("100000-file filter/sort: {:?}", start.elapsed());
    assert_eq!(indices.len(), 1000);
    assert!(indices[..500].iter().all(|&i| browser.files[i].is_folder()));
    assert!(
        indices[500..]
            .iter()
            .all(|&i| !browser.files[i].is_folder())
    );
    assert_eq!(browser.files[indices[0]].name, "File 099998");
}

#[tokio::test]
async fn theme_preferences_migrate_and_persist_all_modes() {
    use mokawa::settings::{Preferences, ThemeMode};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    std::fs::write(&path, r#"{"compact_rows":true,"hide_details":false}"#).unwrap();
    let old = Preferences::load(&path).await.unwrap();
    assert_eq!(old.theme, ThemeMode::System);
    assert!(old.compact_rows);
    for mode in [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark] {
        Preferences {
            theme: mode,
            ..old.clone()
        }
        .save(&path)
        .await
        .unwrap();
        assert_eq!(Preferences::load(&path).await.unwrap().theme, mode);
    }
    assert!(!ThemeMode::System.is_dark(false));
    assert!(ThemeMode::System.is_dark(true));
    assert!(!ThemeMode::Light.is_dark(true));
    assert!(ThemeMode::Dark.is_dark(false));
}

#[tokio::test]
async fn oversized_metadata_response_is_rejected_without_exposing_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("s".repeat(4 * 1024 * 1024 + 1)))
        .expect(1)
        .mount(&server)
        .await;
    let error = Api::for_mock(&server.uri())
        .unwrap()
        .list_files(&session(), "", false)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        Error::Invalid("接口响应过大，请缩小请求范围")
    ));
}

#[tokio::test]
async fn oversized_pagination_token_stops_before_another_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [], "next_page_token": "s".repeat(4097)
        })))
        .expect(1)
        .mount(&server)
        .await;
    let error = Api::for_mock(&server.uri())
        .unwrap()
        .list_files(&session(), "", false)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Invalid("分页数据超出安全范围")));
}
