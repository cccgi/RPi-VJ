use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use vj_broker::auth::{AuthError, PortalAuthManager};
use vj_broker::http::run_http_server;
use vj_broker::socket::BrokerContext;
use vj_broker::state::StateStore;
use vj_broker::supervisor::Supervisor;
use vj_broker::upload::{sanitize_and_validate_filename, UploadError, UploadManager};
use vj_storage::disk::MockDiskSpaceChecker;
use vj_storage::{JobState, ShowStorageManager, StorageConfig};

fn calculate_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[test]
fn test_path_traversal_rejection_comprehensive() {
    // Malicious traversal patterns
    let bad_names = [
        "../test.mp4",
        "..\\test.mp4",
        "../../etc/passwd.mp4",
        "/var/lib/evil.mp4",
        "C:\\Windows\\system32\\cmd.exe.mp4",
        "foo/bar.mp4",
        "foo\\bar.mp4",
        "clip\0bad.mp4",
        "clip\r\nbad.mp4",
        "CON.mp4",
        "NUL.mp4",
        "AUX.mp4",
        "script.sh",
        "malware.exe",
        "payload.py",
        "test.mp4:Zone.Identifier",
        "..",
        "",
    ];

    for name in bad_names {
        let res = sanitize_and_validate_filename(name);
        assert!(
            res.is_err(),
            "Expected '{}' to be rejected for path traversal / invalidity, but it was accepted",
            name
        );
    }

    // Valid filenames
    let good_names = [
        "loop01.mp4",
        "visualizer_seam_2026.mov",
        "acid-techno-4k.mkv",
        "clip.1080p.webm",
        "show-manifest.json",
        "thumb_01.jpg",
        "overlay.png",
    ];

    for name in good_names {
        let res = sanitize_and_validate_filename(name);
        assert!(
            res.is_ok(),
            "Expected valid filename '{}' to be accepted, but got: {:?}",
            name,
            res
        );
    }
}

#[test]
fn test_upload_resumption_and_chunk_verification() {
    let temp = TempDir::new().unwrap();
    let storage_config = StorageConfig::for_testing(temp.path());
    let mock_checker = Arc::new(MockDiskSpaceChecker::new(
        50 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
    ));
    let storage_mgr = ShowStorageManager::with_checker(storage_config, mock_checker).unwrap();

    let mut upload_mgr = UploadManager::new();

    // Prepare 4 chunks of 16 bytes = 64 bytes total
    let total_size = 64u64;
    let chunk_size = 16u64;
    let total_chunks = 4;

    let chunk0 = b"AAAA0000BBBB1111";
    let chunk1 = b"CCCC2222DDDD3333";
    let chunk2 = b"EEEE4444FFFF5555";
    let chunk3 = b"GGGG6666HHHH7777";

    let mut full_file = Vec::new();
    full_file.extend_from_slice(chunk0);
    full_file.extend_from_slice(chunk1);
    full_file.extend_from_slice(chunk2);
    full_file.extend_from_slice(chunk3);
    let full_sha256 = calculate_sha256(&full_file);

    let init = upload_mgr
        .init_upload(
            "psytrance-loop.mp4",
            total_size,
            chunk_size,
            total_chunks,
            Some(full_sha256.clone()),
            &storage_mgr,
        )
        .unwrap();
    let upload_id = init.upload_id;

    // 1. Upload chunk 0 with valid checksum
    let c0_sha = calculate_sha256(chunk0);
    let res0 = upload_mgr
        .receive_chunk(
            upload_id,
            0,
            0,
            chunk0,
            Some(&c0_sha),
            &storage_mgr,
        )
        .unwrap();
    assert_eq!(res0.received_chunks, vec![0]);
    assert_eq!(res0.progress_percent, 25.0);

    // 2. Upload chunk 1 with CORRUPTED checksum -> must be rejected
    let bad_sha = "0000000000000000000000000000000000000000000000000000000000000000";
    let corrupt_res = upload_mgr.receive_chunk(
        upload_id,
        1,
        16,
        chunk1,
        Some(bad_sha),
        &storage_mgr,
    );
    assert!(matches!(
        corrupt_res,
        Err(UploadError::ChunkChecksumMismatch { .. })
    ));

    // Staging progress should still only have chunk 0
    let status_after_corrupt = upload_mgr.get_status(upload_id).unwrap();
    assert_eq!(status_after_corrupt.received_chunks, vec![0]);

    // 3. Simulate connection drop and out-of-order upload resumption: upload chunk 3
    let c3_sha = calculate_sha256(chunk3);
    let res3 = upload_mgr
        .receive_chunk(
            upload_id,
            3,
            48,
            chunk3,
            Some(&c3_sha),
            &storage_mgr,
        )
        .unwrap();
    assert_eq!(res3.received_chunks, vec![0, 3]);
    assert_eq!(res3.progress_percent, 50.0);

    // 4. Query status for resumption: client sees [0, 3] received, missing [1, 2]
    let resume_status = upload_mgr.get_status(upload_id).unwrap();
    assert_eq!(resume_status.received_chunks, vec![0, 3]);

    // 5. Complete fails before missing chunks uploaded
    let premature_comp = upload_mgr.complete_upload(upload_id, None, &storage_mgr);
    assert!(matches!(
        premature_comp,
        Err(UploadError::IncompleteUpload { .. })
    ));

    // 6. Resume: upload missing chunks 1 and 2
    let c1_sha = calculate_sha256(chunk1);
    let _ = upload_mgr
        .receive_chunk(
            upload_id,
            1,
            16,
            chunk1,
            Some(&c1_sha),
            &storage_mgr,
        )
        .unwrap();

    let c2_sha = calculate_sha256(chunk2);
    let res2 = upload_mgr
        .receive_chunk(
            upload_id,
            2,
            32,
            chunk2,
            Some(&c2_sha),
            &storage_mgr,
        )
        .unwrap();
    assert_eq!(res2.received_chunks, vec![0, 1, 2, 3]);
    assert!(res2.is_complete);

    // 7. Complete upload and verify file checksum
    let completed_job = upload_mgr
        .complete_upload(upload_id, Some(&full_sha256), &storage_mgr)
        .unwrap();
    assert_eq!(completed_job.state, JobState::Ready);
    assert_eq!(completed_job.progress_percent, 100.0);
    assert_eq!(
        completed_job.details.get("sha256").unwrap(),
        &full_sha256
    );
}

