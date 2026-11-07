//! Small bounded HTTP/1.1 loopback server; no framework, WebSocket or CDN.
use super::*;
use app::{App, Job};
use gamectx::obj;
use providers::{Config, Transport};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{sync_channel, SyncSender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
struct Incoming {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}
fn read(stream: &mut TcpStream) -> Result<Incoming> {
    stream.set_read_timeout(Some(Duration::from_secs(3))).map_err(|_| err())?;
    stream.set_write_timeout(Some(Duration::from_secs(3))).map_err(|_| err())?;
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut raw = Vec::new();
    let split = loop {
        if Instant::now() > deadline || raw.len() > 16384 {
            return Err(err());
        }
        let mut b = [0; 1];
        stream.read_exact(&mut b).map_err(|_| err())?;
        raw.push(b[0]);
        if raw.ends_with(b"\r\n\r\n") {
            break raw.len();
        }
    };
    let head = std::str::from_utf8(&raw).map_err(|_| err())?;
    let mut lines = head.split("\r\n");
    let first = lines.next().ok_or_else(err)?.split(' ').collect::<Vec<_>>();
    if first.len() != 3 || first[2] != "HTTP/1.1" || !["GET", "POST"].contains(&first[0]) || first[1].len() > 512 {
        return Err(err());
    }
    let mut headers = Vec::<(String, String)>::new();
    for line in lines.filter(|l| !l.is_empty()) {
        let (key, v) = line.split_once(':').ok_or_else(err)?;
        let key = key.to_ascii_lowercase();
        if headers.iter().any(|(k, _)| k == &key) || v.chars().any(|c| c.is_control()) {
            return Err(err());
        }
        headers.push((key, v.trim().into()));
    }
    if headers.iter().any(|(k, _)| k == "transfer-encoding") {
        return Err(err());
    }
    let size = if let Some((_, n)) = headers.iter().find(|(k, _)| k == "content-length") {
        n.parse::<usize>().map_err(|_| err())?
    } else {
        0
    };
    if size > providers::MAX_BYTES {
        return Err(err());
    }
    let method = first[0].to_owned();
    let path = first[1].to_owned();
    raw.resize(split + size, 0);
    let mut received = split;
    while received < raw.len() {
        let remaining = deadline.checked_duration_since(Instant::now()).ok_or_else(err)?;
        stream.set_read_timeout(Some(remaining)).map_err(|_| err())?;
        let n = stream.read(&mut raw[received..]).map_err(|_| err())?;
        if n == 0 {
            return Err(err());
        }
        received += n;
    }
    Ok(Incoming { method, path, headers, body: raw[split..].into() })
}
fn response(stream: &mut TcpStream, status: u16, mime: &str, body: &[u8]) -> Result<()> {
    response_header(stream, status, mime, body, "")
}
fn response_header(stream: &mut TcpStream, status: u16, mime: &str, body: &[u8], extra: &str) -> Result<()> {
    write!(stream,"HTTP/1.1 {} {}\r\n{}Content-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' blob:; media-src 'self' blob:; connect-src 'self'; frame-ancestors 'none'\r\n\r\n",status,if status==200{"OK"}else{"Rejected"},extra,mime,body.len()).map_err(|_|err())?;
    stream.write_all(body).map_err(|_| err())?;
    Ok(())
}
fn worker(app: Arc<Mutex<App>>, rx: std::sync::mpsc::Receiver<Job>, mut transport: impl Transport) {
    let mut followups = std::collections::VecDeque::new();
    loop {
        let mut job = if let Some(j) = followups.pop_front() {
            j
        } else {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(j) => j,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if let Ok(Some(report)) = transport.idle() {
                        if let Ok(mut a) = app.lock() {
                            let _ = a.manager_observe(report);
                        }
                    }
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        };
        let ready = app
            .lock()
            .ok()
            .and_then(|mut a| {
                let ready = a.before_dispatch(&job).ok()?;
                if ready && job.modality == "text" {
                    job.request.text_token = Some(providers::text_token());
                }
                Some(ready)
            })
            .unwrap_or(false);
        if !ready {
            continue;
        }
        // The only provider execution lane: local text, diffusion and audio never overlap.
        let began = Instant::now();
        let result = transport.send(&job.request);
        let provider_ms = began.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let manager_status = transport.manager_status();
        if let Ok(mut a) = app.lock() {
            a.profile_stage(job.id, "provider_ms", provider_ms);
            if let Some(report) = manager_status {
                let _ = a.manager_observe(report);
            }
            let complete = Instant::now();
            match a.complete(&job, result) {
                Ok(next) => followups.extend(next),
                Err(_) => eprintln!("Studio completion rejected; spending remains reserved."),
            }
            a.profile_stage(job.id, "completion_ms", complete.elapsed().as_millis().min(u64::MAX as u128) as u64);
        }
    }
}
fn handle(
    stream: &mut TcpStream,
    app: &Arc<Mutex<App>>,
    queue: &SyncSender<Job>,
    port: u16,
    session: &Option<Arc<Mutex<super::session::Session>>>,
) -> Result<()> {
    let request = read(stream)?;
    let host = format!("{}:{port}", stream.local_addr().map_err(|_| err())?.ip());
    let guest = stream.local_addr().map_err(|_| err())?.ip() != std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
    let header = |name: &str| request.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str());
    if header("host") != Some(host.as_str()) || header("origin").is_some_and(|o| o != format!("http://{host}")) {
        return response(stream, 403, "text/plain", b"Loopback origin required.");
    }
    if guest {
        let session = session.as_ref().ok_or_else(err)?;
        if request.method == "GET" {
            match request.path.as_str() {
                "/" | "/join" => return response(stream, 200, "text/html", include_bytes!("guest.html")),
                "/guest.mjs" => return response(stream, 200, "text/javascript", include_bytes!("guest.mjs")),
                "/studio.css" => return response(stream, 200, "text/css", include_bytes!("studio.css")),
                _ => {}
            }
        }
        if request.method == "POST" && request.path == "/api/join" {
            if header("x-studio-request") != Some("1") || header("content-type") != Some("application/json") {
                return response(stream, 403, "text/plain", b"JSON join required.");
            }
            let args = gamectx::json::parse(std::str::from_utf8(&request.body).map_err(|_| err())?, 256)?;
            fields(&args, &["code"])?;
            match session.lock().map_err(|_| err())?.join(text(&args, "code")?) {
                Ok(token) => {
                    app.lock().map_err(|_| err())?.share();
                    return response_header(
                        stream,
                        200,
                        "application/json",
                        b"{\"ok\":true}",
                        &format!("Set-Cookie: lantern_guest={token}; HttpOnly; SameSite=Strict; Path=/\r\n"),
                    );
                }
                Err(e) => {
                    return response(
                        stream,
                        403,
                        "application/json",
                        obj! {"ok"=>false,"error"=>e.to_string()}.to_json().as_bytes(),
                    )
                }
            }
        }
        if !session.lock().map_err(|_| err())?.authorized(header("cookie")) {
            return response(
                stream,
                401,
                "application/json",
                b"{\"ok\":false,\"error\":\"Join the host session first.\"}",
            );
        }
        if request.method == "GET" && request.path == "/api/guest" {
            return response(
                stream,
                200,
                "application/json",
                app.lock().map_err(|_| err())?.guest_state().to_json().as_bytes(),
            );
        }
        if request.method != "POST" || !["/api/guest_chat", "/api/guest_read"].contains(&request.path.as_str()) {
            return response(
                stream,
                403,
                "application/json",
                b"{\"ok\":false,\"error\":\"This control belongs to the host.\"}",
            );
        }
    } else if request.method == "GET" && request.path == "/api/session" {
        let value = session.as_ref().map_or(obj! {"enabled"=>false}, |s| s.lock().unwrap().info(port));
        return response(stream, 200, "application/json", value.to_json().as_bytes());
    }
    if request.method == "GET" {
        let static_file = match request.path.as_str() {
            "/" => Some(("text/html; charset=utf-8", include_bytes!("index.html").as_slice())),
            "/studio.css" => Some(("text/css", include_bytes!("studio.css").as_slice())),
            "/studio.mjs" => Some(("text/javascript", include_bytes!("studio.mjs").as_slice())),
            "/context.mjs" => Some(("text/javascript", include_bytes!("context.mjs").as_slice())),
            "/models.mjs" => Some(("text/javascript", include_bytes!("models.mjs").as_slice())),
            "/setup.mjs" => Some(("text/javascript", include_bytes!("setup.mjs").as_slice())),
            "/profiler.mjs" => Some(("text/javascript", include_bytes!("profiler.mjs").as_slice())),
            "/voice.mjs" => Some(("text/javascript", include_bytes!("voice.mjs").as_slice())),
            _ => None,
        };
        if let Some((mime, body)) = static_file {
            return response(stream, 200, mime, body);
        }
        if request.path == "/api/state" {
            let mut current = app.lock().map_err(|_| err())?;
            if session.is_some() {
                current.lock_host_player();
            }
            let v = current.state()?;
            return response(stream, 200, "application/json", v.to_json().as_bytes());
        }
        if let Some(key) = request.path.strip_prefix("/api/voice/") {
            return match app.lock().map_err(|_| err())?.voice_recording(key) {
                Ok(bytes) => response(stream, 200, "audio/wav", &bytes),
                Err(_) => response(stream, 404, "text/plain", b"Recording is no longer in this session's backlog."),
            };
        }
        if let Some(query) = request.path.strip_prefix("/api/asset/") {
            let (key, params) = query.split_once('?').unwrap_or((query, ""));
            let candidate = params == "candidate=1";
            if !params.is_empty() && !candidate && !params.starts_with("digest=") {
                return Err(err());
            }
            let bytes = app.lock().map_err(|_| err())?.asset(key, candidate)?;
            if let Some(digest) = params.strip_prefix("digest=") {
                if digest != gamectx::visual::content_digest(&bytes)? {
                    return Err(err());
                }
            }
            return response(stream, 200, "image/png", &bytes);
        }
        return response(stream, 404, "text/plain", b"Not found.");
    }
    if header("x-studio-request") != Some("1") || header("content-type") != Some("application/json") {
        return response(stream, 403, "text/plain", b"JSON studio request header required.");
    }
    let op = request.path.strip_prefix("/api/").ok_or_else(err)?;
    let args = gamectx::json::parse(std::str::from_utf8(&request.body).map_err(|_| err())?, providers::MAX_BYTES)?;
    let mut current = app.lock().map_err(|_| err())?;
    if guest && header("x-studio-generation").is_none() {
        return response(
            stream,
            400,
            "application/json",
            b"{\"ok\":false,\"error\":\"Refresh the shared scene before sending.\"}",
        );
    }
    if let Some(generation) = header("x-studio-generation") {
        if generation.is_empty()
            || generation.len() > 20
            || generation.len() > 1 && generation.starts_with('0')
            || !generation.bytes().all(|c| c.is_ascii_digit())
        {
            return Err(err());
        }
        let generation = generation.parse::<u64>().map_err(|_| err())?;
        if generation != current.epoch {
            let epoch = current.epoch;
            drop(current);
            return response(
                stream,
                409,
                "application/json",
                obj! {"ok"=>false,"error"=>"Scene changed; try your action again.","generation"=>epoch}
                    .to_json()
                    .as_bytes(),
            );
        }
    }
    if !guest && session.is_some() {
        current.lock_host_player();
        if op == "session_rotate" {
            fields(&args, &[])?;
            session.as_ref().unwrap().lock().map_err(|_| err())?.rotate()?;
            current.release_guest()?;
            current.share();
            super::native_voice::cancel();
            return response(stream, 200, "application/json", b"{\"ok\":true,\"result\":{\"rotated\":true}}");
        }
        if op == "select" && args.get("player").and_then(Value::as_str) != Some("player_1") {
            return response(
                stream,
                403,
                "application/json",
                b"{\"ok\":false,\"error\":\"Player 2 belongs to the joined guest.\"}",
            );
        }
        if (["chat", "act", "select", "mic", "promise"].contains(&op) && current.floor() == Some("player_2"))
            || (["delivery", "synthesize"].contains(&op) && !current.host_delivery())
        {
            return response(
                stream,
                409,
                "application/json",
                b"{\"ok\":false,\"error\":\"Player 2 holds the conversation turn.\"}",
            );
        }
    }
    let (value, jobs) = match if guest { current.guest_command(op, &args) } else { current.command(op, &args) } {
        Ok(result) => result,
        Err(_) => {
            let epoch = current.epoch;
            drop(current);
            return response(stream,400,"application/json",obj! {"ok"=>false,"error"=>if op=="settings"{"Settings were not applied. Check the model name, voice values and authorized spending cap."}else if guest {"The turn is busy, the character is elsewhere, or this is a world action. Send conversation to a present character after the current reply."}else{"That request could not be applied to the current game. Refresh and try again."},"generation"=>epoch}.to_json().as_bytes());
        }
    };
    if !guest && session.is_some() {
        current.lock_host_player();
    }
    let generation = current.epoch;
    if ["chat", "guest_chat", "guest_read", "act", "select", "mic", "recognition", "reset", "load"].contains(&op)
        || op == "delivery" && args.get("event").and_then(Value::as_str) == Some("queued")
    {
        providers::cancel_text();
    }
    if (op == "delivery" && args.get("event").and_then(Value::as_str) == Some("interrupted"))
        || op == "recognition"
        || op == "reset"
        || (op == "manager_action" && args.get("action").and_then(Value::as_str) == Some("stop"))
    {
        super::native_voice::cancel();
    }
    drop(current);
    for job in jobs {
        if let Err(error) = queue.try_send(job) {
            let job = match error {
                std::sync::mpsc::TrySendError::Full(j) | std::sync::mpsc::TrySendError::Disconnected(j) => j,
            };
            app.lock().map_err(|_| err())?.cancel_job(&job)?;
        }
    }
    response(
        stream,
        200,
        "application/json",
        obj! {"ok"=>true,"result"=>value,"generation"=>generation}.to_json().as_bytes(),
    )
}
pub fn serve(seed: u32, port: u16, cap: u64, config: Config, dir: PathBuf) -> Result<()> {
    serve_lan(seed, port, cap, config, dir, None)
}
pub fn serve_lan(
    seed: u32,
    port: u16,
    cap: u64,
    config: Config,
    dir: PathBuf,
    address: Option<std::net::Ipv4Addr>,
) -> Result<()> {
    let session = address.map(super::session::Session::new).transpose()?.map(|s| Arc::new(Mutex::new(s)));
    let app = Arc::new(Mutex::new(App::new(seed, cap, config, dir.clone())?));
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).map_err(|_| err())?;
    let port = listener.local_addr().map_err(|_| err())?.port();
    let lan_listener = if let Some(address) = address {
        Some(TcpListener::bind((address, port)).map_err(|_| Error::new("Could not bind that LAN address/port."))?)
    } else {
        None
    };
    let (tx, rx) = sync_channel(8);
    let state = app.clone();
    let manager = super::manager_runtime::Runtime::new(&dir)?;
    std::thread::spawn(move || worker(state, rx, providers::Network { dir, manager }));
    if let Ok(mut a) = app.lock() {
        if a.manager.policy.enabled() {
            if let Ok((_, jobs)) = a.command("manager_action", &obj! {"action"=>"refresh","profile"=>"text"}) {
                for j in jobs {
                    let _ = tx.try_send(j);
                }
            }
        }
    }
    println!("Lantern Vault studio: http://127.0.0.1:{port}/ (authorized cap: {cap} microdollars)");
    if let Some(s) = &session {
        println!("LAN invite: {}", s.lock().map_err(|_| err())?.info(port).to_json());
    }
    let active = Arc::new(AtomicUsize::new(0));
    if let Some(lan_listener) = lan_listener {
        let app = app.clone();
        let tx = tx.clone();
        let active = active.clone();
        let session = session.clone();
        std::thread::spawn(move || accept(lan_listener, app, tx, port, active, session));
    }
    accept(listener, app, tx, port, active, session)
}
fn accept(
    listener: TcpListener,
    app: Arc<Mutex<App>>,
    tx: SyncSender<Job>,
    port: u16,
    active: Arc<AtomicUsize>,
    session: Option<Arc<Mutex<super::session::Session>>>,
) -> Result<()> {
    for connection in listener.incoming() {
        let mut stream = connection.map_err(|_| err())?;
        if active.load(Ordering::SeqCst) >= 8 {
            let _ = response(&mut stream, 503, "text/plain", b"Studio busy.");
            continue;
        }
        active.fetch_add(1, Ordering::SeqCst);
        let active = active.clone();
        let app = app.clone();
        let queue = tx.clone();
        let session = session.clone();
        std::thread::spawn(move || {
            if handle(&mut stream, &app, &queue, port, &session).is_err() {
                let _ = response(
                    &mut stream,
                    400,
                    "application/json",
                    b"{\"ok\":false,\"error\":\"Rejected or stale studio request.\"}",
                );
            }
            active.fetch_sub(1, Ordering::SeqCst);
        });
    }
    Ok(())
}
#[cfg(test)]
pub(super) fn run_worker(app: Arc<Mutex<App>>, rx: std::sync::mpsc::Receiver<Job>, transport: impl Transport) {
    worker(app, rx, transport)
}
