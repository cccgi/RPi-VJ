use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use vj_broker::auth::PortalAuthManager;
use vj_broker::http::run_http_server;
use vj_broker::protocol::Message;
#[cfg(unix)]
use vj_broker::socket::handle_client_stream;
use vj_broker::socket::BrokerContext;
use vj_broker::state::StateStore;
use vj_broker::supervisor::{Supervisor, SupervisorAction};
use vj_broker::upload::{sanitize_and_validate_filename, UploadManager};
use vj_storage::{ShowStorageManager, StorageConfig};

fn get_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn execute_action(action: SupervisorAction, dry_run: bool) {
    match action {
        SupervisorAction::None => {}
        SupervisorAction::LogWarning { reason } => {
            eprintln!("[supervisor] WARNING: {}", reason);
        }
        SupervisorAction::RestartRenderer { count_in_window } => {
            eprintln!(
                "[supervisor] ESCALATION LEVEL 1: Restarting rpi-vj-renderer.service (attempt {}/3)...",
                count_in_window
            );
            if !dry_run {
                let _ = Command::new("systemctl")
                    .args(["restart", "rpi-vj-renderer.service"])
                    .status();
            }
        }
        SupervisorAction::ControlledReboot { reason } => {
            eprintln!(
                "[supervisor] ESCALATION LEVEL 2: CONTROLLED REBOOT: {}",
                reason
            );
            if !dry_run {
                let _ = Command::new("systemctl").arg("reboot").status();
            }
        }
        SupervisorAction::EnterDurableDegraded { reason } => {
            eprintln!(
                "[supervisor] ESCALATION LEVEL 3: DURABLE DEGRADED MODE: {}",
                reason
            );
        }
    }
}

