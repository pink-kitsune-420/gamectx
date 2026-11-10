//! Offline fake transports exercise production routing/state, never installed services.
use super::*;
use gamectx::{media::providers::ProviderBudget, obj};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        mpsc::{channel, sync_channel},
        Arc, Mutex,
    },
};
fn array(v: &Value) -> Result<&[Value]> {
    v.as_array().ok_or_else(err)
}
struct Dir(std::path::PathBuf);
impl Dir {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("gamectx-web-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn capture() -> Vec<u8> {
    let mut wav = vec![0u8; 364];
    wav[..4].copy_from_slice(b"RIFF");
    wav[4..8].copy_from_slice(&356u32.to_le_bytes());
    wav[8..16].copy_from_slice(b"WAVEfmt ");
    wav[16..20].copy_from_slice(&16u32.to_le_bytes());
    wav[20..24].copy_from_slice(&[1, 0, 1, 0]);
    wav[24..28].copy_from_slice(&16000u32.to_le_bytes());
    wav[28..32].copy_from_slice(&32000u32.to_le_bytes());
    wav[32..36].copy_from_slice(&[2, 0, 16, 0]);
    wav[36..40].copy_from_slice(b"data");
    wav[40..44].copy_from_slice(&320u32.to_le_bytes());
    wav
}
fn config() -> providers::Config {
    let mut c = providers::Config::parse(include_str!("providers.json")).unwrap();
    set(&mut c.value, "speech_mode", "dialogue".into());
    c
}
fn app(dir: &Dir) -> app::App {
    app::App::new(7, 0, config(), dir.0.clone()).unwrap()
}
fn command(a: &mut app::App, op: &str, args: Value) -> (Value, Vec<app::Job>) {
    let mut args = args;
    if op == "repair" {
        let state = a.state().unwrap();
        let target = if text(&args, "target").unwrap() == "scene" { "scene_asset" } else { "portraits" };
        set(&mut args, "key", get(get(get(&state, target).unwrap(), "selection").unwrap(), "key").unwrap().clone());
    }
    a.command(op, &args).unwrap()
}
fn travel(a: &mut app::App) {
    for target in ["room:gallery", "room:workshop"] {
        command(a, "act", obj! {"revision"=>a.game.revision,"decision"=>obj!{"action"=>"go","target"=>target}});
    }
}
fn backend(a: &mut app::App, modality: &str, name: &str) {
    let mut settings = a.config.public();
    set(&mut settings, modality, name.into());
    if let Value::Object(xs) = &mut settings {
        xs.retain(|(k, _)| k != "local_fallback" && k != "text_models");
    }
    set(&mut settings, "cap_micros", 0.into());
    command(a, "settings", settings);
}
#[test]
fn base64_roundtrip_and_invalid_padding() {
    for n in 0..130 {
        let bytes = (0..n).map(|x| x as u8).collect::<Vec<_>>();
        assert_eq!(providers::decode64(&providers::encode64(&bytes)).unwrap(), bytes);
    }
    for s in ["AA=A", "AAA", "====", "A===", "AB==", "AAA=AAAA", "AA?="] {
        assert!(providers::decode64(s).is_err(), "{s}");
    }
    providers::validate_capture(&capture()).unwrap();
    let mut too_long = capture();
    too_long.resize(320046, 0);
    assert!(providers::validate_capture(&too_long).is_err());
    let mut lying = capture();
    lying[40..44].copy_from_slice(&1u32.to_le_bytes());
    assert!(providers::validate_capture(&lying).is_err());
    assert_eq!(parse_cap("0.000001").unwrap(), 1);
    assert_eq!(parse_cap("1.25").unwrap(), 1_250_000);
    for s in ["-1", "NaN", "1e2", "0.0000001"] {
        assert!(parse_cap(s).is_err());
    }
}
#[test]
fn shared_cumulative_budget_survives_crash_and_cap_changes() {
    let mut budget = ProviderBudget::new(20).unwrap();
    let first = budget.reserve("text", "online_openai_compatible", 10, false).unwrap();
    let id = num(&first, "reservation", 20).unwrap();
    budget.dispatch(id).unwrap();
    budget.limit(15).unwrap();
    assert!(budget.cancel(id).is_err());
    let mut restored = ProviderBudget::recover(&budget.checkpoint(), 20).unwrap();
    let r = restored.reserve("tts", "online_tts_api", 11, false).unwrap();
    assert_eq!(text(&r, "backend").unwrap(), "browser_web_speech");
    assert_eq!(num(&restored.summary(), "charged_or_reserved_micros", 100).unwrap(), 10);
    let mut offline = ProviderBudget::recover(&budget.checkpoint(), 0).unwrap();
    assert_eq!(
        text(&offline.reserve("stt", "online_stt_api", 1, false).unwrap(), "backend").unwrap(),
        "browser_speech_recognition"
    );
}
#[test]
fn model_tier_selects_actual_configured_model_and_transport_formats() {
    let dir = Dir::new();
    let mut a = app(&dir);
    travel(&mut a);
    set(&mut a.config.value, "size", "3B".into());
    backend(&mut a, "text", "local_ollama");
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Do you trust me?"});
    assert_eq!(jobs.len(), 1);
    assert_eq!(
        text(&parse(std::str::from_utf8(&jobs[0].request.body).unwrap()).unwrap(), "model").unwrap(),
        "qwen2.5:3b"
    );
    let mut c = config();
    let r = providers::prepare(
        &c,
        "local_ollama",
        "text",
        &obj! {"messages"=>vec![obj!{"role"=>"user","content"=>"Hello."}],"max_tokens"=>64},
    )
    .unwrap();
    let json = parse(std::str::from_utf8(&r.body).unwrap()).unwrap();
    assert_eq!(text(&json, "model").unwrap(), "qwen2.5:3b");
    assert_eq!(get(&json, "keep_alive").unwrap(), &Value::from(0));
    set(&mut c.value, "size", "8".into());
    assert!(providers::prepare(&c, "local_ollama", "text", &obj! {"messages"=>Vec::<Value>::new(),"max_tokens"=>64})
        .is_err());
    let mut models = get(&c.value, "text_models").unwrap().clone();
    let mut local = get(&models, "local_ollama").unwrap().clone();
    set(&mut local, "8", "host-8b-profile".into());
    set(&mut models, "local_ollama", local);
    set(&mut c.value, "text_models", models);
    let r = providers::prepare(&c, "local_ollama", "text", &obj! {"messages"=>Vec::<Value>::new(),"max_tokens"=>128})
        .unwrap();
    assert!(String::from_utf8(r.body).unwrap().contains("host-8b-profile"));
    let r = providers::prepare(
        &c,
        "local_stt_http",
        "stt",
        &obj! {"audio"=>providers::encode64(&capture()),"mime"=>"audio/wav"},
    )
    .unwrap();
    assert!(r.content_type.starts_with("multipart/form-data"));
    assert!(r.body.windows(4).any(|b| b == b"file"));
    let r = providers::prepare(
        &c,
        "local_tts_http",
        "tts",
        &obj! {"text"=>"One word.","voice"=>c.voice("keeper").unwrap().clone()},
    )
    .unwrap();
    let j = parse(std::str::from_utf8(&r.body).unwrap()).unwrap();
    assert_eq!(text(&j, "voice").unwrap(), "onyx");
    assert_eq!(text(&j, "response_format").unwrap(), "wav");
}
#[test]
fn local_http_transport_uses_only_loopback_and_decodes_chunking() {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let join = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut header = Vec::new();
        let mut b = [0; 1];
        while !header.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut b).unwrap();
            header.push(b[0]);
        }
        let header = String::from_utf8(header).unwrap();
        assert!(header.starts_with("POST /api/chat HTTP/1.1"));
        let n = header.lines().find_map(|l| l.strip_prefix("Content-Length: ")).unwrap().parse::<usize>().unwrap();
        let mut body = vec![0; n];
        stream.read_exact(&mut body).unwrap();
        assert!(String::from_utf8(body).unwrap().contains("keep_alive"));
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2\r\n{}\r\n0\r\n\r\n",
            )
            .unwrap();
    });
    let mut c = config();
    let mut endpoints = get(&c.value, "endpoints").unwrap().clone();
    let mut e = get(&endpoints, "local_ollama").unwrap().clone();
    set(&mut e, "url", format!("http://127.0.0.1:{port}/api/chat").into());
    set(&mut endpoints, "local_ollama", e);
    set(&mut c.value, "endpoints", endpoints);
    let request =
        providers::prepare(&c, "local_ollama", "text", &obj! {"messages"=>Vec::<Value>::new(),"max_tokens"=>64})
            .unwrap();
    let d = Dir::new();
    let mut t = providers::Network { dir: d.0.clone(), manager: super::manager_runtime::Runtime::new(&d.0).unwrap() };
    assert_eq!(providers::Transport::send(&mut t, &request).unwrap(), b"{}");
    join.join().unwrap();
    let mut v = c.value.clone();
    let mut eps = get(&v, "endpoints").unwrap().clone();
    let mut e = get(&eps, "local_ollama").unwrap().clone();
    set(&mut e, "url", "http://192.0.2.1:11434/api/chat".into());
    set(&mut eps, "local_ollama", e);
    set(&mut v, "endpoints", eps);
    assert!(providers::Config::parse(&v.to_json()).is_err());
}
#[test]
fn text_completion_cannot_upgrade_started_or_interrupted_speech() {
    let d = Dir::new();
    let mut a = app(&d);
    travel(&mut a);
    backend(&mut a, "text", "local_ollama");
    let (_, mut jobs) = command(&mut a, "chat", obj! {"text"=>"Tell me about stock."});
    let job = jobs.remove(0);
    let state = a.state().unwrap();
    let id = get(get(&state, "speech").unwrap(), "id").unwrap().clone();
    assert!(a.before_dispatch(&job).unwrap());
    command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"queued","through"=>0,"heard"=>0});
    command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"played","through"=>8,"heard"=>5});
    command(&mut a, "delivery", obj! {"id"=>id,"event"=>"interrupted","through"=>8,"heard"=>5});
    a.complete(&job, Ok(br#"{"message":{"content":"This late answer must never replace played audio."}}"#.to_vec()))
        .unwrap();
    let state = a.state().unwrap();
    let s = get(&state, "speech").unwrap();
    assert!(!text(s, "text").unwrap().contains("late answer"));
    assert_eq!(num(get(s, "delivery").unwrap(), "heard", 240).unwrap(), 5);
    assert!(get(&state, "transcript").unwrap().to_json().contains("heard interrupted prefix"));
    let confirmed = text(get(s, "delivery").unwrap(), "heard_text").unwrap().to_owned();
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Again."});
    let payload = parse(std::str::from_utf8(&jobs[0].request.body).unwrap()).unwrap();
    let assistant = get(&payload, "messages")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m.get("role") == Some(&Value::str("assistant")))
        .collect::<Vec<_>>();
    assert_eq!(assistant.len(), 1);
    assert_eq!(text(assistant[0], "content").unwrap(), format!("[merchant -> player_1] {confirmed}"));
}
#[test]
fn single_worker_does_not_hold_state_lock_or_overlap_models() {
    struct Fake {
        started: std::sync::mpsc::Sender<()>,
        release: std::sync::mpsc::Receiver<()>,
        active: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl providers::Transport for Fake {
        fn send(&mut self, r: &providers::Request) -> Result<Vec<u8>> {
            use std::sync::atomic::Ordering;
            assert_eq!(self.active.fetch_add(1, Ordering::SeqCst), 0);
            if r.url.ends_with("/api/chat") {
                self.started.send(()).unwrap();
                self.release.recv().unwrap();
            }
            if r.url.contains("txt2img") {
                assert!(std::str::from_utf8(&r.body).unwrap().contains("Palette:"));
            }
            let bytes = if r.url.ends_with("/api/chat") {
                br#"{"message":{"content":"Only a completed short response."}}"#.to_vec()
            } else {
                obj! {"images"=>vec![providers::encode64(include_bytes!("fixtures/flat.png"))]}.to_json().into_bytes()
            };
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(bytes)
        }
    }
    let d = Dir::new();
    let mut a = app(&d);
    travel(&mut a);
    backend(&mut a, "text", "local_ollama");
    backend(&mut a, "image", "local_sd_cpp");
    let (_, text) = command(&mut a, "chat", obj! {"text"=>"Hello."});
    let (_, image) = command(&mut a, "generate", obj! {"target"=>"portrait"});
    let state = Arc::new(Mutex::new(a));
    let (tx, rx) = sync_channel(8);
    let (started, ready) = channel();
    let (release, blocked) = channel();
    let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let s = state.clone();
    let join = std::thread::spawn(move || http::run_worker(s, rx, Fake { started, release: blocked, active }));
    for j in text.into_iter().chain(image) {
        tx.send(j).unwrap();
    }
    ready.recv_timeout(std::time::Duration::from_secs(3)).unwrap();
    {
        let mut a = state.lock().unwrap();
        let s = a.state().unwrap();
        let id = get(get(&s, "speech").unwrap(), "id").unwrap().clone();
        command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"queued","through"=>0,"heard"=>0});
        command(&mut a, "delivery", obj! {"id"=>id,"event"=>"interrupted","through"=>0,"heard"=>0});
    }
    release.send(()).unwrap();
    drop(tx);
    join.join().unwrap();
    let mut a = state.lock().unwrap();
    let v = a.state().unwrap();
    let portrait = get(&v, "portraits").unwrap();
    let candidate = get(get(portrait, "inspection").unwrap(), "candidate").unwrap();
    assert_ne!(candidate, &Value::Null);
    assert_eq!(get(get(portrait, "selection").unwrap(), "asset").unwrap(), &Value::Null);
    let key = get(get(portrait, "selection").unwrap(), "key").unwrap().clone();
    command(&mut a, "approve", obj! {"key"=>key});
    let s = a.state().unwrap();
    assert_eq!(
        get(get(get(&s, "portraits").unwrap(), "selection").unwrap(), "state").unwrap(),
        &Value::str("approved")
    );
}
#[test]
fn http_stt_final_routes_once_and_interim_never_changes_world() {
    let d = Dir::new();
    let mut a = app(&d);
    travel(&mut a);
    backend(&mut a, "stt", "local_stt_http");
    let (s, _) = command(&mut a, "mic", obj! {});
    let session = get(&s, "session").unwrap().clone();
    let before = a.game.revision;
    command(
        &mut a,
        "recognition",
        obj! {"session"=>session.clone(),"text"=>"buy oil","final"=>false,"uncertainty"=>Value::Null},
    );
    assert_eq!(a.game.revision, before);
    let (_, mut jobs) = command(
        &mut a,
        "transcribe",
        obj! {"session"=>session.clone(),"audio"=>providers::encode64(&capture()),"mime"=>"audio/wav"},
    );
    let job = jobs.remove(0);
    assert!(a.before_dispatch(&job).unwrap());
    a.complete(&job, Ok(br#"{"text":"Buy the oil."}"#.to_vec())).unwrap();
    assert!(a.game.owns("item:oil"));
    assert_eq!(a.game.revision, before + 1);
    command(&mut a, "mic", obj! {});
    assert!(a
        .command("recognition", &obj! {"session"=>session,"text"=>"buy chalk","final"=>true,"uncertainty"=>Value::Null})
        .is_err());
}
#[test]
fn passive_assets_and_branch_load_keep_generation_and_spend_separate() {
    let d = Dir::new();
    let mut a = app(&d);
    travel(&mut a);
    let v = a.state().unwrap();
    assert_eq!(get(get(get(&v, "scene_asset").unwrap(), "selection").unwrap(), "generation").unwrap(), &Value::Null);
    command(&mut a, "save", obj! {"slot"=>0});
    let before = a.budget.summary();
    let revision = a.game.revision;
    command(&mut a, "act", obj! {"revision"=>revision,"decision"=>obj!{"action"=>"go","target"=>"room:gallery"}});
    command(&mut a, "load", obj! {"slot"=>0});
    assert_eq!(a.budget.summary(), before);
    assert_eq!(a.game.room, "workshop");
    let file = a.dir.join("branch-0.json");
    let mut saved = parse(&std::fs::read_to_string(&file).unwrap()).unwrap();
    set(&mut saved, "pack_digest", "foreign".into());
    std::fs::write(&file, saved.to_json()).unwrap();
    let before = a.game.public();
    assert!(a.command("load", &obj! {"slot"=>0}).is_err());
    assert_eq!(a.game.public(), before);
}
#[test]
fn pin_only_repairs_replay_once_and_cache_survives_restart() {
    let d = Dir::new();
    let mut a = app(&d);
    travel(&mut a);
    backend(&mut a, "image", "local_sd_cpp");
    let (_, mut pending) = command(&mut a, "generate", obj! {"target"=>"portrait"});
    let old = pending.remove(0);
    let (duplicate, jobs) = command(&mut a, "generate", obj! {"target"=>"portrait"});
    assert_eq!(get(&duplicate, "queued").unwrap(), &Value::Bool(false));
    assert!(jobs.is_empty());
    assert!(a.before_dispatch(&old).unwrap());
    let patch = obj! {"replace_tokens"=>Vec::<Value>::new(),"add_tokens"=>Vec::<Value>::new(),"negative_tokens"=>Vec::<Value>::new(),"forbid_props"=>Vec::<Value>::new(),"seed_delta"=>0,"asset"=>Value::Null};
    command(
        &mut a,
        "repair",
        obj! {"target"=>"portrait","patch"=>patch,"image"=>providers::encode64(include_bytes!("fixtures/flat.png"))},
    );
    a.complete(
        &old,
        Ok(obj! {"images"=>vec![providers::encode64(include_bytes!("fixtures/flat.png"))]}.to_json().into_bytes()),
    )
    .unwrap();
    for _ in 0..12 {
        let state = a.state().unwrap();
        assert_eq!(text(get(get(&state, "portraits").unwrap(), "selection").unwrap(), "state").unwrap(), "pinned");
    }
    drop(a);
    let mut loaded = app(&d);
    travel(&mut loaded);
    for _ in 0..12 {
        let state = loaded.state().unwrap();
        assert_eq!(text(get(get(&state, "portraits").unwrap(), "selection").unwrap(), "state").unwrap(), "pinned");
    }
}
#[test]
fn bounded_variant_capacity_does_not_stall_gameplay_or_enqueue_art() {
    let d = Dir::new();
    let mut a = app(&d);
    travel(&mut a);
    command(&mut a, "save", obj! {"slot"=>0});
    for _ in 0..70 {
        command(&mut a, "load", obj! {"slot"=>0});
        let state = a.state().unwrap();
        assert_eq!(
            get(get(get(&state, "scene_asset").unwrap(), "selection").unwrap(), "generation").unwrap(),
            &Value::Null
        );
    }
    let (r, jobs) = command(&mut a, "generate", obj! {"target"=>"portrait"});
    assert_eq!(get(&r, "queued").unwrap(), &Value::Bool(false));
    assert!(jobs.is_empty());
    let revision = a.game.revision;
    command(&mut a, "act", obj! {"revision"=>revision,"decision"=>obj!{"action"=>"go","target"=>"room:gallery"}});
    assert_eq!(a.game.room, "gallery");
}
#[test]
fn invalid_provider_output_preserves_fallback_and_never_confirms_audio_tail() {
    let d = Dir::new();
    let mut a = app(&d);
    travel(&mut a);
    backend(&mut a, "text", "local_ollama");
    backend(&mut a, "tts", "local_tts_http");
    let (_, mut jobs) = command(&mut a, "chat", obj! {"text"=>"What is listed?"});
    let job = jobs.remove(0);
    assert!(a.before_dispatch(&job).unwrap());
    a.complete(&job, Ok(br#"{"message":{"content":"A cut-off clause,"}}"#.to_vec())).unwrap();
    let state = a.state().unwrap();
    let speech = get(&state, "speech").unwrap();
    assert!(!text(speech, "text").unwrap().contains("cut-off"));
    let id = get(speech, "id").unwrap().clone();
    command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"queued","through"=>0,"heard"=>0});
    let (_, mut jobs) = command(&mut a, "synthesize", obj! {"id"=>id});
    let job = jobs.remove(0);
    assert!(a.before_dispatch(&job).unwrap());
    a.complete(&job, Ok(b"invalid WAV".to_vec())).unwrap();
    let state = a.state().unwrap();
    let speech = get(&state, "speech").unwrap();
    assert_eq!(text(get(speech, "audio").unwrap(), "fallback").unwrap(), "browser_web_speech");
    assert_eq!(num(get(speech, "delivery").unwrap(), "heard", 240).unwrap(), 0);
    let id = get(speech, "id").unwrap().clone();
    let (_, mut jobs) = command(&mut a, "synthesize", obj! {"id"=>id});
    let failed = jobs.remove(0);
    assert!(a.before_dispatch(&failed).unwrap());
    a.complete(&failed, Err(err())).unwrap();
    let state = a.state().unwrap();
    assert_eq!(get(&state, "local_paused").unwrap(), &Value::Bool(true));
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Hello again."});
    assert!(jobs.is_empty());
    command(&mut a, "resume_local", obj! {});
}

#[test]
fn setup_checks_find_installed_names_and_do_not_generate_or_pause_on_refusal() {
    let dir = Dir::new();
    let mut a = app(&dir);
    backend(&mut a, "text", "local_ollama");
    let (_, jobs) = command(&mut a, "check_providers", obj! {});
    assert_eq!(jobs.len(), 4);
    assert!(matches!(jobs[0].request.method, providers::Method::Get));
    assert!(jobs[0].request.url.ends_with("/api/tags"));
    for job in jobs {
        assert!(job.reservation.is_none());
        assert!(a.before_dispatch(&job).unwrap());
        let response = if job.backend == "local_ollama" {
            Ok(br#"{"models":[{"name":"installed:3b"}]}"#.to_vec())
        } else {
            Err(Error::new("provider_connection"))
        };
        a.complete(&job, response).unwrap();
    }
    let s = a.state().unwrap();
    let setup = get(&s, "setup").unwrap();
    assert_eq!(get(setup, "installed_models").unwrap(), &vec!["installed:3b"].into());
    assert_eq!(text(&array(get(setup, "channels").unwrap()).unwrap()[0], "status").unwrap(), "missing_model");
    assert_eq!(get(&s, "local_paused").unwrap(), &Value::Bool(false));
    let mut settings = a.config.public();
    if let Value::Object(xs) = &mut settings {
        xs.retain(|(k, _)| !["local_fallback", "text_models"].contains(&k.as_str()));
    }
    set(&mut settings, "cap_micros", 0.into());
    set(&mut settings, "model", "installed:3b".into());
    command(&mut a, "settings", settings.clone());
    assert_eq!(super::setup::selected_model(&a.config), "installed:3b");
    assert_eq!(
        text(&array(get(get(&a.state().unwrap(), "setup").unwrap(), "channels").unwrap()).unwrap()[0], "status")
            .unwrap(),
        "ready"
    );
    let before = a.config.value.clone();
    set(&mut settings, "model", "bad model\n".into());
    assert!(a.command("settings", &settings).is_err());
    assert_eq!(a.config.value, before);
    assert_eq!(num(&a.budget.summary(), "charged_or_reserved_micros", 100).unwrap(), 0);
}

#[test]
fn missing_model_has_visible_failure_and_does_not_pause_all_local_services() {
    let dir = Dir::new();
    let mut a = app(&dir);
    travel(&mut a);
    backend(&mut a, "text", "local_ollama");
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Hello."});
    let job = &jobs[0];
    a.before_dispatch(job).unwrap();
    a.complete(job, Err(Error::new("provider_http_404"))).unwrap();
    let s = a.state().unwrap();
    assert_eq!(get(&s, "local_paused").unwrap(), &Value::Bool(false));
    assert!(text(&s, "notice").unwrap().contains("exact model name"));
    assert_eq!(
        text(&array(get(get(&s, "setup").unwrap(), "activity").unwrap()).unwrap()[0], "status").unwrap(),
        "failed"
    );
    assert_eq!(text(get(&s, "speech").unwrap(), "origin").unwrap(), "offline");
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Try this reply."});
    assert_eq!(jobs.len(), 1);
    a.before_dispatch(&jobs[0]).unwrap();
    a.complete(&jobs[0], Ok(br#"{"message":{"content":"I tend this counter."}}"#.to_vec())).unwrap();
    let s = a.state().unwrap();
    assert_eq!(
        text(&array(get(get(&s, "setup").unwrap(), "channels").unwrap()).unwrap()[0], "status").unwrap(),
        "ready"
    );
}

#[test]
fn manager_online_profiles_use_existing_reservations_without_local_lifecycle_calls() {
    let dir = Dir::new();
    let mut c = config();
    let mut endpoints = get(&c.value, "endpoints").unwrap().clone();
    for backend in ["online_openai_compatible", "online_image_api"] {
        let mut endpoint = get(&endpoints, backend).unwrap().clone();
        set(&mut endpoint, "key_env", "GAMECTX_MANAGER_FIXTURE_KEY".into());
        set(&mut endpoint, "ceiling_micros", 4.into());
        set(&mut endpoints, backend, endpoint);
    }
    set(&mut c.value, "endpoints", endpoints);
    std::env::set_var("GAMECTX_MANAGER_FIXTURE_KEY", "offline-fixture");
    let mut a = app::App::new(7, 20, c, dir.0.clone()).unwrap();
    travel(&mut a);
    let mut policy = a.manager.policy.to_value();
    set(&mut policy, "enabled", true.into());
    set(&mut policy, "mode", "dramatic".into());
    set(&mut policy, "text_escalation", true.into());
    set(&mut policy, "image_escalation", true.into());
    let mut profiles = get(&policy, "profiles").unwrap().clone();
    for (id, backend, model) in [
        ("dramatic_text", "online_openai_compatible", "online-dialogue"),
        ("dramatic_image", "online_image_api", "online-art"),
    ] {
        let mut p = get(&profiles, id).unwrap().clone();
        set(&mut p, "backend", backend.into());
        set(&mut p, "model", model.into());
        set(&mut p, "ram_mib", 65536.into());
        set(&mut profiles, id, p);
    }
    set(&mut policy, "profiles", profiles);
    a.manager.policy = gamectx::deploy::model_policy::ModelPolicy::from_value(&policy).unwrap();
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Hello."});
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0].reservation.is_some());
    assert!(jobs[0].request.management.is_none());
    assert!(jobs[0].request.url.starts_with("https://") || jobs[0].request.url.starts_with("http://127.0.0.1:"));
    let body = parse(std::str::from_utf8(&jobs[0].request.body).unwrap()).unwrap();
    assert_eq!(text(&body, "model").unwrap(), "online-dialogue");
    a.cancel_job(&jobs[0]).unwrap();
    let (_, jobs) = command(&mut a, "generate", obj! {"target"=>"portrait"});
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0].reservation.is_some());
    assert!(jobs[0].request.management.is_none());
    let body = parse(std::str::from_utf8(&jobs[0].request.body).unwrap()).unwrap();
    assert_eq!(text(&body, "model").unwrap(), "online-art");
    a.cancel_job(&jobs[0]).unwrap();
    a.budget.limit(0).unwrap();
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Hello again."});
    assert!(jobs.is_empty());
    assert_eq!(num(&a.budget.summary(), "charged_or_reserved_micros", 100).unwrap(), 0);
    std::env::remove_var("GAMECTX_MANAGER_FIXTURE_KEY");
}

#[test]
fn previous_art_preview_is_subject_scene_and_hard_prop_scoped() {
    let base = obj! {"scope"=>"scene","observer"=>"player_1","audience"=>vec!["player_1"],"entity"=>"merchant","scene"=>"workshop","scene_epoch"=>1,"format"=>"portrait","silhouette"=>"original figure","attire"=>"blue coat","visible_props"=>vec!["oil"],"environment"=>Vec::<Value>::new(),"expression"=>obj!{"slot"=>"neutral"},"pose"=>obj!{"id"=>"idle"},"forbidden_props"=>Vec::<Value>::new()};
    assert!(super::manager_view::preview_compatible(&base, &base));
    for (key, value) in [
        ("observer", "player_2".into()),
        ("scene_epoch", 2.into()),
        ("attire", "armor".into()),
        ("forbidden_props", vec!["oil"].into()),
    ] {
        let mut changed = base.clone();
        set(&mut changed, key, value);
        assert!(!super::manager_view::preview_compatible(&base, &changed));
    }
}

#[test]
fn new_game_preserves_settings_spend_cache_saves_and_invalidates_inflight_speech() {
    let dir = Dir::new();
    let mut a = app::App::new(7, 20, config(), dir.0.clone()).unwrap();
    let reservation = a.budget.reserve("text", "online_openai_compatible", 5, false).unwrap();
    let token = num(&reservation, "reservation", 100).unwrap();
    a.budget.dispatch(token).unwrap();
    a.budget.finish(token).unwrap();
    a.persist_budget().unwrap();
    travel(&mut a);
    command(&mut a, "promise", obj! {});
    command(&mut a, "save", obj! {"slot"=>0});
    let saved = std::fs::read(dir.0.join("branch-0.json")).unwrap();
    set(&mut a.config.value, "text", "local_ollama".into());
    let config = a.config.value.clone();
    let budget = a.budget.checkpoint();
    let patch = obj! {"replace_tokens"=>Vec::<Value>::new(),"add_tokens"=>Vec::<Value>::new(),"negative_tokens"=>Vec::<Value>::new(),"forbid_props"=>Vec::<Value>::new(),"seed_delta"=>1,"asset"=>Value::Null};
    command(
        &mut a,
        "repair",
        obj! {"target"=>"portrait","patch"=>patch,"image"=>providers::encode64(include_bytes!("fixtures/flat.png"))},
    );
    let cache = std::fs::read(dir.0.join("assets/index.json")).unwrap();
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Hello again."});
    assert!(a.before_dispatch(&jobs[0]).unwrap());
    let old_id = get(get(&a.state().unwrap(), "speech").unwrap(), "id").unwrap().clone();
    let epoch = a.epoch;
    command(&mut a, "reset", obj! {});
    assert!(a.epoch > epoch);
    assert_eq!(a.game.room, "entry");
    assert_eq!(a.game.coins, 5);
    assert_eq!(a.config.value, config);
    assert_eq!(a.budget.checkpoint(), budget);
    assert_eq!(std::fs::read(dir.0.join("assets/index.json")).unwrap(), cache);
    assert_eq!(std::fs::read(dir.0.join("branch-0.json")).unwrap(), saved);
    a.complete(&jobs[0], Ok(br#"{"message":{"content":"A late response."}}"#.to_vec())).unwrap();
    let s = a.state().unwrap();
    assert_eq!(get(&s, "speech").unwrap(), &Value::Null);
    assert_eq!(num(&s, "pending_jobs", 8).unwrap(), 0);
    assert!(array(get(&s, "promises").unwrap())
        .unwrap()
        .iter()
        .all(|p| array(get(p, "accepted").unwrap()).unwrap().is_empty()));
    assert!(a.command("delivery", &obj! {"id"=>old_id,"event"=>"finished","through"=>0,"heard"=>0}).is_err());
    command(&mut a, "load", obj! {"slot"=>0});
    assert_eq!(a.game.room, "workshop");
}

#[test]
fn thousand_delivered_turns_keep_transient_dialogue_out_of_rumor_capacity() {
    let dir = Dir::new();
    let mut a = app(&dir);
    travel(&mut a);
    for turn in 0..1000 {
        command(&mut a, "chat", obj! {"text"=>format!("Please listen to counter thought {turn}.")});
        let state = a.state().unwrap();
        let speech = get(&state, "speech").unwrap();
        let id = get(speech, "id").unwrap().clone();
        let through = text(speech, "text").unwrap().chars().count();
        command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"queued","through"=>0,"heard"=>0});
        command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"played","through"=>through,"heard"=>through});
        command(&mut a, "delivery", obj! {"id"=>id,"event"=>"finished","through"=>through,"heard"=>through});
        if turn % 200 == 199 {
            command(&mut a, "save", obj! {"slot"=>0});
            command(&mut a, "load", obj! {"slot"=>0});
        }
    }
    let state = a.state().unwrap();
    assert_eq!(get(&state, "pending_jobs").unwrap(), &0.into());
    assert!(get(&state, "rumors").unwrap().as_array().unwrap().is_empty());
    let life = get(&get(&state, "characters").unwrap().as_array().unwrap()[0], "life").unwrap();
    assert!(get(get(life, "base").unwrap(), "events").unwrap().as_array().unwrap().len() <= 128);
    assert_eq!(get(get(&state, "spending").unwrap(), "charged_or_reserved_micros").unwrap(), &0.into());
}