#[test]
fn test_auth_rate_limiting_and_token_flow() {
    let temp = TempDir::new().unwrap();
    let auth_path = temp.path().join("portal-auth.json");

    let mut auth_mgr = PortalAuthManager::new(&auth_path).unwrap();
    auth_mgr.set_setup_code("RPI-VJ-SEC1-SEC2").unwrap();

    let attacker_ip = "192.168.1.100";
    let valid_ip = "192.168.1.101";

    // 5 failed login attempts
    for _ in 0..5 {
        let res = auth_mgr.login("BAD-CODE", attacker_ip);
        assert!(matches!(res, Err(AuthError::InvalidCredentials)));
    }

    // 6th attempt is blocked by rate-limiter
    let rate_limited_res = auth_mgr.login("BAD-CODE", attacker_ip);
    assert!(matches!(
        rate_limited_res,
        Err(AuthError::RateLimited { .. })
    ));

    // Even valid code is rate-limited for attacker IP
    let valid_blocked = auth_mgr.login("RPI-VJ-SEC1-SEC2", attacker_ip);
    assert!(matches!(
        valid_blocked,
        Err(AuthError::RateLimited { .. })
    ));

    // Valid IP logs in successfully
    let token_res = auth_mgr.login("RPI-VJ-SEC1-SEC2", valid_ip);
    assert!(token_res.is_ok());
    let token = token_res.unwrap();
    assert!(token.starts_with("vj_tok_"));
    assert!(auth_mgr.validate_token(&token));

    // Revoke token
    let revoke_res = auth_mgr.revoke_token(&token).unwrap();
    assert!(revoke_res);
    assert!(!auth_mgr.validate_token(&token));
}

