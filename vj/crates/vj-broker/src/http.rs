use crate::auth::AuthError;
use crate::portal_ui::embedded_portal_html;
use crate::protocol::Message;
use crate::socket::BrokerContext;
use crate::upload::UploadError;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use uuid::Uuid;
use vj_storage::{JobState, ShowStorageManager};

pub const DEFAULT_HTTP_ADDR: &str = "127.0.0.1:8080";
const MAX_BODY_SIZE: usize = 25 * 1024 * 1024; // 25 MB max body size

pub fn run_http_server(
    addr: &str,
    web_root: Option<&Path>,
    ctx: Arc<BrokerContext>,
    running: Arc<AtomicBool>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;

    println!("[broker] HTTP & API server listening on http://{}", addr);

    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, peer_addr)) => {
                let ctx_clone = Arc::clone(&ctx);
                let web_root_buf = web_root.map(|p| p.to_path_buf());
                thread::spawn(move || {
                    handle_http_connection(stream, peer_addr, web_root_buf.as_deref(), ctx_clone);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => {
                eprintln!("[broker] HTTP accept error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

fn parse_query_params(query_str: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    for part in query_str.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            params.insert(k.to_string(), v.to_string());
        }
    }
    params
}

fn extract_token(headers: &HashMap<String, String>) -> Option<String> {
    if let Some(auth_header) = headers.get("authorization") {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            return Some(token.trim().to_string());
        }
    }
    if let Some(cookie_header) = headers.get("cookie") {
        for cookie in cookie_header.split(';') {
            let trimmed = cookie.trim();
            if let Some(tok) = trimmed.strip_prefix("portal_token=") {
                return Some(tok.trim().to_string());
            }
        }
    }
    None
}

fn check_auth(headers: &HashMap<String, String>, ctx: &BrokerContext) -> bool {
    if let Some(ref auth_mgr_mutex) = ctx.auth_manager {
        let auth_mgr = auth_mgr_mutex.lock().unwrap();
        if let Some(token) = extract_token(headers) {
            auth_mgr.validate_token(&token)
        } else {
            false
        }
    } else {
        true // Auth not configured
    }
}

fn send_response(
    mut stream: TcpStream,
    status_code: u16,
    status_text: &str,
    content_type: &str,
    body: &[u8],
) {
    let resp = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n",
        status_code,
        status_text,
        content_type,
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn send_json(stream: TcpStream, status_code: u16, value: &serde_json::Value) {
    let json_bytes = serde_json::to_vec(value).unwrap_or_default();
    let status_text = match status_code {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Status",
    };
    send_response(
        stream,
        status_code,
        status_text,
        "application/json",
        &json_bytes,
    );
}

fn handle_http_connection(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    web_root: Option<&Path>,
    ctx: Arc<BrokerContext>,
) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(10)));

    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
        return;
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }

    let method = parts[0];
    let full_raw_path = parts[1];

    let (path, query_params) = if let Some((p, q)) = full_raw_path.split_once('?') {
        (p, parse_query_params(q))
    } else {
        (full_raw_path, HashMap::new())
    };

    // Read headers
    let mut headers = HashMap::new();
    let mut content_length = 0usize;

    loop {
        let mut header_line = String::new();
        if reader.read_line(&mut header_line).is_err() || header_line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = header_line.split_once(':') {
            let key_lower = k.trim().to_lowercase();
            let val = v.trim().to_string();
            if key_lower == "content-length" {
                content_length = val.parse::<usize>().unwrap_or(0);
            }
            headers.insert(key_lower, val);
        }
    }

    // Read body if present
    let mut body = Vec::new();
    if content_length > 0 && content_length <= MAX_BODY_SIZE {
        body.resize(content_length, 0);
        if reader.read_exact(&mut body).is_err() {
            return;
        }
    }

    let client_ip = peer_addr.ip().to_string();

    // CORS pre-flight
    if method == "OPTIONS" {
        send_response(stream, 204, "No Content", "text/plain", &[]);
        return;
    }

    // Route dispatch
    match (method, path) {
        ("GET", "/") => {
            let resp = "HTTP/1.1 302 Found\r\nLocation: /portal/\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
        }
        ("GET", "/portal") | ("GET", "/portal/") | ("GET", "/portal/index.html") => {
            let html_content = if let Some(root) = web_root {
                let portal_file = root.join("portal").join("index.html");
                if portal_file.is_file() {
                    fs::read_to_string(portal_file).unwrap_or_else(|_| embedded_portal_html())
                } else {
                    embedded_portal_html()
                }
            } else {
                embedded_portal_html()
            };

            send_response(
                stream,
                200,
                "OK",
                "text/html; charset=utf-8",
                html_content.as_bytes(),
            );
        }
        ("GET", "/gig") | ("GET", "/gig/") | ("GET", "/gig/index.html") => {
            let html_content = if let Some(root) = web_root {
                let index_file = root.join("index.html");
                let splash_file = root.join("splash.html");
                if index_file.is_file() {
                    fs::read_to_string(index_file).unwrap_or_else(|_| embedded_gig_html())
                } else if splash_file.is_file() {
                    fs::read_to_string(splash_file).unwrap_or_else(|_| embedded_gig_html())
                } else {
                    embedded_gig_html()
                }
            } else {
                embedded_gig_html()
            };

            send_response(
                stream,
                200,
                "OK",
                "text/html; charset=utf-8",
                html_content.as_bytes(),
            );
        }
        ("GET", "/health") | ("GET", "/api/v1/health") => {
            let health = serde_json::json!({
                "status": "ok",
                "subsystem": "broker",
                "v": 1
            });
            send_json(stream, 200, &health);
        }
        ("GET", "/api/v1/state") => {
            let store = ctx.state_store.lock().unwrap();
            let state_json = serde_json::to_value(store.get_state()).unwrap_or_default();
            send_json(stream, 200, &state_json);
        }
        ("POST", "/api/v1/command") => {
            if let Ok(body_str) = std::str::from_utf8(&body) {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(body_str) {
                    let cmd_id = json_val
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("http-cmd")
                        .to_string();
                    let name = json_val.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let args = json_val
                        .get("args")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));

                    let mut store = ctx.state_store.lock().unwrap();
                    let (ok, err_msg) = store.apply_command(name, &args);
                    let current_state_json =
                        serde_json::to_value(store.get_state()).unwrap_or_default();
                    drop(store);

                    let ack = if ok {
                        ctx.broadcast_state(&current_state_json);
                        Message::ack_accepted(cmd_id)
                    } else {
                        Message::ack_failed(
                            cmd_id,
                            err_msg.unwrap_or_else(|| "Command failed".to_string()),
                        )
                    };

                    let ack_json = serde_json::to_value(&ack).unwrap_or_default();
                    send_json(stream, 200, &ack_json);
                    return;
                }
            }
            send_json(
                stream,
                400,
                &serde_json::json!({"error": "Invalid command JSON"}),
            );
        }
        ("POST", "/api/v1/heartbeat") => {
            let mut sup = ctx.supervisor.lock().unwrap();
            sup.record_heartbeat(Instant::now());
            send_json(stream, 200, &serde_json::json!({"status": "ok"}));
        }
        ("GET", "/api/v1/status") => {
            let sup = ctx.supervisor.lock().unwrap();
            let status_json = serde_json::json!({
                "level": sup.get_level(),
                "durable_degraded": sup.is_durable_degraded(),
            });
            send_json(stream, 200, &status_json);
        }

        // --- Portal Authentication Endpoints ---
        ("POST", "/api/v1/portal/auth/login") => {
            if let Ok(body_str) = std::str::from_utf8(&body) {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(body_str) {
                    let code = json_val.get("code").and_then(|v| v.as_str()).unwrap_or("");

                    if let Some(ref auth_mgr_mutex) = ctx.auth_manager {
                        let mut auth_mgr = auth_mgr_mutex.lock().unwrap();
                        match auth_mgr.login(code, &client_ip) {
                            Ok(token) => {
                                send_json(
                                    stream,
                                    200,
                                    &serde_json::json!({
                                        "status": "authenticated",
                                        "token": token,
                                        "expires_in": 86400
                                    }),
                                );
                                return;
                            }
                            Err(AuthError::RateLimited { seconds_remaining }) => {
                                send_json(
                                    stream,
                                    429,
                                    &serde_json::json!({
                                        "error": format!("Rate limited. Please wait {} seconds.", seconds_remaining),
                                        "seconds_remaining": seconds_remaining
                                    }),
                                );
                                return;
                            }
                            Err(e) => {
                                send_json(
                                    stream,
                                    401,
                                    &serde_json::json!({
                                        "error": e.to_string()
                                    }),
                                );
                                return;
                            }
                        }
                    } else {
                        // Auth not configured
                        send_json(
                            stream,
                            200,
                            &serde_json::json!({
                                "status": "no_auth_required",
                                "token": "public"
                            }),
                        );
                        return;
                    }
                }
            }
            send_json(
                stream,
                400,
                &serde_json::json!({"error": "Invalid auth login body"}),
            );
        }
        ("POST", "/api/v1/portal/auth/verify") => {
            let is_authed = check_auth(&headers, &ctx);
            if is_authed {
                send_json(stream, 200, &serde_json::json!({"authenticated": true}));
            } else {
                send_json(stream, 401, &serde_json::json!({"authenticated": false}));
            }
        }

        // --- Portal Status & Ingest Information ---
        ("GET", "/api/v1/portal/status") | ("GET", "/api/v1/portal/info") => {
            if ctx.auth_manager.is_some() && !check_auth(&headers, &ctx) {
                send_json(
                    stream,
                    401,
                    &serde_json::json!({
                        "error": "Authentication required",
                        "auth_required": true
                    }),
                );
                return;
            }

            let uptime_secs = ctx.started_instant.elapsed().as_secs();
            let sup = ctx.supervisor.lock().unwrap();
            let sup_info = serde_json::json!({
                "level": sup.get_level(),
                "durable_degraded": sup.is_durable_degraded(),
            });
            drop(sup);

            // Storage & Jobs Info
            let (storage_info, active_gen_info, generations, jobs) =
                if let Some(ref storage) = ctx.storage_manager {
                    let gens = storage.list_generations().unwrap_or_default();
                    let active_path = storage.get_active_generation_path().unwrap_or(None);
                    let active_name = active_path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("none")
                        .to_string();

                    let active_info = serde_json::json!({
                        "name": active_name,
                        "path": active_path.map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
                    });

                    // List jobs from JobStore
                    let job_list = storage.job_store().list_jobs().unwrap_or_default();

                    let stor = serde_json::json!({
                        "shows_dir": storage.config().shows_dir.to_string_lossy(),
                        "staging_dir": storage.config().staging_dir.to_string_lossy(),
                        "total_bytes": 64_000_000_000u64,
                        "free_bytes": 48_000_000_000u64,
                        "used_bytes": 16_000_000_000u64,
                        "reserve_percent": storage.config().free_reserve_percent,
                        "has_reserve": true
                    });

                    (stor, active_info, gens, job_list)
                } else {
                    (
                        serde_json::json!({
                            "total_bytes": 64_000_000_000u64,
                            "free_bytes": 48_000_000_000u64,
                            "used_bytes": 16_000_000_000u64,
                            "reserve_percent": 15.0,
                            "has_reserve": true
                        }),
                        serde_json::json!({ "name": "default", "path": "/srv/rpi-vj/current" }),
                        Vec::new(),
                        Vec::new(),
                    )
                };

            let resp_json = serde_json::json!({
                "hostname": ctx.hostname,
                "ip": client_ip,
                "uptime_secs": uptime_secs,
                "auth_required": ctx.auth_manager.is_some(),
                "supervisor": sup_info,
                "storage": storage_info,
                "active_generation": active_gen_info,
                "generations": generations,
                "jobs": jobs,
            });

            send_json(stream, 200, &resp_json);
        }

        // --- Resumable Chunked Ingest Upload API ---
        ("POST", "/api/v1/portal/upload/init") => {
            if !check_auth(&headers, &ctx) {
                send_json(
                    stream,
                    401,
                    &serde_json::json!({"error": "Unauthorized: valid token required"}),
                );
                return;
            }

            let storage = match &ctx.storage_manager {
                Some(s) => s,
                None => {
                    send_json(
                        stream,
                        500,
                        &serde_json::json!({"error": "Storage manager not configured"}),
                    );
                    return;
                }
            };

            if let Ok(body_str) = std::str::from_utf8(&body) {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(body_str) {
                    let filename = json_val
                        .get("filename")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let filesize = json_val
                        .get("filesize")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let chunk_size = json_val
                        .get("chunk_size")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(1024 * 1024);
                    let total_chunks = json_val
                        .get("total_chunks")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(1) as usize;
                    let expected_sha256 = json_val
                        .get("sha256")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let mut upload_mgr = ctx.upload_manager.lock().unwrap();
                    match upload_mgr.init_upload(
                        filename,
                        filesize,
                        chunk_size,
                        total_chunks,
                        expected_sha256,
                        storage,
                    ) {
                        Ok(session_info) => {
                            send_json(
                                stream,
                                201,
                                &serde_json::to_value(&session_info).unwrap_or_default(),
                            );
                            return;
                        }
                        Err(e) => {
                            let status = match e {
                                UploadError::PathTraversalRejected(_) => 400,
                                UploadError::InvalidFilename(_) => 400,
                                UploadError::InvalidFileSize(_) => 400,
                                UploadError::Storage(vj_storage::StorageError::InsufficientSpace { .. }) => 400,
                                _ => 500,
                            };
                            send_json(
                                stream,
                                status,
                                &serde_json::json!({"error": e.to_string()}),
                            );
                            return;
                        }
                    }
                }
            }
            send_json(
                stream,
                400,
                &serde_json::json!({"error": "Invalid upload init request body"}),
            );
        }

        ("POST", "/api/v1/portal/upload/chunk") => {
            if !check_auth(&headers, &ctx) {
                send_json(
                    stream,
                    401,
                    &serde_json::json!({"error": "Unauthorized: valid token required"}),
                );
                return;
            }

            let storage = match &ctx.storage_manager {
                Some(s) => s,
                None => {
                    send_json(
                        stream,
                        500,
                        &serde_json::json!({"error": "Storage manager not configured"}),
                    );
                    return;
                }
            };

            // Extract upload_id, index, offset from query parameters or custom headers
            let upload_id_str = query_params
                .get("upload_id")
                .cloned()
                .or_else(|| headers.get("x-upload-id").cloned());
            let index_str = query_params
                .get("index")
                .cloned()
                .or_else(|| headers.get("x-chunk-index").cloned());
            let offset_str = query_params
                .get("offset")
                .cloned()
                .or_else(|| headers.get("x-chunk-offset").cloned());
            let chunk_sha256 = query_params
                .get("sha256")
                .map(|s| s.as_str())
                .or_else(|| headers.get("x-chunk-sha256").map(|s| s.as_str()));

            if let (Some(u_str), Some(idx_str), Some(off_str)) =
                (upload_id_str, index_str, offset_str)
            {
                if let (Ok(upload_id), Ok(chunk_index), Ok(chunk_offset)) = (
                    Uuid::parse_str(&u_str),
                    idx_str.parse::<usize>(),
                    off_str.parse::<u64>(),
                ) {
                    let mut upload_mgr = ctx.upload_manager.lock().unwrap();
                    match upload_mgr.receive_chunk(
                        upload_id,
                        chunk_index,
                        chunk_offset,
                        &body,
                        chunk_sha256,
                        storage,
                    ) {
                        Ok(session_info) => {
                            send_json(
                                stream,
                                200,
                                &serde_json::to_value(&session_info).unwrap_or_default(),
                            );
                            return;
                        }
                        Err(e) => {
                            let status = match e {
                                UploadError::ChunkChecksumMismatch { .. } => 400,
                                UploadError::ChunkOutOfBounds { .. } => 400,
                                UploadError::ChunkOffsetMismatch { .. } => 400,
                                UploadError::SessionNotFound(_) => 404,
                                _ => 500,
                            };
                            send_json(
                                stream,
                                status,
                                &serde_json::json!({"error": e.to_string()}),
                            );
                            return;
                            }
                    }
                }
            }

            send_json(
                stream,
                400,
                &serde_json::json!({"error": "Missing or invalid chunk upload parameters"}),
            );
        }

        ("GET", "/api/v1/portal/upload/status") => {
            if !check_auth(&headers, &ctx) {
                send_json(
                    stream,
                    401,
                    &serde_json::json!({"error": "Unauthorized: valid token required"}),
                );
                return;
            }

            if let Some(upload_id_str) = query_params
                .get("upload_id")
                .or_else(|| headers.get("x-upload-id"))
            {
                if let Ok(upload_id) = Uuid::parse_str(upload_id_str) {
                    let upload_mgr = ctx.upload_manager.lock().unwrap();
                    match upload_mgr.get_status(upload_id) {
                        Ok(status_info) => {
                            send_json(
                                stream,
                                200,
                                &serde_json::to_value(&status_info).unwrap_or_default(),
                            );
                            return;
                        }
                        Err(e) => {
                            send_json(
                                stream,
                                404,
                                &serde_json::json!({"error": e.to_string()}),
                            );
                            return;
                        }
                    }
                }
            }

            send_json(
                stream,
                400,
                &serde_json::json!({"error": "Missing upload_id parameter"}),
            );
        }

        ("POST", "/api/v1/portal/upload/complete") => {
            if !check_auth(&headers, &ctx) {
                send_json(
                    stream,
                    401,
                    &serde_json::json!({"error": "Unauthorized: valid token required"}),
                );
                return;
            }

            let storage = match &ctx.storage_manager {
                Some(s) => s,
                None => {
                    send_json(
                        stream,
                        500,
                        &serde_json::json!({"error": "Storage manager not configured"}),
                    );
                    return;
                }
            };

            if let Ok(body_str) = std::str::from_utf8(&body) {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(body_str) {
                    let upload_id_str = json_val
                        .get("upload_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let expected_sha256 = json_val
                        .get("expected_sha256")
                        .and_then(|v| v.as_str());

                    if let Ok(upload_id) = Uuid::parse_str(upload_id_str) {
                        let mut upload_mgr = ctx.upload_manager.lock().unwrap();
                        match upload_mgr.complete_upload(upload_id, expected_sha256, storage) {
                            Ok(job) => {
                                send_json(
                                    stream,
                                    200,
                                    &serde_json::json!({
                                        "status": "complete",
                                        "job": job
                                    }),
                                );
                                return;
                            }
                            Err(e) => {
                                let status = match e {
                                    UploadError::IncompleteUpload { .. } => 400,
                                    UploadError::FileChecksumMismatch { .. } => 400,
                                    UploadError::ValidationFailed(_) => 400,
                                    UploadError::SessionNotFound(_) => 404,
                                    _ => 500,
                                };
                                send_json(
                                    stream,
                                    status,
                                    &serde_json::json!({"error": e.to_string()}),
                                );
                                return;
                            }
                        }
                    }
                }
            }

            send_json(
                stream,
                400,
                &serde_json::json!({"error": "Invalid upload completion request"}),
            );
        }

        ("POST", "/api/v1/portal/activate") => {
            if !check_auth(&headers, &ctx) {
                send_json(
                    stream,
                    401,
                    &serde_json::json!({"error": "Unauthorized: valid token required"}),
                );
                return;
            }

            let storage = match &ctx.storage_manager {
                Some(s) => s,
                None => {
                    send_json(
                        stream,
                        500,
                        &serde_json::json!({"error": "Storage manager not configured"}),
                    );
                    return;
                }
            };

            if let Ok(body_str) = std::str::from_utf8(&body) {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(body_str) {
                    let job_id_opt = json_val
                        .get("job_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| Uuid::parse_str(s).ok());
                    let gen_name_opt = json_val
                        .get("generation_name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let gen_name = if let Some(gen_name) = gen_name_opt {
                        gen_name
                    } else if let Some(job_id) = job_id_opt {
                        let new_gen = ShowStorageManager::generate_generation_name(job_id.as_bytes());
                        match storage.promote_staging_to_generation(job_id, &new_gen) {
                            Ok(_) => {
                                let _ = storage.job_store().transition_job(job_id, |j| {
                                    j.state = JobState::Activating;
                                    j.target_generation = Some(new_gen.clone());
                                });
                                let _ = storage.job_store().transition_job(job_id, |j| {
                                    j.state = JobState::Complete;
                                });
                                new_gen
                            }
                            Err(e) => {
                                send_json(
                                    stream,
                                    500,
                                    &serde_json::json!({"error": format!("Promotion failed: {}", e)}),
                                );
                                return;
                            }
                        }
                    } else {
                        send_json(
                            stream,
                            400,
                            &serde_json::json!({"error": "Missing job_id or generation_name"}),
                        );
                        return;
                    };

                    match storage.activate_generation(&gen_name) {
                        Ok(()) => {
                            send_json(
                                stream,
                                200,
                                &serde_json::json!({
                                    "status": "activated",
                                    "active_generation": gen_name
                                }),
                            );
                            return;
                        }
                        Err(e) => {
                            send_json(
                                stream,
                                500,
                                &serde_json::json!({"error": format!("Activation failed: {}", e)}),
                            );
                            return;
                        }
                    }
                }
            }

            send_json(
                stream,
                400,
                &serde_json::json!({"error": "Invalid activation body"}),
            );
        }

        ("POST", "/api/v1/portal/rollback") => {
            if !check_auth(&headers, &ctx) {
                send_json(
                    stream,
                    401,
                    &serde_json::json!({"error": "Unauthorized: valid token required"}),
                );
                return;
            }

            let storage = match &ctx.storage_manager {
                Some(s) => s,
                None => {
                    send_json(
                        stream,
                        500,
                        &serde_json::json!({"error": "Storage manager not configured"}),
                    );
                    return;
                }
            };

            match storage.rollback() {
                Ok(rolled_back_to) => {
                    send_json(
                        stream,
                        200,
                        &serde_json::json!({
                            "status": "rolled_back",
                            "active_generation": rolled_back_to
                        }),
                    );
                }
                Err(e) => {
                    send_json(
                        stream,
                        500,
                        &serde_json::json!({"error": format!("Rollback failed: {}", e)}),
                    );
                }
            }
        }

        ("POST", "/api/v1/portal/jobs/cancel") => {
            if !check_auth(&headers, &ctx) {
                send_json(
                    stream,
                    401,
                    &serde_json::json!({"error": "Unauthorized: valid token required"}),
                );
                return;
            }

            let storage = match &ctx.storage_manager {
                Some(s) => s,
                None => {
                    send_json(
                        stream,
                        500,
                        &serde_json::json!({"error": "Storage manager not configured"}),
                    );
                    return;
                }
            };

            if let Ok(body_str) = std::str::from_utf8(&body) {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(body_str) {
                    if let Some(job_id_str) = json_val.get("job_id").and_then(|v| v.as_str()) {
                        if let Ok(job_id) = Uuid::parse_str(job_id_str) {
                            let _ = storage.job_store().transition_job(job_id, |j| {
                                j.state = JobState::Cancelled;
                            });
                            send_json(
                                stream,
                                200,
                                &serde_json::json!({"status": "cancelled", "job_id": job_id}),
                            );
                            return;
                        }
                    }
                }
            }
            send_json(
                stream,
                400,
                &serde_json::json!({"error": "Invalid cancel request"}),
            );
        }

        _ => {
            send_json(stream, 404, &serde_json::json!({"error": "Not Found"}));
        }
    }
}