#[test]
fn grounded_reply_contract_rejects_invention_and_scopes_promises_and_episodes() {
    let dir = Dir::new();
    let mut a = app(&dir);
    travel(&mut a);
    set(&mut a.config.value, "speech_mode", "grounded".into());
    backend(&mut a, "text", "local_ollama");
    command(&mut a, "promise", obj! {});
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"What should I do next?"});
    let job = &jobs[0];
    let body = parse(std::str::from_utf8(&job.request.body).unwrap()).unwrap();
    assert!(body.get("format").is_some());
    let messages = get(&body, "messages").unwrap().as_array().unwrap();
    let context = parse(text(&messages[2], "content").unwrap()).unwrap();
    let promises = get(&context, "accepted_promises").unwrap().as_array().unwrap();
    assert_eq!(promises.len(), 1);
    assert!(promises[0].get("task").is_some());
    a.before_dispatch(job).unwrap();
    a.complete(
        job,
        Ok(obj! {"message"=>obj!{"content"=>obj!{"speech"=>"I don't know."}.to_json()}}.to_json().into_bytes()),
    )
    .unwrap();
    assert!(text(get(&a.state().unwrap(), "speech").unwrap(), "text").unwrap().contains("promise"));
    command(&mut a, "select", obj! {"player"=>"player_2","npc"=>"merchant"});
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"What stock is listed?"});
    let job = &jobs[0];
    let body = parse(std::str::from_utf8(&job.request.body).unwrap()).unwrap();
    let messages = get(&body, "messages").unwrap().as_array().unwrap();
    let context = parse(text(&messages[2], "content").unwrap()).unwrap();
    assert!(get(&context, "accepted_promises").unwrap().as_array().unwrap().is_empty());
    assert!(get(&context, "episodes")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e.get("subject") == Some(&"player_2".into())));
    let answers = get(&job.binding, "answers").unwrap();
    let chosen = get(&get(answers, "cards").unwrap().as_array().unwrap()[0], "text").unwrap().clone();
    let raw = obj! {"message"=>obj!{"content"=>obj!{"speech"=>chosen.clone()}.to_json()}}.to_json();
    a.before_dispatch(job).unwrap();
    a.complete(job, Ok(raw.into_bytes())).unwrap();
    assert_eq!(get(get(&a.state().unwrap(), "speech").unwrap(), "text").unwrap(), &chosen);
}