#[test]
fn test_http_portal_end_to_end_api() {
    let temp = TempDir::new().unwrap();
    let storage_config = StorageConfig::for_testing(temp.path());
    let mock_checker = Arc::new(MockDiskSpaceChecker::new(
        50 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
    ));
    let storage_mgr = Arc::new(ShowStorageManager::with_checker(storage_config, mock_checker).unwrap());

    let auth_path = temp.path().join("portal-auth.json");
    let mut auth_mgr_inner = PortalAuthManager::new(&auth_path).unwrap();
    auth_mgr_inner.set_setup_code("RPI-VJ-TEST-CODE").unwrap();
    let auth_mgr = Arc::new(Mutex::new(auth_mgr_inner));

    let state_store = StateStore::new(None);
    let supervisor = Supervisor::new(None, None, std::time::Instant::now(), 1700000000);

    let ctx = Arc::new(BrokerContext::with_portal(
        state_store,
        supervisor,
        Some(storage_mgr.clone()),
        Some(auth_mgr),
        Some("rpi-vj.local".to_string()),
    ));

    let running = Arc::new(AtomicBool::new(true));
    let http_addr = "127.0.0.1:18090";

    let server_ctx = Arc::clone(&ctx);
    let server_running = Arc::clone(&running);
    let server_handle = thread::spawn(move || {
        let _ = run_http_server(http_addr, None, server_ctx, server_running);
    });

    thread::sleep(Duration::from_millis(100));

    // Helper HTTP POST
    let send_http_request = |req: &str| -> (u16, serde_json::Value, String) {
        let mut stream = TcpStream::connect(http_addr).unwrap();
        stream.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = stream.read_to_string(&mut resp);

        let status_code = resp
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);

        let body_str = resp.split("\r\n\r\n").nth(1).unwrap_or("");
        let json_val = serde_json::from_str(body_str).unwrap_or(serde_json::json!({}));
        if status_code == 0 {
            eprintln!("[DEBUG] req failed. req=\n{}\nresp=\n{}", req, resp);
        }
        (status_code, json_val, body_str.to_string())
    };

    // 1. GET /portal/ -> HTML UI
    let req_portal = "GET /portal/ HTTP/1.1\r\nHost: 127.0.0.1:18090\r\nConnection: close\r\n\r\n";
    let (code, _, body_text) = send_http_request(req_portal);
    assert_eq!(code, 200);
    assert!(body_text.contains("RPi-VJ Appliance — Ingest Portal"));

    // 2. Unauthenticated request to /api/v1/portal/upload/init -> 401
    let unauth_body = serde_json::json!({"filename":"a.mp4"}).to_string();
    let req_unauth = format!(
        "POST /api/v1/portal/upload/init HTTP/1.1\r\nHost: 127.0.0.1:18090\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        unauth_body.len(), unauth_body
    );
    let (code_unauth, _, _) = send_http_request(&req_unauth);
    assert_eq!(code_unauth, 401);

    // 3. Login with Setup Code
    let login_body = serde_json::json!({"code":"RPI-VJ-TEST-CODE"}).to_string();
    let req_login = format!(
        "POST /api/v1/portal/auth/login HTTP/1.1\r\nHost: 127.0.0.1:18090\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        login_body.len(), login_body
    );
    let (login_code, login_json, _) = send_http_request(&req_login);
    assert_eq!(login_code, 200);
    let token = login_json.get("token").and_then(|v| v.as_str()).unwrap().to_string();
    assert!(token.starts_with("vj_tok_"));

    // 4. GET /api/v1/portal/status with Bearer token
    let req_status = format!(
        "GET /api/v1/portal/status HTTP/1.1\r\nHost: 127.0.0.1:18090\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",
        token
    );
    let (status_code, status_json, _) = send_http_request(&req_status);
    assert_eq!(status_code, 200);
    assert_eq!(status_json.get("hostname").unwrap(), "rpi-vj.local");
    assert!(status_json.get("storage").is_some());

    // 5. Initialize resumable chunked upload
    let chunk_data = b"0123456789ABCDEF"; // 16 bytes
    let total_size = 16u64;
    let init_body = serde_json::json!({
        "filename": "gig-loop-01.mp4",
        "filesize": total_size,
        "chunk_size": 16,
        "total_chunks": 1
    }).to_string();

    let req_init = format!(
        "POST /api/v1/portal/upload/init HTTP/1.1\r\nHost: 127.0.0.1:18090\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        token, init_body.len(), init_body
    );
    let (init_code, init_json, _) = send_http_request(&req_init);
    assert_eq!(init_code, 201);
    let upload_id = init_json.get("upload_id").and_then(|v| v.as_str()).unwrap().to_string();
    let job_id = init_json.get("job_id").and_then(|v| v.as_str()).unwrap().to_string();

    // 6. Upload chunk 0
    let chunk_req = format!(
        "POST /api/v1/portal/upload/chunk?upload_id={}&index=0&offset=0 HTTP/1.1\r\nHost: 127.0.0.1:18090\r\nAuthorization: Bearer {}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        upload_id, token, chunk_data.len()
    );
    let mut chunk_raw_req = chunk_req.into_bytes();
    chunk_raw_req.extend_from_slice(chunk_data);

    let mut stream = TcpStream::connect(http_addr).unwrap();
    stream.write_all(&chunk_raw_req).unwrap();
    let mut chunk_resp_str = String::new();
    stream.read_to_string(&mut chunk_resp_str).unwrap();
    assert!(chunk_resp_str.contains("200 OK"));

    // 7. Complete upload
    let comp_body = serde_json::json!({ "upload_id": upload_id }).to_string();
    let req_comp = format!(
        "POST /api/v1/portal/upload/complete HTTP/1.1\r\nHost: 127.0.0.1:18090\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        token, comp_body.len(), comp_body
    );
    let (comp_code, comp_json, _) = send_http_request(&req_comp);
    assert_eq!(comp_code, 200);
    assert_eq!(comp_json.get("status").unwrap(), "complete");

    // 8. Activate ready generation
    let act_body = serde_json::json!({ "job_id": job_id }).to_string();
    let req_act = format!(
        "POST /api/v1/portal/activate HTTP/1.1\r\nHost: 127.0.0.1:18090\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        token, act_body.len(), act_body
    );
    let (act_code, act_json, _) = send_http_request(&req_act);
    assert_eq!(act_code, 200);
    assert_eq!(act_json.get("status").unwrap(), "activated");
    let active_gen = act_json.get("active_generation").and_then(|v| v.as_str()).unwrap();
    assert!(active_gen.starts_with("gen-"));

    // Cleanup
    running.store(false, Ordering::Relaxed);
    let _ = TcpStream::connect(http_addr);
    let _ = server_handle.join();
}