pub fn embedded_gig_html() -> String {
    concat!(
        "<!DOCTYPE html>\n",
        "<html lang=\"en\">\n",
        "<head>\n",
        "    <meta charset=\"UTF-8\">\n",
        "    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n",
        "    <title>RPi-VJ V3 Gig Show</title>\n",
        "    <style>\n",
        "        * { margin: 0; padding: 0; box-sizing: border-box; cursor: none !important; user-select: none; }\n",
        "        html, body { width: 100vw; height: 100vh; overflow: hidden; background: #000; color: #00ffcc; font-family: monospace; display: flex; flex-direction: column; align-items: center; justify-content: center; }\n",
        "        #canvas { position: absolute; top: 0; left: 0; width: 100%; height: 100%; z-index: 1; }\n",
        "        .overlay { position: relative; z-index: 2; text-align: center; background: rgba(5, 5, 8, 0.75); padding: 24px 48px; border: 1px solid rgba(0, 255, 204, 0.3); border-radius: 8px; }\n",
        "        .title { font-size: 32px; letter-spacing: 8px; font-weight: bold; margin-bottom: 8px; }\n",
        "        .stat-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; margin-top: 16px; font-size: 14px; }\n",
        "        .stat-box { border: 1px solid rgba(0, 255, 204, 0.2); padding: 8px 16px; border-radius: 4px; }\n",
        "        .stat-val { font-size: 20px; font-weight: bold; color: #fff; margin-top: 4px; }\n",
        "    </style>\n",
        "</head>\n",
        "<body>\n",
        "    <canvas id=\"canvas\"></canvas>\n",
        "    <div class=\"overlay\">\n",
        "        <div class=\"title\">RPI-VJ V3 &bull; LIVE</div>\n",
        "        <div class=\"stat-grid\">\n",
        "            <div class=\"stat-box\"><div>BANK</div><div class=\"stat-val\" id=\"stat-bank\">0</div></div>\n",
        "            <div class=\"stat-box\"><div>CLIP</div><div class=\"stat-val\" id=\"stat-clip\">OFF</div></div>\n",
        "            <div class=\"stat-box\"><div>BPM</div><div class=\"stat-val\" id=\"stat-bpm\">120.0</div></div>\n",
        "        </div>\n",
        "    </div>\n",
        "    <script>\n",
        "        const canvas = document.getElementById(\"canvas\");\n",
        "        const ctx = canvas.getContext(\"2d\");\n",
        "        let frameCount = 0;\n",
        "        function resize() { canvas.width = window.innerWidth; canvas.height = window.innerHeight; }\n",
        "        window.addEventListener(\"resize\", resize);\n",
        "        resize();\n",
        "        function render(time) {\n",
        "            frameCount++;\n",
        "            ctx.fillStyle = \"#05050a\";\n",
        "            ctx.fillRect(0, 0, canvas.width, canvas.height);\n",
        "            const cx = canvas.width / 2; const cy = canvas.height / 2; const t = time * 0.002;\n",
        "            ctx.lineWidth = 3;\n",
        "            for (let i = 0; i < 5; i++) {\n",
        "                ctx.strokeStyle = `hsl(${(t * 40 + i * 50) % 360}, 90%, 60%)`;\n",
        "                ctx.beginPath();\n",
        "                const r = 80 + i * 45 + Math.sin(t + i) * 20;\n",
        "                ctx.arc(cx, cy, Math.max(10, r), 0, Math.PI * 2);\n",
        "                ctx.stroke();\n",
        "            }\n",
        "            requestAnimationFrame(render);\n",
        "        }\n",
        "        requestAnimationFrame(render);\n",
        "        async function sendHeartbeat() {\n",
        "            try {\n",
        "                await fetch(\"/api/v1/heartbeat\", {\n",
        "                    method: \"POST\",\n",
        "                    headers: { \"Content-Type\": \"application/json\" },\n",
        "                    body: JSON.stringify({ source: \"web_renderer\", frame_count: frameCount })\n",
        "                });\n",
        "            } catch (e) {}\n",
        "        }\n",
        "        setInterval(sendHeartbeat, 1000);\n",
        "        async function pollState() {\n",
        "            try {\n",
        "                const res = await fetch(\"/api/v1/state\");\n",
        "                if (res.ok) {\n",
        "                    const data = await res.json();\n",
        "                    document.getElementById(\"stat-bank\").textContent = data.active_bank;\n",
        "                    document.getElementById(\"stat-clip\").textContent = data.active_clip !== null ? data.active_clip : \"OFF\";\n",
        "                    document.getElementById(\"stat-bpm\").textContent = data.tempo_bpm.toFixed(1);\n",
        "                }\n",
        "            } catch (e) {}\n",
        "        }\n",
        "        setInterval(pollState, 200);\n",
        "    </script>\n",
        "</body>\n",
        "</html>\n"
    ).to_string()
}