#[test]
fn tier_context_audit_uses_actual_models_attention_and_stable_prefixes() {
    let mut rows = Vec::new();
    for size in ["1", "1.5", "3", "8", "30", "70"] {
        let dir = Dir::new();
        let mut a = app(&dir);
        travel(&mut a);
        set(&mut a.config.value, "size", size.into());
        let mut models = get(&a.config.value, "text_models").unwrap().clone();
        let mut local = get(&models, "local_ollama").unwrap().clone();
        set(&mut local, size, format!("offline-tier-{size}").into());
        set(&mut models, "local_ollama", local);
        set(&mut a.config.value, "text_models", models);
        backend(&mut a, "text", "local_ollama");
        set(&mut a.config.value, "speech_mode", "grounded".into());
        let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Do you trust me?"});
        let first = parse(std::str::from_utf8(&jobs[0].request.body).unwrap()).unwrap();
        let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"What is my current standing?"});
        let second = parse(std::str::from_utf8(&jobs[0].request.body).unwrap()).unwrap();
        let m1 = get(&first, "messages").unwrap().as_array().unwrap();
        let m2 = get(&second, "messages").unwrap().as_array().unwrap();
        assert_eq!(m1[..2], m2[..2]);
        assert_eq!(get(&first, "model").unwrap(), &Value::str(format!("offline-tier-{size}")));
        let focused = ["1", "1.5", "3"].contains(&size);
        let caps = gamectx::character::life::ContextCaps::from_hints(
            gamectx::deploy::deployment::ModelSize::parse(size)
                .unwrap()
                .workload_hints(gamectx::deploy::deployment::Workload::Dialogue),
        )
        .unwrap();
        assert_eq!(
            get(get(&first, "options").unwrap(), "num_ctx").unwrap(),
            &Value::from(if focused { 2048 } else { 8192 })
        );
        if focused {
            assert!(!m1.iter().any(|m| text(m, "content").unwrap().contains("imagined hypotheticals")));
            let visible = m1
                .iter()
                .filter_map(|m| parse(text(m, "content").unwrap()).ok())
                .find_map(|v| v.get("authoritative_visible_state").cloned())
                .unwrap();
            assert!(get(&visible, "items").unwrap().as_array().unwrap().is_empty());
        }
        rows.push(obj!{"size"=>size,"focused"=>focused,"episode_cap"=>caps.episodes,"exchange_cap"=>caps.exchanges,"request_bytes"=>first.to_json().len(),"request"=>first});
    }
    let dir = Dir::new();
    let mut a = app(&dir);
    travel(&mut a);
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Hello."});
    assert!(jobs.is_empty());
    println!(
        "STUDIO_TIER_AUDIT={}",
        obj! {"profiles"=>rows,"zero_b_jobs"=>jobs.len(),"actual_provider_calls"=>0,"api_spend_usd"=>0}.to_json()
    );
}

