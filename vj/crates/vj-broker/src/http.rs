use crate::protocol::Message;
use crate::socket::BrokerContext;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

pub const DEFAULT_HTTP_ADDR: &str = "127.0.0.1:8080";

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
            Ok((stream, _peer)) => {
                let ctx_clone = Arc::clone(&ctx);
                let web_root_buf = web_root.map(|p| p.to_path_buf());
                thread::spawn(move || {
                    handle_http_connection(stream, web_root_buf.as_deref(), ctx_clone);
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

fn handle_http_connection(
    mut stream: TcpStream,
    web_root: Option<&Path>,
    ctx: Arc<BrokerContext>,
) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(5)));

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
    let path = parts[1];

    // Read headers
    let mut content_length = 0usize;
    loop {
        let mut header_line = String::new();
        if reader.read_line(&mut header_line).is_err() || header_line.trim().is_empty() {
            break;
        }
        let lower = header_line.to_lowercase();
        if lower.starts_with("content-length:") {
            if let Some(val) = header_line.split(':').nth(1) {
                content_length = val.trim().parse::<usize>().unwrap_or(0);
            }
        }
    }

    // Read body if present
    let mut body = Vec::new();
    if content_length > 0 && content_length <= 1024 * 1024 {
        body.resize(content_length, 0);
        if reader.read_exact(&mut body).is_err() {
            return;
        }
    }

    // Route dispatch
    match (method, path) {
        ("GET", "/") => {
            let resp = "HTTP/1.1 302 Found\r\nLocation: /gig/\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
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

            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html_content.len(),
                html_content
            );
            let _ = stream.write_all(resp.as_bytes());
        }
        ("GET", "/api/v1/state") => {
            let store = ctx.state_store.lock().unwrap();
            let state_json = serde_json::to_string(store.get_state()).unwrap_or_default();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                state_json.len(),
                state_json
            );
            let _ = stream.write_all(resp.as_bytes());
        }
        ("POST", "/api/v1/command") => {
            if let Ok(body_str) = std::str::from_utf8(&body) {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(body_str) {
                    let cmd_id = json_val.get("id").and_then(|v| v.as_str()).unwrap_or("http-cmd").to_string();
                    let name = json_val.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let args = json_val.get("args").cloned().unwrap_or(serde_json::json!({}));

                    let mut store = ctx.state_store.lock().unwrap();
                    let (ok, err_msg) = store.apply_command(name, &args);
                    let current_state_json = serde_json::to_value(store.get_state()).unwrap_or_default();
                    drop(store);

                    let ack = if ok {
                        ctx.broadcast_state(&current_state_json);
                        Message::ack_accepted(cmd_id)
                    } else {
                        Message::ack_failed(cmd_id, err_msg.unwrap_or_else(|| "Command failed".to_string()))
                    };

                    let ack_json = serde_json::to_string(&ack).unwrap_or_default();
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        ack_json.len(),
                        ack_json
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    return;
                }
            }
            let resp = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
        }
        ("POST", "/api/v1/heartbeat") => {
            let mut sup = ctx.supervisor.lock().unwrap();
            sup.record_heartbeat(Instant::now());
            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
            let _ = stream.write_all(resp.as_bytes());
        }
        ("GET", "/api/v1/status") => {
            let sup = ctx.supervisor.lock().unwrap();
            let status_json = serde_json::json!({
                "level": sup.get_level(),
                "durable_degraded": sup.is_durable_degraded(),
            });
            let body_str = serde_json::to_string(&status_json).unwrap_or_default();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body_str.len(),
                body_str
            );
            let _ = stream.write_all(resp.as_bytes());
        }
        _ => {
            let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
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
