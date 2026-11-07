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
    write!(stream,"HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' blob:; media-src 'self' blob:; connect-src 'self'; frame-ancestors 'none'\r\n\r\n",status,if status==200{"OK"}else{"Rejected"},mime,body.len()).map_err(|_|err())?;
    stream.write_all(body).map_err(|_| err())?;
    Ok(())
}
fn worker(app: Arc<Mutex<App>>, rx: std::sync::mpsc::Receiver<Job>, mut transport: impl Transport) {
    let mut followups = std::collections::VecDeque::new();
    loop {
        let job = if let Some(j) = followups.pop_front() {
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
        let ready = app.lock().ok().and_then(|mut a| a.before_dispatch(&job).ok()).unwrap_or(false);
        if !ready {
            continue;
        }
        // The only provider execution lane: local text, diffusion and audio never overlap.
        let result = transport.send(&job.request);
        let manager_status = transport.manager_status();
        if let Ok(mut a) = app.lock() {
            if let Some(report) = manager_status {
                let _ = a.manager_observe(report);
            }
            match a.complete(&job, result) {
                Ok(next) => followups.extend(next),
                Err(_) => eprintln!("Studio completion rejected; spending remains reserved."),
            }
        }
    }
}
fn handle(stream: &mut TcpStream, app: &Arc<Mutex<App>>, queue: &SyncSender<Job>, port: u16) -> Result<()> {
    let request = read(stream)?;
    let host = format!("127.0.0.1:{port}");
    let header = |name: &str| request.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str());
    if header("host") != Some(host.as_str()) || header("origin").is_some_and(|o| o != format!("http://{host}")) {
        return response(stream, 403, "text/plain", b"Loopback origin required.");
    }
    if request.method == "GET" {
        let static_file = match request.path.as_str() {
            "/" => Some(("text/html; charset=utf-8", include_bytes!("index.html").as_slice())),
            "/studio.css" => Some(("text/css", include_bytes!("studio.css").as_slice())),
            "/studio.mjs" => Some(("text/javascript", include_bytes!("studio.mjs").as_slice())),
            "/context.mjs" => Some(("text/javascript", include_bytes!("context.mjs").as_slice())),
            "/models.mjs" => Some(("text/javascript", include_bytes!("models.mjs").as_slice())),
            "/setup.mjs" => Some(("text/javascript", include_bytes!("setup.mjs").as_slice())),
            "/voice.mjs" => Some(("text/javascript", include_bytes!("voice.mjs").as_slice())),
            _ => None,
        };
        if let Some((mime, body)) = static_file {
            return response(stream, 200, mime, body);
        }
        if request.path == "/api/state" {
            let v = app.lock().map_err(|_| err())?.state()?;
            return response(stream, 200, "application/json", v.to_json().as_bytes());
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
    let (value, jobs) = match current.command(op, &args) {
        Ok(result) => result,
        Err(_) => {
            let epoch = current.epoch;
            drop(current);
            return response(stream,400,"application/json",obj! {"ok"=>false,"error"=>if op=="settings"{"Settings were not applied. Check the model name, voice values and authorized spending cap."}else{"That request could not be applied to the current game. Refresh and try again."},"generation"=>epoch}.to_json().as_bytes());
        }
    };
    let generation = current.epoch;
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
    let app = Arc::new(Mutex::new(App::new(seed, cap, config, dir.clone())?));
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).map_err(|_| err())?;
    let port = listener.local_addr().map_err(|_| err())?.port();
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
    let active = Arc::new(AtomicUsize::new(0));
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
        std::thread::spawn(move || {
            if handle(&mut stream, &app, &queue, port).is_err() {
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