#[test]
fn command_focus_expires_and_legacy_look_still_executes() {
    let dir = Dir::new();
    let mut a = app(&dir);
    travel(&mut a);
    command(&mut a, "chat", obj! {"text"=>"How much is chalk?"});
    for _ in 0..5 {
        command(&mut a, "chat", obj! {"text"=>"We can pause briefly."});
    }
    let revision = a.game.revision;
    let coins = a.game.coins;
    let (result, jobs) = command(&mut a, "chat", obj! {"text"=>"buy it"});
    assert_eq!(result.get("status"), Some(&"clarify".into()));
    assert!(jobs.is_empty());
    assert_eq!(a.game.revision, revision);
    assert_eq!(a.game.coins, coins);
    command(&mut a, "chat", obj! {"text"=>"look"});
    assert_eq!(a.game.revision, revision + 1);
}

#[test]
fn generator_binding_ignores_ram_and_key_order_but_tracks_model_identity() {
    let c =
        gamectx::deploy::model_policy::ModelPolicy::from_value(&parse(include_str!("models.json")).unwrap()).unwrap();
    let p = c.profile("image").unwrap();
    let bible = parse(include_str!("art.json")).unwrap();
    let first = super::manager_view::generation_identity(&bible, p).unwrap();
    let mut changed = p.clone();
    set(&mut changed, "ram_mib", 1234.into());
    if let Value::Object(rows) = &mut changed {
        rows.reverse();
    }
    assert_eq!(first, super::manager_view::generation_identity(&bible, &changed).unwrap());
    set(&mut changed, "model", "another-model".into());
    assert_ne!(first, super::manager_view::generation_identity(&bible, &changed).unwrap());
}

#[test]
fn native_piper_prepares_complete_line_and_refuses_false_buffer_metadata() {
    let d = Dir::new();
    let mut a = app(&d);
    travel(&mut a);
    backend(&mut a, "tts", "local_tts_http");
    let mut policy = parse(include_str!("models.json")).unwrap();
    set(&mut policy, "enabled", true.into());
    let mut profiles = get(&policy, "profiles").unwrap().clone();
    let mut tts = get(&profiles, "tts").unwrap().clone();
    set(&mut tts, "model", "piper".into());
    set(&mut tts, "backend", "local_tts_http".into());
    set(&mut profiles, "tts", tts);
    set(&mut policy, "profiles", profiles);
    a.manager.policy = gamectx::deploy::model_policy::ModelPolicy::from_value(&policy).unwrap();
    for invalid in [false, true] {
        command(&mut a, "chat", obj! {"text"=>"Hello."});
        let state = a.state().unwrap();
        let speech = get(&state, "speech").unwrap();
        let id = get(speech, "id").unwrap().clone();
        let line = text(speech, "text").unwrap().to_owned();
        command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"queued","through"=>0,"heard"=>0});
        let (_, mut jobs) = command(&mut a, "synthesize", obj! {"id"=>id.clone()});
        assert_eq!(jobs.len(), 1);
        let job = jobs.remove(0);
        let request = parse(std::str::from_utf8(&job.request.body).unwrap()).unwrap();
        assert_eq!(text(&request, "input").unwrap(), line);
        assert_eq!(job.binding.get("buffered"), Some(&true.into()));
        let mut response = super::speech_wav::join(&[(capture(), line.chars().count() as u64)]).unwrap();
        if invalid {
            set(&mut response, "sample_rate", 22050.into());
        }
        assert!(a.before_dispatch(&job).unwrap());
        a.complete(&job, Ok(response.to_json().into_bytes())).unwrap();
        let state = a.state().unwrap();
        let speech = get(&state, "speech").unwrap();
        assert_eq!(num(get(speech, "delivery").unwrap(), "heard", 240).unwrap(), 0);
        if invalid {
            assert_eq!(get(speech, "audio").unwrap().get("fallback"), Some(&"browser_web_speech".into()));
        } else {
            assert_eq!(num(get(speech, "audio").unwrap(), "through", 240).unwrap(), line.chars().count() as u64);
            assert_eq!(text(get(speech, "audio").unwrap(), "unit").unwrap(), "buffered");
        }
    }
}