fn run_self_tests() -> bool {
    println!("=== Running vj-broker Self-Tests ===");
    let mut passed = 0;
    let mut failed = 0;

    // Test 1: Deduplication & Acceptance
    print!("Test 1: Command dedup and immediate non-blocking ack ... ");
    let t0 = Instant::now();
    let store = StateStore::new(None);
    let sup = Supervisor::new(None, None, t0, 1700000000);
    let ctx = Arc::new(BrokerContext::new(store, sup));

    let cmd_line =
        r#"{"v":1,"id":"cmd-1","type":"command","name":"clip.trigger","args":{"key":3}}"#;
    let resp1 = ctx.handle_message(cmd_line);
    let resp2 = ctx.handle_message(cmd_line); // Duplicate

    match (resp1, resp2) {
        (
            Some(Message::Ack {
                id: id1,
                status: s1,
                ..
            }),
            Some(Message::Ack {
                id: id2,
                status: s2,
                ..
            }),
        ) => {
            if id1 == "cmd-1"
                && id2 == "cmd-1"
                && s1 == vj_broker::AckStatus::Accepted
                && s2 == vj_broker::AckStatus::Accepted
            {
                println!("PASS");
                passed += 1;
            } else {
                println!("FAIL (status mismatch)");
                failed += 1;
            }
        }
        _ => {
            println!("FAIL (invalid response format)");
            failed += 1;
        }
    }

    // Test 2: State consistency
    print!("Test 2: State store holds active clip and version ... ");
    let state = ctx.state_store.lock().unwrap().get_state().clone();
    if state.active_clip == Some(3) && state.version == 2 {
        println!("PASS");
        passed += 1;
    } else {
        println!("FAIL (state: {:?})", state);
        failed += 1;
    }

    // Test 3: Monotonic watchdog without wall-clock dependency
    print!("Test 3: Monotonic watchdog escalation ... ");
    let mut supervisor = Supervisor::new(None, None, t0, 1700000000);
    let a1 = supervisor.check_health(t0 + Duration::from_secs(2), 1700000000);
    let a2 = supervisor.check_health(t0 + Duration::from_secs(8), 1700000000);
    if a1 == SupervisorAction::None
        && matches!(
            a2,
            SupervisorAction::RestartRenderer { count_in_window: 1 }
        )
    {
        println!("PASS");
        passed += 1;
    } else {
        println!("FAIL");
        failed += 1;
    }

    // Test 4: Path traversal rejection in upload validation
    print!("Test 4: Strict path traversal prevention ... ");
    if sanitize_and_validate_filename("../secret.mp4").is_err()
        && sanitize_and_validate_filename("/etc/shadow.mov").is_err()
        && sanitize_and_validate_filename("valid_clip.mp4").is_ok()
    {
        println!("PASS");
        passed += 1;
    } else {
        println!("FAIL");
        failed += 1;
    }

    // Test 5: Resumable chunked upload & job integration
    print!("Test 5: Resumable chunked upload & job integration ... ");
    let temp_dir = std::env::temp_dir().join(format!("vj-test-{}", uuid::Uuid::new_v4()));
    let _ = fs::create_dir_all(&temp_dir);
    let storage_cfg = StorageConfig::for_testing(&temp_dir);
    let mock_checker = Arc::new(vj_storage::disk::MockDiskSpaceChecker::new(
        50 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
    ));
    if let Ok(storage_mgr) = ShowStorageManager::with_checker(storage_cfg, mock_checker) {
        let mut upload_mgr = UploadManager::new();
        let payload = b"0123456789abcdef";
        if let Ok(init) = upload_mgr.init_upload(
            "test-loop.mp4",
            16,
            8,
            2,
            None,
            &storage_mgr,
        ) {
            let u_id = init.upload_id;
            let c0 = upload_mgr.receive_chunk(u_id, 0, 0, &payload[0..8], None, &storage_mgr);
            let c1 = upload_mgr.receive_chunk(u_id, 1, 8, &payload[8..16], None, &storage_mgr);
            let comp = upload_mgr.complete_upload(u_id, None, &storage_mgr);

            if c0.is_ok() && c1.is_ok() && comp.is_ok() {
                println!("PASS");
                passed += 1;
            } else {
                println!("FAIL (upload failure)");
                failed += 1;
            }
        } else {
            println!("FAIL (init failed)");
            failed += 1;
        }
    } else {
        println!("FAIL (storage init failed)");
        failed += 1;
    }
    let _ = fs::remove_dir_all(&temp_dir);

    println!("=== Summary: {} passed, {} failed ===", passed, failed);
    failed == 0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    #[allow(unused_variables, unused_mut, unused_assignments)]
    let mut socket_path = PathBuf::from("/run/rpi-vj/broker.sock");
    let mut http_addr = "0.0.0.0:8080".to_string();
    let mut web_root = PathBuf::from("/opt/rpi-vj/web");
    let mut state_file = PathBuf::from("/var/lib/rpi-vj/desired-state.json");
    let mut supervisor_state = PathBuf::from("/var/lib/rpi-vj/supervisor-state.json");
    let mut boot_ledger = PathBuf::from("/var/lib/rpi-vj/boot-ledger.jsonl");
    let mut auth_file = PathBuf::from("/var/lib/rpi-vj/portal-auth.json");
    let mut shows_dir = PathBuf::from("/srv/rpi-vj/shows");
    let mut staging_dir = PathBuf::from("/srv/rpi-vj/staging");
    let mut current_link = PathBuf::from("/srv/rpi-vj/current");
    let mut job_ledger = PathBuf::from("/var/lib/rpi-vj/ingest-jobs.jsonl");
    let mut hostname = "rpi-vj.local".to_string();
    let mut dry_run = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--test" => {
                if run_self_tests() {
                    std::process::exit(0);
                } else {
                    std::process::exit(1);
                }
            }
            "--dry-run" => {
                dry_run = true;
                i += 1;
            }
            "--socket" if i + 1 < args.len() => {
                socket_path = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--http" if i + 1 < args.len() => {
                http_addr = args[i + 1].clone();
                i += 2;
            }
            "--web-root" if i + 1 < args.len() => {
                web_root = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--state-file" if i + 1 < args.len() => {
                state_file = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--supervisor-state" if i + 1 < args.len() => {
                supervisor_state = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--boot-ledger" if i + 1 < args.len() => {
                boot_ledger = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--auth-file" if i + 1 < args.len() => {
                auth_file = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--shows-dir" if i + 1 < args.len() => {
                shows_dir = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--staging-dir" if i + 1 < args.len() => {
                staging_dir = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--current-link" if i + 1 < args.len() => {
                current_link = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--job-ledger" if i + 1 < args.len() => {
                job_ledger = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--hostname" if i + 1 < args.len() => {
                hostname = args[i + 1].clone();
                i += 2;
            }
            "-h" | "--help" => {
                println!("Usage: vj-broker [OPTIONS]");
                println!("RPi-VJ Appliance V3 -- Command Broker, Supervisor & LAN Ingest Portal");
                println!("Options:");
                println!("  --socket <path>           Unix domain socket path (default: /run/rpi-vj/broker.sock)");
                println!("  --http <addr>             HTTP listen address (default: 0.0.0.0:8080)");
                println!("  --web-root <dir>          Web document root (default: /opt/rpi-vj/web)");
                println!("  --state-file <path>       Desired show state persistence path");
                println!("  --supervisor-state <path> Supervisor reboot persistence path");
                println!("  --boot-ledger <path>      Boot ledger log path");
                println!("  --auth-file <path>        Portal authentication credentials JSON");
                println!("  --shows-dir <path>        Installed shows directory");
                println!("  --staging-dir <path>      Ingest staging directory");
                println!("  --current-link <path>     Current active show symlink");
                println!("  --job-ledger <path>       Ingest jobs JSONL ledger");
                println!("  --hostname <name>         Appliance mDNS / identity hostname");
                println!("  --test                    Run built-in test suite");
                println!("  --dry-run                 Run without executing systemctl actions");
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                std::process::exit(1);
            }
        }
    }

    println!("========================================================");
    println!(" RPi-VJ Appliance V3 -- Broker & LAN Ingest Portal");
    println!("========================================================");

    let running = Arc::new(AtomicBool::new(true));

    let state_store = StateStore::new(Some(state_file));
    let supervisor = Supervisor::new(
        Some(supervisor_state),
        Some(boot_ledger),
        Instant::now(),
        get_epoch_secs(),
    );

    // Initialize Storage Manager
    let storage_config = StorageConfig {
        shows_dir,
        staging_dir,
        current_link,
        job_ledger_path: job_ledger,
        free_reserve_percent: 15.0,
    };
    let storage_mgr = ShowStorageManager::new(storage_config)
        .map(Arc::new)
        .map_err(|e| eprintln!("[broker] Warning initializing storage manager: {}", e))
        .ok();

    // Initialize Auth Manager
    let auth_mgr = PortalAuthManager::new(auth_file)
        .map(|mgr| Arc::new(Mutex::new(mgr)))
        .map_err(|e| eprintln!("[broker] Warning initializing portal auth: {}", e))
        .ok();

    let ctx = Arc::new(BrokerContext::with_portal(
        state_store,
        supervisor,
        storage_mgr,
        auth_mgr,
        Some(hostname),
    ));

    // Spawn HTTP Server
    let http_ctx = Arc::clone(&ctx);
    let http_running = Arc::clone(&running);
    let http_addr_clone = http_addr.clone();
    let web_root_clone = web_root.clone();
    thread::spawn(move || {
        if let Err(e) = run_http_server(
            &http_addr_clone,
            Some(&web_root_clone),
            http_ctx,
            http_running,
        ) {
            eprintln!("[broker] HTTP server error: {}", e);
        }
    });

    // Spawn Monotonic Supervisor Watchdog
    let sup_ctx = Arc::clone(&ctx);
    let sup_running = Arc::clone(&running);
    thread::spawn(move || {
        while sup_running.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(1000));
            let now = Instant::now();
            let epoch = get_epoch_secs();

            let action = {
                let mut sup = sup_ctx.supervisor.lock().unwrap();
                sup.check_health(now, epoch)
            };

            execute_action(action, dry_run);
        }
    });

    // Unix Domain Socket listener (on Unix) or fallback
    #[cfg(unix)]
    {
        if let Some(parent) = socket_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::remove_file(&socket_path);

        use std::os::unix::net::UnixListener;
        match UnixListener::bind(&socket_path) {
            Ok(listener) => {
                let _ = fs::set_permissions(
                    &socket_path,
                    std::os::unix::fs::PermissionsExt::from_mode(0o777),
                );
                listener.set_nonblocking(true).unwrap_or(());
                println!(
                    "[broker] Unix socket listening on {}",
                    socket_path.display()
                );

                while running.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let ctx_clone = Arc::clone(&ctx);
                            thread::spawn(move || {
                                if let Ok(writer_stream) = stream.try_clone() {
                                    handle_client_stream(stream, writer_stream, ctx_clone);
                                }
                            });
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(e) => {
                            eprintln!("[broker] Unix socket accept error: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "[broker] Failed to bind Unix socket {}: {}",
                    socket_path.display(),
                    e
                );
            }
        }
        let _ = fs::remove_file(&socket_path);
    }

    #[cfg(not(unix))]
    {
        let _ = &socket_path;
        println!("[broker] Non-Unix host detected. Running HTTP & supervisor watchdog.");
        while running.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(500));
        }
    }

    println!("[broker] Broker exiting cleanly.");
}
