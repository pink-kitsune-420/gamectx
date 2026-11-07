//! Offline fake transports exercise production routing/state, never installed services.
use super::*;
use gamectx::{obj, studio::ProviderBudget};
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
    let mut t = providers::Network { dir: d.0.clone() };
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
        let caps = gamectx::life::ContextCaps::from_hints(
            gamectx::deployment::ModelSize::parse(size)
                .unwrap()
                .workload_hints(gamectx::deployment::Workload::Dialogue),
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