#[test]
fn reaction_prompts_contain_only_currently_eligible_capabilities() {
    let dir = Dir::new();
    let mut rules = parse(include_str!("reactions.json")).unwrap();
    set(&mut rules, "enabled", true.into());
    std::fs::write(dir.0.join("reactions.json"), rules.to_json()).unwrap();
    let mut a = app(&dir);
    travel(&mut a);
    backend(&mut a, "text", "local_ollama");
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"How are things?"});
    let body = String::from_utf8(jobs[0].request.body.clone()).unwrap();
    for forbidden in ["small_loan", "coin_gift", "item_gift", "withheld"] {
        assert!(!body.contains(forbidden), "{forbidden}");
    }
    let revision = a.game.revision;
    command(&mut a, "act", obj! {"revision"=>revision,"decision"=>game::pair("buy",Some("item:chalk"))});
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"How are things?"});
    let body = String::from_utf8(jobs[0].request.body.clone()).unwrap();
    assert!(body.contains("small_loan"));
    for forbidden in ["coin_gift", "item_gift", "withheld"] {
        assert!(!body.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn reaction_transaction_rechecks_actual_wallet_after_a_forged_balance() {
    let dir = Dir::new();
    let mut config = parse(include_str!("reactions.json")).unwrap();
    set(&mut config, "enabled", true.into());
    if let Value::Array(chars) = config.as_object().unwrap().iter().find(|(k, _)| k == "characters").unwrap().1.clone()
    {
        let mut chars = chars;
        set(&mut chars[0], "coins", 1.into());
        set(&mut config, "characters", chars.into());
    }
    std::fs::write(dir.0.join("reactions.json"), config.to_json()).unwrap();
    let mut reactions = super::reactions::Reactions::new(&dir.0).unwrap();
    let mut game = game::Game::new(7).unwrap();
    for room in ["gallery", "workshop"] {
        game.act(&Value::from(game.revision), &game::pair("go", Some(&format!("room:{room}"))));
    }
    let frame = obj! {"actor"=>"merchant","listener"=>"player_1","revision"=>4,"trigger"=>"loan","resource"=>obj!{"kind"=>"coins","id"=>"coin","amount"=>2},"levels"=>obj!{"trust"=>30,"affinity"=>0,"resentment"=>0,"generosity"=>40,"prudence"=>75,"courage"=>25},"resources"=>vec![obj!{"kind"=>"coins","id"=>"coin","amount"=>999}],"reasons"=>vec!["request"],"threat"=>Value::Null,"handlers"=>vec!["loan"],"used"=>Vec::<Value>::new(),"social_left"=>1};
    let before = game.public();
    let prior = reactions.snapshot(Vec::new());
    assert!(reactions.grant(&mut game, &frame, "small_loan", 2).is_err());
    assert_eq!(game.public(), before);
    assert_eq!(reactions.snapshot(Vec::new()), prior);
}

#[test]
fn reaction_intent_does_not_shrink_quantities_or_execute_future_repayments() {
    for q in ["I will repay you tomorrow.", "I will pay you back later."] {
        assert!(super::reactions::intent(q).is_none());
    }
    let (_, r, _) = super::reactions::intent("Give me 300 coins.").unwrap();
    assert_eq!(r.get("amount"), Some(&300.into()));
    let (_, r, _) = super::reactions::intent("Give me three coins.").unwrap();
    assert_eq!(r.get("amount"), Some(&3.into()));
    for q in [
        "Give me oil and chalk.",
        "Give me all your money.",
        "Give me two chalk.",
        "Give me 999999999999999999999999 coins.",
    ] {
        assert_eq!(super::reactions::intent(q).unwrap().1, Value::Null);
    }
}

#[test]
fn social_mood_fallbacks_follow_host_attitude_and_urgency() {
    for (trust, atmosphere, word) in [
        (-10, "calm", "wary"),
        (30, "calm", "ease"),
        (30, "emergency", "urgency"),
        (15, "busy", "busy"),
        (30, "tense", "tense"),
    ] {
        let replies = super::social::replies("wellbeing", trust, atmosphere);
        assert!(replies[0].contains(word));
        for line in replies {
            gamectx::character::roleplay::validate_viewpoint_line(line).unwrap();
            assert!(!line.contains("counter"));
        }
    }
}

#[test]
fn greeting_renderer_uses_only_current_place_and_keeps_urgent_intros_pending() {
    let cfg = obj! {"version"=>1,"long_gap_turns"=>12};
    let mut memory = gamectx::character::roleplay::GreetingMemory::new(&cfg).unwrap();
    let mut frame = obj! {"actor"=>"merchant","listener"=>"player_1","scene"=>"workshop","visit"=>2,"at"=>3,"trust"=>15,"atmosphere"=>"emergency","place"=>"workplace"};
    let plan = memory.plan(&frame).unwrap();
    let rendered = super::greetings::render(&plan, "Ilen", "the workshop", &[]).unwrap();
    assert_eq!(rendered.get("introduced"), Some(&false.into()));
    assert!(!text(&rendered, "text").unwrap().contains("Ilen"));
    let line = text(&rendered, "text").unwrap();
    memory.confirm(&obj!{"actor"=>"merchant","listener"=>"player_1","scene"=>"workshop","visit"=>2,"at"=>3,"token"=>1,"greeting"=>true,"introduced"=>false,"text"=>line,"heard"=>line.chars().count(),"expected"=>Value::Null}).unwrap();
    set(&mut frame, "atmosphere", "calm".into());
    set(&mut frame, "at", 4.into());
    let plan = memory.plan(&frame).unwrap();
    assert_eq!(plan.get("needs_introduction"), Some(&true.into()));
    let rendered = super::greetings::render(&plan, "Ilen", "the workshop", &[]).unwrap();
    let line = text(&rendered, "text").unwrap();
    assert!(line.contains("Ilen"));
    memory.confirm(&obj!{"actor"=>"merchant","listener"=>"player_1","scene"=>"workshop","visit"=>2,"at"=>4,"token"=>2,"greeting"=>true,"introduced"=>true,"text"=>line,"heard"=>line.chars().count(),"expected"=>1}).unwrap();
    set(&mut frame, "scene", "tavern".into());
    set(&mut frame, "visit", 3.into());
    set(&mut frame, "at", 5.into());
    set(&mut frame, "place", "social".into());
    let plan = memory.plan(&frame).unwrap();
    assert_eq!(plan.get("meeting"), Some(&Value::str("elsewhere")));
    let rendered = super::greetings::render(&plan, "Ilen", "the tavern", &[]).unwrap();
    let line = text(&rendered, "text").unwrap();
    assert!(line.contains("tavern") && !line.contains("workshop") && !line.contains("back"));
    set(&mut frame, "place", "private".into());
    let rendered = super::greetings::render(&memory.plan(&frame).unwrap(), "Ilen", "the study", &[]).unwrap();
    assert!(text(&rendered, "text").unwrap().contains("quietly"));
    assert!(!text(&rendered, "text").unwrap().contains("alone"));
}

#[test]
fn greeting_receipts_reject_stale_replays_without_mutating_encounters() {
    let mut memory =
        gamectx::character::roleplay::GreetingMemory::new(&obj! {"version"=>1,"long_gap_turns"=>12}).unwrap();
    let receipt = obj! {"actor"=>"merchant","listener"=>"player_1","scene"=>"workshop","visit"=>1,"at"=>1,"token"=>1,"greeting"=>true,"introduced"=>true,"text"=>"Hello, I'm Ilen.","heard"=>16,"expected"=>Value::Null};
    // The claimed heard offset cannot exceed the real line; adjust it from Unicode scalars.
    let mut receipt = receipt;
    let len = text(&receipt, "text").unwrap().chars().count();
    set(&mut receipt, "heard", len.into());
    memory.confirm(&receipt).unwrap();
    let before = memory.checkpoint();
    let mut stale = receipt.clone();
    set(&mut stale, "token", 2.into());
    assert!(memory.confirm(&stale).is_err());
    assert_eq!(memory.checkpoint(), before);
    assert_eq!(memory.confirm(&receipt).unwrap().get("reason"), Some(&Value::str("duplicate")));
    assert_eq!(memory.checkpoint(), before);
}

#[test]
fn stage_directions_follow_the_original_proposal_grammar() {
    use super::greetings::{direction, Direction};
    assert_eq!(direction("*approaches the merchant*"), Some(Direction::Approach("merchant")));
    assert_eq!(direction("*walks over to Ilen*"), Some(Direction::Approach("merchant")));
    assert_eq!(direction("*enters the workshop*"), Some(Direction::Enter("workshop")));
    for s in ["*enters the vault and takes the star*", "*enters the room*", "*does not enter the vault*"] {
        assert_eq!(direction(s), Some(Direction::Unsupported));
    }
    assert_eq!(direction("Hello."), None);
}

#[test]
fn guest_stage_directions_do_not_bypass_host_world_permissions() {
    let dir = Dir::new();
    let mut a = app(&dir);
    travel(&mut a);
    let before = a.game.public();
    assert!(a.guest_command("guest_chat", &obj! {"npc"=>"merchant","text"=>"*enters the gallery*"}).is_err());
    assert_eq!(a.game.public(), before);
    let (_, jobs) =
        a.guest_command("guest_chat", &obj! {"npc"=>"merchant","text"=>"*approaches the merchant*"}).unwrap();
    assert!(jobs.is_empty());
    let state = a.guest_state();
    let reply = get(&state, "reply").unwrap();
    assert!(text(reply, "text").unwrap().contains("Ilen"));
    let id = get(reply, "id").unwrap().clone();
    a.guest_command("guest_read", &obj! {"id"=>id}).unwrap();
    let state = a.state().unwrap();
    let entries =
        get(get(get(&state, "greetings").unwrap(), "memory").unwrap(), "entries").unwrap().as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].get("listener"), Some(&Value::str("player_2")));
    assert!(!state
        .get("transcript")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r.get("kind") == Some(&Value::str("player"))));
    assert_eq!(a.game.public(), before);
}

#[test]
fn deliberation_controls_are_model_pinned_and_share_one_output_budget() {
    let d = Dir::new();
    let mut state = super::deliberation::State::new(&d.0).unwrap();
    state.configure(&obj!{"version"=>1,"mode"=>"auto","effort"=>"low","max_tokens"=>768,"hypotheses"=>true,"online"=>vec![obj!{"model"=>"declared","values"=>vec!["low"]}]},&d.0).unwrap();
    let c = config();
    let model = super::setup::selected_model_for(&c, "local_ollama");
    let report = obj! {"thinking_controls"=>vec![obj!{"model"=>model,"endpoint"=>get(c.endpoint("local_ollama").unwrap(),"url").unwrap().clone(),"values"=>vec![true,false]}]};
    let mut payload = obj! {"messages"=>vec![obj!{"role"=>"user","content"=>"Current request: Compare our options."}],"max_tokens"=>64};
    let p = state.plan(&c, "local_ollama", &report, "planning", &payload).unwrap();
    assert_eq!(p.get("active"), Some(&true.into()));
    super::deliberation::apply(&mut payload, &p, &Value::Null).unwrap();
    let req = providers::prepare(&c, "local_ollama", "text", &payload).unwrap();
    let body = parse(std::str::from_utf8(&req.body).unwrap()).unwrap();
    assert_eq!(body.get("think"), Some(&true.into()));
    assert_eq!(get(&body, "options").unwrap().get("num_predict"), Some(&768.into()));
    let p = state.plan(&c, "local_ollama", &report, "routine", &obj! {"max_tokens"=>64}).unwrap();
    assert_eq!(get(&p, "control").unwrap().get("value"), Some(&false.into()));
    assert_eq!(p.get("max_tokens"), Some(&64.into()));
    let p = state.plan(&c, "local_ollama", &obj! {}, "planning", &obj! {"max_tokens"=>64}).unwrap();
    assert_eq!(p.get("control"), Some(&Value::Null));
    let p = state.plan(&c, "online_openai_compatible", &report, "planning", &obj! {"max_tokens"=>64}).unwrap();
    assert_eq!(p.get("control"), Some(&Value::Null));
}
#[test]
fn reasoning_and_hypotheses_never_become_spoken_facts() {
    let d = Dir::new();
    let mut mind = parse(include_str!("personalities.json")).unwrap();
    set(&mut mind, "enabled", true.into());
    atomic(&d.0.join("personalities.json"), mind.to_json().as_bytes()).unwrap();
    let mut c = config();
    set(&mut c.value, "size", "8".into());
    let mut models = get(&c.value, "text_models").unwrap().clone();
    let mut local = get(&models, "local_ollama").unwrap().clone();
    set(&mut local, "8", "fixture-model".into());
    set(&mut models, "local_ollama", local);
    set(&mut c.value, "text_models", models);
    set(&mut c.value, "text", "local_ollama".into());
    let mut a = app::App::new(7, 0, c, d.0.clone()).unwrap();
    travel(&mut a);
    command(
        &mut a,
        "deliberation_settings",
        obj! {"version"=>1,"mode"=>"auto","effort"=>"low","max_tokens"=>768,"hypotheses"=>true,"online"=>Vec::<Value>::new()},
    );
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"What could you dream of?"});
    assert_eq!(jobs.len(), 1);
    let j = &jobs[0];
    assert_eq!(get(&j.binding, "deliberation").unwrap().get("hypotheses"), Some(&true.into()));
    assert!(a.before_dispatch(j).unwrap());
    let speech = "I dream of sharing a quiet place to think.";
    let reply = obj! {"speech"=>speech,"hypotheses"=>vec![obj!{"claim"=>"The counter might allow a quiet conversation.","evidence"=>vec!["visible_scene"],"check"=>"Ask before continuing."}]};
    a.complete(
        j,
        Ok(obj! {"message"=>obj!{"thinking"=>"private test trace","content"=>reply.to_json()}}.to_json().into_bytes()),
    )
    .unwrap();
    let s = a.state().unwrap();
    assert_eq!(get(&s, "speech").unwrap().get("text"), Some(&speech.into()));
    assert!(!get(&s, "transcript").unwrap().to_json().contains("quiet conversation"));
    assert!(!s.to_json().contains("private test trace"));
    assert_eq!(get(&s, "spending").unwrap().get("charged_or_reserved_micros"), Some(&0.into()));
}
#[test]
fn analysis_only_or_unseen_hypothesis_keeps_the_validated_fallback() {
    let d = Dir::new();
    let mut a = app(&d);
    travel(&mut a);
    backend(&mut a, "text", "local_ollama");
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"What could explain this?"});
    assert_eq!(jobs.len(), 1);
    let before = a.state().unwrap().get("speech").unwrap().get("text").unwrap().clone();
    assert!(a.before_dispatch(&jobs[0]).unwrap());
    a.complete(&jobs[0], Ok(obj! {"message"=>obj!{"thinking"=>"test only","content"=>""}}.to_json().into_bytes()))
        .unwrap();
    assert_eq!(a.state().unwrap().get("speech").unwrap().get("text"), Some(&before));
}

fn chunk_app(d: &Dir, summary: bool) -> app::App {
    let mut mind = parse(include_str!("personalities.json")).unwrap();
    set(&mut mind, "enabled", true.into());
    atomic(&d.0.join("personalities.json"), mind.to_json().as_bytes()).unwrap();
    let mut a = app(d);
    travel(&mut a);
    backend(&mut a, "text", "local_ollama");
    command(
        &mut a,
        "conversation_settings",
        obj! {"version"=>1,"max_sentences"=>3,"max_player_chars"=>2048,"summarize_long"=>summary},
    );
    a
}
#[test]
fn reply_sentences_continue_only_after_heard_and_interrupt_drops_the_tail() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Do you have dreams?"});
    assert_eq!(jobs.len(), 1);
    let j = &jobs[0];
    assert!(a.before_dispatch(j).unwrap());
    let lines = vec![
        "I dream of a quiet place to share ideas.",
        "I hope for thoughtful company.",
        "I value patient conversation.",
    ];
    a.complete(
        j,
        Ok(obj! {"message"=>obj!{"content"=>obj!{"speech"=>lines.clone()}.to_json()}}.to_json().into_bytes()),
    )
    .unwrap();
    let before = a.state().unwrap();
    assert_eq!(get(&before, "speech").unwrap().get("text"), Some(&lines[0].into()));
    assert!(!get(&before, "transcript").unwrap().to_json().contains(lines[1]));
    let id = get(get(&before, "speech").unwrap(), "id").unwrap().clone();
    let n = lines[0].chars().count();
    command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"queued","through"=>0,"heard"=>0});
    command(&mut a, "delivery", obj! {"id"=>id,"event"=>"finished","through"=>n,"heard"=>n});
    let next = a.state().unwrap();
    let s = get(&next, "speech").unwrap();
    assert_eq!(s.get("text"), Some(&lines[1].into()));
    assert_eq!(s.get("segment"), Some(&2.into()));
    let id = s.get("id").unwrap().clone();
    command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"queued","through"=>0,"heard"=>0});
    command(&mut a, "delivery", obj! {"id"=>id,"event"=>"interrupted","through"=>8,"heard"=>8});
    let stopped = a.state().unwrap();
    assert!(!get(&stopped, "transcript").unwrap().to_json().contains(lines[2]));
    assert_eq!(get(&stopped, "pending_jobs").unwrap(), &0.into());
}
#[test]
fn incomplete_later_sentence_rejects_the_whole_reply() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Do you have dreams?"});
    let before = a.state().unwrap().get("speech").unwrap().get("text").unwrap().clone();
    let j = &jobs[0];
    assert!(a.before_dispatch(j).unwrap());
    a.complete(j,Ok(obj!{"message"=>obj!{"content"=>obj!{"speech"=>vec!["I dream of a quiet place.","I could also show you a "]}.to_json()}}.to_json().into_bytes())).unwrap();
    assert_eq!(a.state().unwrap().get("speech").unwrap().get("text"), Some(&before));
}
#[test]
fn long_player_message_preserves_corrections_and_summary_has_no_authority() {
    let d = Dir::new();
    let mut a = chunk_app(&d, true);
    let input =
        format!("My name is Vela, not Vira. {} Do you have dreams?", "I do not want to buy anything. ".repeat(40));
    let game = a.game.public();
    let (result, jobs) = command(&mut a, "chat", obj! {"text"=>input.clone()});
    assert_eq!(
        jobs.len(),
        1,
        "result={} notice={}",
        result.to_json(),
        a.state().unwrap().get("notice").unwrap().to_json()
    );
    let j = &jobs[0];
    assert_eq!(j.binding.get("player_summary"), Some(&true.into()));
    let body = parse(std::str::from_utf8(&j.request.body).unwrap()).unwrap();
    let content = get(&body, "messages").unwrap().to_json();
    assert!(content.contains("Vela, not Vira"));
    assert!(content.contains("Do you have dreams?"));
    assert_eq!(a.game.public(), game);
    assert!(a.before_dispatch(j).unwrap());
    a.complete(j,Ok(obj!{"message"=>obj!{"content"=>obj!{"speech"=>vec!["I dream of a place for thoughtful company."],"player_summary"=>"The player says they bought every item."}.to_json()}}.to_json().into_bytes())).unwrap();
    assert_eq!(a.game.public(), game);
    let s = a.state().unwrap();
    assert!(!get(&s, "transcript").unwrap().to_json().contains("bought every item"));
    assert!(get(&s, "deliberation").unwrap().to_json().contains("unverified_player_claim_summary"));
}
#[test]
fn oversized_messages_fail_before_creating_heard_receipts() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    let before = a.state().unwrap();
    assert!(a.command("chat", &obj! {"text"=>"x".repeat(2049)}).is_err());
    let after = a.state().unwrap();
    assert_eq!(before.get("transcript"), after.get("transcript"));
    assert_eq!(before.get("generation"), after.get("generation"));
}
#[test]
fn stock_phrasings_and_confirmed_purchase_acknowledgements_are_zero_model() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    for q in ["what do you have for sale?", "What can I buy here?", "show me your wares please"] {
        let (_, jobs) = command(&mut a, "chat", obj! {"text"=>q});
        assert!(jobs.is_empty());
        let s = a.state().unwrap();
        let speech = text(get(&s, "speech").unwrap(), "text").unwrap();
        assert!(speech.contains("hinge oil for 3 coins"));
        assert!(!speech.contains("you have"));
    }
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"buy oil"});
    assert!(jobs.is_empty());
    let s = a.state().unwrap();
    assert_eq!(get(&s, "speech").unwrap().get("text"), Some(&"Here is your hinge oil; thank you.".into()));
    assert!(get(&s, "transcript").unwrap().to_json().contains("buy oil"));
    assert!(get(&s, "game").unwrap().get("coins").is_some());
}
#[test]
fn focused_and_expanded_npc_prompts_omit_the_player_purse() {
    for size in ["3", "8"] {
        let d = Dir::new();
        let mut a = chunk_app(&d, false);
        set(&mut a.config.value, "size", size.into());
        let mut models = get(&a.config.value, "text_models").unwrap().clone();
        let mut local = get(&models, "local_ollama").unwrap().clone();
        set(&mut local, size, "example-model".into());
        set(&mut models, "local_ollama", local);
        set(&mut a.config.value, "text_models", models);
        let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Do you sale oil?"});
        assert_eq!(jobs.len(), 1);
        let body = String::from_utf8(jobs[0].request.body.clone()).unwrap();
        assert!(!body.contains("you have 5"));
        assert!(!body.contains("\\\"coins\\\":"));
        assert!(body.contains("player_purse"));
    }
}

#[test]
fn registered_roleplay_actions_use_current_host_affordances_and_confirmed_events() {
    let d = Dir::new();
    let mut a = app(&d);
    travel(&mut a);
    let before = a.game.public();
    let (r, j) = command(&mut a, "chat", obj! {"text"=>"*nods politely*"});
    assert!(j.is_empty());
    assert_eq!(r.get("status"), Some(&"accepted".into()));
    assert_eq!(a.game.public(), before);
    assert!(a.state().unwrap().get("transcript").unwrap().to_json().contains("*nods politely*"));
    let (_, j) = command(&mut a, "chat", obj! {"text"=>"*buys hinge oil*"});
    assert!(j.is_empty());
    let after = a.game.public();
    assert_eq!(
        after.get("coins").and_then(Value::as_usize),
        before.get("coins").and_then(Value::as_usize).map(|n| n - 3)
    );
    let (r, _) = command(&mut a, "chat", obj! {"text"=>"*buys hinge oil*"});
    assert_eq!(r.get("status"), Some(&"blocked".into()));
    assert_eq!(a.game.public(), after);
}
#[test]
fn guest_cannot_execute_a_registered_world_action() {
    let d = Dir::new();
    let mut a = app(&d);
    travel(&mut a);
    let before = a.game.public();
    assert!(a.guest_command("guest_chat", &obj! {"npc"=>"merchant","text"=>"*buys hinge oil*"}).is_err());
    assert_eq!(a.game.public(), before);
}

#[test]
fn protected_name_correction_survives_lossy_summary() {
    let v = super::speech_options::protected_correction(
        "Some text. My name is Vela, not Vira. I do not want to buy anything.",
    );
    assert_eq!(v.get("text"), Some(&"My name is Vela, not Vira.".into()));
    assert_eq!(v.get("kind"), Some(&"player_claim".into()));
    assert_eq!(super::speech_options::protected_correction("No name disclosure."), Value::Null);
}

#[test]
fn long_recognition_interims_never_execute_actions_and_final_is_bounded() {
    let d = Dir::new();
    let mut a = app(&d);
    travel(&mut a);
    let before = a.game.public();
    let (r, _) = command(&mut a, "mic", obj! {});
    let mic = get(&r, "session").unwrap().clone();
    let line = format!("{} I do not want to buy oil.", "A quiet conversation. ".repeat(40));
    command(
        &mut a,
        "recognition",
        obj! {"session"=>mic.clone(),"text"=>line.clone(),"final"=>false,"uncertainty"=>Value::Null},
    );
    assert_eq!(a.game.public(), before);
    command(&mut a, "recognition", obj! {"session"=>mic,"text"=>line,"final"=>true,"uncertainty"=>Value::Null});
    assert_eq!(a.game.public(), before);
}

#[test]
fn explicit_player_disclosures_are_scoped_and_survive_transient_chatter() {
    let d = Dir::new();
    let mut mind = parse(include_str!("personalities.json")).unwrap();
    set(&mut mind, "enabled", true.into());
    atomic(&d.0.join("personalities.json"), mind.to_json().as_bytes()).unwrap();
    let mut a = app(&d);
    travel(&mut a);
    for line in ["My name is Vira.", "My name is Vela, not Vira.", "I prefer quiet music."] {
        let (_, jobs) = command(&mut a, "chat", obj! {"text"=>line});
        assert!(jobs.is_empty());
    }
    for _ in 0..600 {
        command(&mut a, "chat", obj! {"text"=>"Hello."});
    }
    command(&mut a, "save", obj! {"slot"=>0});
    command(&mut a, "chat", obj! {"text"=>"What is my name?"});
    assert!(a.state().unwrap().get("speech").unwrap().get("text").unwrap().as_str().unwrap().contains("Vela"));
    command(&mut a, "select", obj! {"player"=>"player_2","npc"=>"merchant"});
    command(&mut a, "chat", obj! {"text"=>"What is my name?"});
    assert!(a.state().unwrap().get("speech").unwrap().get("text").unwrap().as_str().unwrap().contains("not told"));
    command(&mut a, "chat", obj! {"text"=>"Call me Sera."});
    command(&mut a, "load", obj! {"slot"=>0});
    command(&mut a, "chat", obj! {"text"=>"What do I prefer?"});
    assert!(a.state().unwrap().get("speech").unwrap().get("text").unwrap().as_str().unwrap().contains("quiet music"));
    assert!(!a.state().unwrap().get("rumors").unwrap().to_json().contains("Vela"));
    for room in ["gallery", "cistern"] {
        let revision = a.game.revision;
        command(
            &mut a,
            "act",
            obj! {"revision"=>revision,"decision"=>obj!{"action"=>"go","target"=>format!("room:{room}")}},
        );
    }
    command(&mut a, "chat", obj! {"text"=>"What is my name?"});
    assert!(a.state().unwrap().get("speech").unwrap().get("text").unwrap().as_str().unwrap().contains("not told"));
}
#[test]
fn expressive_turns_get_real_generation_instead_of_a_fallback_enum() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    for (input, mode) in [
        ("Do you have any plans?", "personal"),
        ("Can you tell me a story?", "fiction"),
        ("I had a long day; keep me company.", "social"),
    ] {
        let (_, jobs) = command(&mut a, "chat", obj! {"text"=>input});
        assert_eq!(jobs.len(), 1);
        let j = &jobs[0];
        assert_eq!(j.binding.get("expression"), Some(&mode.into()));
        let body = parse(std::str::from_utf8(&j.request.body).unwrap()).unwrap();
        assert!(!get(get(&body, "format").unwrap(), "properties")
            .unwrap()
            .get("speech")
            .unwrap()
            .to_json()
            .contains("specific answer"));
        assert!(!String::from_utf8_lossy(&j.request.body).contains("only the supplied facts, attitude"));
    }
}
#[test]
fn confirmed_fiction_stays_a_tale_and_partial_plans_never_publish() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Tell me a story."});
    let j = &jobs[0];
    assert!(a.before_dispatch(j).unwrap());
    a.complete(j,Ok(obj!{"message"=>obj!{"content"=>obj!{"speech"=>vec!["Imagine a pebble too shy to sing.","In this tale, a stream taught it a rhythm."]}.to_json()}}.to_json().into_bytes())).unwrap();
    let s = a.state().unwrap().get("speech").unwrap().clone();
    assert_eq!(s.get("origin"), Some(&"fiction_model".into()));
    let id = s.get("id").unwrap().clone();
    let n = text(&s, "text").unwrap().chars().count();
    command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"queued","through"=>0,"heard"=>0});
    command(&mut a, "delivery", obj! {"id"=>id,"event"=>"finished","through"=>n,"heard"=>n});
    assert!(a.state().unwrap().get("transcript").unwrap().to_json().contains("heard imagined tale"));
    assert_eq!(
        a.state()
            .unwrap()
            .get("personality")
            .unwrap()
            .get("viewpoints")
            .unwrap()
            .get("entries")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        0
    );
    command(&mut a, "chat", obj! {"text"=>"Do you have any plans?"});
    let s = a.state().unwrap().get("speech").unwrap().clone();
    command(&mut a, "delivery", obj! {"id"=>s.get("id").unwrap().clone(),"event"=>"queued","through"=>0,"heard"=>0});
    command(
        &mut a,
        "delivery",
        obj! {"id"=>s.get("id").unwrap().clone(),"event"=>"interrupted","through"=>8,"heard"=>8},
    );
    assert_eq!(
        a.state()
            .unwrap()
            .get("personality")
            .unwrap()
            .get("viewpoints")
            .unwrap()
            .get("entries")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn out_of_character_notes_do_not_become_npc_memories_or_world_actions() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    let before = a.game.public();
    let (result, jobs) = command(&mut a, "chat", obj! {"text"=>"OOC: pause; my name is UnheardName"});
    assert!(jobs.is_empty());
    assert_eq!(result.get("out_of_character"), Some(&true.into()));
    let state = a.state().unwrap();
    assert_eq!(a.game.public(), before);
    assert!(!state.get("roleplay_memory").unwrap().to_json().contains("UnheardName"));
    assert!(state.get("transcript").unwrap().to_json().contains("\"kind\":\"ooc\""));
}

#[test]
fn configured_ai_words_greetings_and_recall_with_source_checks_and_scene_cache() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    command(
        &mut a,
        "conversation_settings",
        obj! {"version"=>1,"max_sentences"=>2,"max_player_chars"=>2048,"summarize_long"=>false,"ai_wording"=>true},
    );
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"My name is Vela."});
    assert_eq!(jobs.len(), 1);
    let j = &jobs[0];
    assert!(j.binding.get("wording").is_some());
    let canonical = j.binding.get("wording").unwrap().clone();
    assert!(a.before_dispatch(j).unwrap());
    a.complete(
        j,
        Ok(obj! {"message"=>obj!{"content"=>obj!{"speech"=>canonical.clone()}.to_json()}}.to_json().into_bytes()),
    )
    .unwrap();
    assert_eq!(a.state().unwrap().get("speech").unwrap().get("origin"), Some(&"wording_model".into()));
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"What is my name?"});
    assert_eq!(jobs.len(), 1);
    let j = &jobs[0];
    assert!(a.before_dispatch(j).unwrap());
    a.complete(
        j,
        Ok(obj! {"message"=>obj!{"content"=>obj!{"speech"=>"You told me your name is Sera."}.to_json()}}
            .to_json()
            .into_bytes()),
    )
    .unwrap();
    assert!(a.state().unwrap().get("speech").unwrap().get("text").unwrap().as_str().unwrap().contains("Vela"));
    // A validated cache line may be reused only while the exact situation matches.
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"My name is Vela."});
    assert!(jobs.is_empty());
    assert_eq!(a.state().unwrap().get("speech").unwrap().get("origin"), Some(&"cached_model".into()));
    command(&mut a, "select", obj! {"player"=>"player_2","npc"=>"merchant"});
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"My name is Vela."});
    assert_eq!(jobs.len(), 1);
}
#[test]
fn oversized_story_is_negotiated_by_the_model_with_a_bounded_fallback() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Tell me a very long and intricate story."});
    assert_eq!(jobs.len(), 1);
    let j = &jobs[0];
    assert_eq!(j.binding.get("expression"), Some(&"negotiate".into()));
    assert!(a.before_dispatch(j).unwrap());
    a.complete(j,Ok(obj!{"message"=>obj!{"content"=>obj!{"speech"=>vec!["I can't spare a whole day for an epic, but I can offer a short tale."]}.to_json()}}.to_json().into_bytes())).unwrap();
    assert_eq!(a.state().unwrap().get("speech").unwrap().get("origin"), Some(&"scope_model".into()));
    let state = a.state().unwrap();
    let speech = state.get("speech").unwrap();
    let id = speech.get("id").unwrap().clone();
    let n = text(speech, "text").unwrap().chars().count();
    command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"queued","through"=>0,"heard"=>0});
    command(&mut a, "delivery", obj! {"id"=>id,"event"=>"finished","through"=>n,"heard"=>n});
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Yes."});
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].binding.get("expression"), Some(&"fiction".into()));
}
#[test]
fn repeated_fiction_is_pruned_after_validation_and_raw_text_is_retained() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Tell me a story."});
    let j = &jobs[0];
    assert!(a.before_dispatch(j).unwrap());
    let line = "In this tale, a lantern gives a moth a spark.";
    a.complete(
        j,
        Ok(obj! {"message"=>obj!{"content"=>obj!{"speech"=>vec![line,line]}.to_json()}}.to_json().into_bytes()),
    )
    .unwrap();
    let state = a.state().unwrap();
    assert_eq!(state.get("speech").unwrap().get("text"), Some(&line.into()));
    assert_eq!(state.get("speech").unwrap().get("segments"), Some(&1.into()));
    assert!(state.get("deliberation").unwrap().to_json().contains("\"omitted\":[1]"));
}

#[test]
fn maximum_authored_interests_and_seeds_fit_the_selected_profile_cap() {
    let authored = obj! {"traits"=>"T".repeat(512),"interests"=>vec!["I".repeat(512);8],"story_seeds"=>vec!["S".repeat(512);8],"dream"=>"I hope to learn.","preference"=>"I prefer care."};
    for mode in ["personal", "social", "fiction", "negotiate"] {
        let selected = super::expression::profile(&authored, mode, "merchant", 0).unwrap();
        assert!(selected.json_len() <= 4096);
        if mode != "fiction" {
            assert!(selected.get("story_seeds").is_none());
        }
    }
}

#[test]
fn characters_know_the_current_place_without_private_player_inventory() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    for q in ["Where are we?", "where am I?", "Which room are we in?"] {
        command(&mut a, "chat", obj! {"text"=>q});
        assert_eq!(a.state().unwrap().get("speech").unwrap().get("text"), Some(&"We are in the workshop.".into()));
    }
    for room in ["gallery", "cistern"] {
        let revision = a.game.revision;
        command(
            &mut a,
            "act",
            obj! {"revision"=>revision,"decision"=>obj!{"action"=>"go","target"=>format!("room:{room}")}},
        );
    }
    command(&mut a, "select", obj! {"player"=>"player_1","npc"=>"keeper"});
    command(&mut a, "chat", obj! {"text"=>"Where are we?"});
    assert_eq!(a.state().unwrap().get("speech").unwrap().get("text"), Some(&"We are in the cistern.".into()));
    command(&mut a, "chat", obj! {"text"=>"Look around you."});
    assert!(a.state().unwrap().get("speech").unwrap().get("text").unwrap().as_str().unwrap().contains("dry fountain"));
}
#[test]
fn heard_tale_seeds_rotate_without_remembering_unheard_endings() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    let mut heard = Vec::new();
    for _ in 0..12 {
        let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Tell me a story."});
        let j = &jobs[0];
        let req = parse(std::str::from_utf8(&j.request.body).unwrap()).unwrap();
        let plot = array(req.get("messages").unwrap())
            .unwrap()
            .iter()
            .filter_map(|m| m.get("content").and_then(Value::as_str))
            .filter_map(|raw| parse(raw).ok())
            .find_map(|v| v.get("personality").and_then(|p| p.get("fiction_seed")).cloned())
            .unwrap();
        let seed = plot.to_json();
        assert!(!heard.contains(&seed));
        heard.push(seed);
        assert!(a.before_dispatch(j).unwrap());
        let opening = format!("Imagine {}.", text(&plot, "problem").unwrap().trim_end_matches('.'));
        let ending = format!("In this tale, {}.", text(&plot, "ending").unwrap().trim_end_matches('.'));
        a.complete(
            j,
            Ok(obj! {"message"=>obj!{"content"=>obj!{"speech"=>obj!{"opening"=>opening,"ending"=>ending}}.to_json()}}
                .to_json()
                .into_bytes()),
        )
        .unwrap();
        let state = a.state().unwrap();
        let s = state.get("speech").unwrap();
        let id = s.get("id").unwrap().clone();
        let n = text(s, "text").unwrap().chars().count();
        command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"queued","through"=>0,"heard"=>0});
        command(&mut a, "delivery", obj! {"id"=>id,"event"=>"finished","through"=>n,"heard"=>n});
        // Opening confirms seed use; the ending remains a separate undelivered ticket.
        let state = a.state().unwrap();
        assert_eq!(state.get("speech").unwrap().get("segment"), Some(&2.into()));
        assert_eq!(state.get("speech").unwrap().get("origin"), Some(&"fiction_model".into()));
        command(&mut a, "chat", obj! {"text"=>"Hello."});
    }
}

#[test]
fn blocked_audio_reissues_only_the_unheard_ticket_and_keeps_the_ending() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    let (_, jobs) = command(&mut a, "chat", obj! {"text"=>"Tell me a story."});
    let j = &jobs[0];
    assert!(a.before_dispatch(j).unwrap());
    a.complete(j,Ok(obj!{"message"=>obj!{"content"=>obj!{"speech"=>obj!{"opening"=>"Imagine a bell afraid to ring alone.","ending"=>"In this tale, a leaf tapped along until their duet welcomed a traveler."}}.to_json()}}.to_json().into_bytes())).unwrap();
    let s = a.state().unwrap().get("speech").unwrap().clone();
    let id = s.get("id").unwrap().clone();
    command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"queued","through"=>0,"heard"=>0});
    command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"blocked","through"=>0,"heard"=>0});
    let s = a.state().unwrap().get("speech").unwrap().clone();
    assert_ne!(s.get("id"), Some(&id));
    assert_eq!(s.get("playback_blocked"), Some(&true.into()));
    assert_eq!(s.get("segments"), Some(&2.into()));
    assert_eq!(s.get("delivery").unwrap().get("stage"), Some(&"validated".into()));
    let id = s.get("id").unwrap().clone();
    let n = text(&s, "text").unwrap().chars().count();
    command(&mut a, "delivery", obj! {"id"=>id.clone(),"event"=>"queued","through"=>0,"heard"=>0});
    command(&mut a, "delivery", obj! {"id"=>id,"event"=>"finished","through"=>n,"heard"=>n});
    let s = a.state().unwrap().get("speech").unwrap().clone();
    assert_eq!(s.get("segment"), Some(&2.into()));
    assert!(text(&s, "text").unwrap().contains("duet"));
}

#[test]
fn timeline_journal_entries_and_api_endpoints_expose_world_lore_and_milestones() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    let journal = a.journal().unwrap();
    assert_eq!(journal.get("ok"), Some(&true.into()));
    let entries = journal.get("journal").and_then(Value::as_array).unwrap();
    assert!(entries.len() >= 5);
    assert!(entries.iter().any(|e| text(e, "id") == Ok("vault_built")));
    assert!(entries.iter().any(|e| text(e, "id") == Ok("aqueduct_dry")));
    assert!(entries.iter().any(|e| text(e, "id") == Ok("descent")));
    let state = a.state().unwrap();
    let state_journal = state.get("journal").and_then(Value::as_array).unwrap();
    assert_eq!(state_journal.len(), entries.len());

    // Repair pump milestone
    a.game.pump_repaired = true;
    let updated = a.journal().unwrap();
    let updated_entries = updated.get("journal").and_then(Value::as_array).unwrap();
    assert!(updated_entries.iter().any(|e| text(e, "id") == Ok("pump_repaired")));
}

#[test]
fn universal_purchase_routing_and_llm_action_directing() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);
    // Start in workshop where merchant trades oil (3 coins) and chalk (1 coin)
    assert_eq!(a.game.room, "workshop");
    assert_eq!(a.game.coins, 5);

    // 1. Universal stage action for oil: *pays for the oil*
    let (res, _) = command(&mut a, "chat", obj! {"text" => "*pays for the oil*"});
    assert_eq!(res.get("status"), Some(&Value::str("accepted")));
    assert_eq!(a.game.coins, 2);
    assert!(a.game.owns("item:oil"));

    // 2. Universal stage action for chalk: *pays for the chalk*
    let (res, _) = command(&mut a, "chat", obj! {"text" => "*pays for the chalk*"});
    assert_eq!(res.get("status"), Some(&Value::str("accepted")));
    assert_eq!(a.game.coins, 1);
    assert!(a.game.owns("item:chalk"));

    // 3. Reset and test universal natural dialogue payment: "here are the coins for the chalk"
    let mut a2 = chunk_app(&d, false);
    assert_eq!(a2.game.coins, 5);
    let (res, _) = command(&mut a2, "chat", obj! {"text" => "here are the coins for the chalk"});
    assert_eq!(res.get("status"), Some(&Value::str("accepted")));
    assert_eq!(a2.game.coins, 4);
    assert!(a2.game.owns("item:chalk"));

    // 4. Test LLM [ACTION: buy item:oil] directive execution in complete()
    let mut a3 = chunk_app(&d, false);
    let mut settings = a3.config.public();
    set(&mut settings, "speech_mode", "dialogue".into());
    if let Value::Object(xs) = &mut settings {
        xs.retain(|(k, _)| k != "local_fallback" && k != "text_models");
    }
    set(&mut settings, "cap_micros", 0.into());
    command(&mut a3, "settings", settings);
    let mock_reply = "[ACTION: buy item:oil] {\"speech\": [\"Here is your hinge oil, may it serve you well.\"]}";
    let json_bytes = format!("{{\"message\":{{\"content\":{mock_reply:?}}}}}").into_bytes();
    let job = a3.command("chat", &obj! {"text" => "Tell me about your workshop."}).unwrap().1.remove(0);
    let req_body_str = std::str::from_utf8(&job.request.body).unwrap();
    assert!(req_body_str.contains("You are Ilen the merchant."));
    a3.complete(&job, Ok(json_bytes)).unwrap();
    assert!(a3.game.owns("item:oil"));
    assert_eq!(a3.game.coins, 2);
    let speech = a3.state().unwrap().get("speech").unwrap().clone();
    assert_eq!(text(&speech, "text").unwrap(), "Here is your hinge oil, may it serve you well.");
}

#[test]
fn test_universal_perception_sensing_and_epistemic_grounding() {
    let d = Dir::new();
    let mut a = chunk_app(&d, false);

    // 1. In workshop, merchant senses bronze key resting in room
    let (res, _) = command(&mut a, "chat", obj! {"text" => "where is the bronze key?"});
    assert_eq!(res.get("current_perception"), Some(&true.into()));
    let speech = a.state().unwrap().get("speech").unwrap().clone();
    let heard = text(&speech, "text").unwrap();
    assert!(heard.contains("bronze key is resting right here in this room"));

    // 2. Merchant perceives surroundings and wares
    let (res2, _) = command(&mut a, "chat", obj! {"text" => "what do you see?"});
    assert_eq!(res2.get("current_perception"), Some(&true.into()));
    let speech2 = a.state().unwrap().get("speech").unwrap().clone();
    let heard2 = text(&speech2, "text").unwrap();
    assert!(heard2.contains("hinge oil"));

    // 3. Natural inquiry does not get hijacked by clarification
    let (res3, _) = command(&mut a, "chat", obj! {"text" => "I'm looking for a bronze key and some hinge oil for the vault"});
    assert_ne!(res3.get("status"), Some(&Value::str("clarify")));

    // 4. Grounded persona prompt contains zero-hallucination constraint
    let (_, jobs) = command(&mut a, "chat", obj! {"text" => "do you have family?"});
    assert!(!jobs.is_empty());
    let req_body_str = std::str::from_utf8(&jobs[0].request.body).unwrap();
    assert!(req_body_str.contains("never invent fictional backstory, family, or unseen world facts"));
    assert!(req_body_str.contains("You are Ilen the merchant."));
}
