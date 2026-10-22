//! The watcher against a mock Discord desktop app (Unix socket).
#![cfg(unix)]

use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use vc_core::discord::*;

// ------------------------------------------------------------ mock Discord --

#[derive(Default)]
struct MockState {
    received: Vec<(String, Option<String>, Value)>,
    valid_tokens: HashSet<String>,
    channel: Value,
}

struct Mock {
    path: PathBuf,
    state: Arc<Mutex<MockState>>,
    emit: Arc<Mutex<Option<mpsc::UnboundedSender<(u32, Value)>>>>,
    _dir: tempfile::TempDir,
}

impl Mock {
    async fn start(channel: Value, valid: &[&str]) -> Mock {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("discord-ipc-0");
        let listener = UnixListener::bind(&path).unwrap();
        let state = Arc::new(Mutex::new(MockState {
            valid_tokens: valid.iter().map(|s| s.to_string()).collect(),
            channel,
            ..Default::default()
        }));
        let emit: Arc<Mutex<Option<mpsc::UnboundedSender<(u32, Value)>>>> = Arc::new(Mutex::new(None));
        let (st, em) = (state.clone(), emit.clone());
        tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = listener.accept().await else { return };
                let (mut r, mut w) = tokio::io::split(sock);
                let (tx, mut rx) = mpsc::unbounded_channel::<(u32, Value)>();
                *em.lock().unwrap() = Some(tx.clone());
                tokio::spawn(async move {
                    while let Some((op, v)) = rx.recv().await {
                        if write_frame(&mut w, op, &v).await.is_err() {
                            return;
                        }
                    }
                });
                let st = st.clone();
                tokio::spawn(async move {
                    while let Ok(Some((op, msg))) = read_frame(&mut r).await {
                        match op {
                            OP_HANDSHAKE => {
                                let _ = tx.send((OP_FRAME, json!({"cmd": "DISPATCH", "evt": "READY",
                                    "data": {"v": 1, "user": {"id": "1", "username": "me"}}})));
                                let _ = tx.send((OP_PING, json!({"nonce": "ping-1"})));
                            }
                            OP_PONG => st.lock().unwrap().received.push(("PONG".into(), None, msg)),
                            OP_FRAME => {
                                let cmd = msg["cmd"].as_str().unwrap_or("").to_string();
                                let evt = msg.get("evt").and_then(Value::as_str).map(str::to_string);
                                let args = msg["args"].clone();
                                let nonce = msg["nonce"].clone();
                                st.lock().unwrap().received.push((cmd.clone(), evt.clone(), args.clone()));
                                let reply = match cmd.as_str() {
                                    "AUTHORIZE" => json!({"cmd": cmd, "nonce": nonce, "data": {"code": "auth-code"}}),
                                    "AUTHENTICATE" => {
                                        let t = args["access_token"].as_str().unwrap_or("");
                                        if st.lock().unwrap().valid_tokens.contains(t) {
                                            json!({"cmd": cmd, "nonce": nonce, "data": {"user": {"id": "1"}}})
                                        } else {
                                            json!({"cmd": cmd, "nonce": nonce, "evt": "ERROR",
                                                   "data": {"code": 4009, "message": "Invalid token"}})
                                        }
                                    }
                                    "GET_SELECTED_VOICE_CHANNEL" => {
                                        json!({"cmd": cmd, "nonce": nonce, "data": st.lock().unwrap().channel.clone()})
                                    }
                                    _ => json!({"cmd": cmd, "nonce": nonce, "evt": evt, "data": {"evt": evt}}),
                                };
                                let _ = tx.send((OP_FRAME, reply));
                            }
                            _ => {}
                        }
                    }
                });
            }
        });
        Mock { path, state, emit, _dir: dir }
    }

    fn dispatch(&self, evt: &str, data: Value) {
        if let Some(tx) = self.emit.lock().unwrap().as_ref() {
            let _ = tx.send((OP_FRAME, json!({"cmd": "DISPATCH", "evt": evt, "data": data})));
        }
    }

    fn cmds(&self) -> Vec<String> {
        self.state.lock().unwrap().received.iter().map(|r| r.0.clone()).collect()
    }
}

// --------------------------------------------------------------- test host --

#[derive(Default)]
struct TestHost {
    targets: Mutex<Vec<String>>,
    quiet: AtomicBool,
    cooldown: Mutex<f64>,
    token: Mutex<Option<Token>>,
    statuses: Mutex<Vec<(Status, String)>>,
    presence: Mutex<Vec<Vec<Member>>>,
    alerts: Mutex<Vec<String>>,
    plays: Mutex<Vec<String>>,
    lefts: Mutex<Vec<String>>,
    present_alert: AtomicBool,
}

impl Host for TestHost {
    fn targets(&self) -> Vec<String> { self.targets.lock().unwrap().clone() }
    fn cooldown(&self) -> f64 { *self.cooldown.lock().unwrap() }
    fn quiet(&self) -> bool { self.quiet.load(Ordering::SeqCst) }
    fn alert_if_present(&self) -> bool { self.present_alert.load(Ordering::SeqCst) }
    fn load_token(&self) -> Option<Token> { self.token.lock().unwrap().clone() }
    fn save_token(&self, t: &Token) { *self.token.lock().unwrap() = Some(t.clone()) }
    fn status(&self, s: Status, d: &str) { self.statuses.lock().unwrap().push((s, d.into())) }
    fn presence(&self, m: Vec<Member>) { self.presence.lock().unwrap().push(m) }
    fn alert(&self, k: &str) { self.alerts.lock().unwrap().push(k.into()) }
    fn play(&self, k: &str) { self.plays.lock().unwrap().push(k.into()) }
    fn left(&self, k: &str) { self.lefts.lock().unwrap().push(k.into()) }
    fn log(&self, _m: &str) {}
}

fn host(targets: &[&str]) -> Arc<TestHost> {
    let h = TestHost::default();
    *h.targets.lock().unwrap() = targets.iter().map(|s| s.to_string()).collect();
    h.present_alert.store(true, Ordering::SeqCst);
    Arc::new(h)
}

fn creds() -> Creds {
    Creds {
        client_id: "123".into(),
        client_secret: "secret".into(),
        redirect_uri: "http://localhost".into(),
        scopes: vec!["rpc".into(), "rpc.voice.read".into()],
    }
}

fn token_fn(log: Arc<Mutex<Vec<Vec<(String, String)>>>>) -> TokenFn {
    Arc::new(move |form: Vec<(String, String)>| {
        let log = log.clone();
        Box::pin(async move {
            let grant = form.iter().find(|(k, _)| k == "grant_type").map(|(_, v)| v.clone()).unwrap_or_default();
            log.lock().unwrap().push(form);
            Ok(if grant == "refresh_token" {
                json!({"access_token": "tok-2", "refresh_token": "ref-2"})
            } else {
                json!({"access_token": "tok-1", "refresh_token": "ref-1"})
            })
        })
    })
}

async fn until(what: &str, mut f: impl FnMut() -> bool) {
    for _ in 0..200 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("timed out waiting for {what}");
}

fn last_presence(h: &TestHost) -> Vec<String> {
    h.presence.lock().unwrap().last().map(|m| m.iter().map(|x| x.key.clone()).collect()).unwrap_or_default()
}

// ------------------------------------------------------------------- tests --

#[tokio::test]
async fn full_flow_join_leave_and_stop() {
    let channel = json!({"id": "c1", "name": "General", "voice_states": [
        {"user": {"id": "555", "username": "Maria_K", "avatar": "abc"}},
        {"user": {"id": "556", "username": "stranger"}}
    ]});
    let mock = Mock::start(channel, &["tok-1"]).await;
    let h = host(&["maria_k", "moderator", "123456789012345678"]);
    let tokens = Arc::new(Mutex::new(Vec::new()));
    let stop = CancellationToken::new();
    let dynh: Arc<dyn Host> = h.clone();
    let task = tokio::spawn(run_forever(dynh, creds(), token_fn(tokens.clone()), stop.clone(), Some(mock.path.clone())));

    until("already-present alert", || h.alerts.lock().unwrap().len() == 1).await;
    assert_eq!(last_presence(&h), vec!["maria_k"]);
    assert_eq!(h.plays.lock().unwrap().as_slice(), ["maria_k"]);
    assert!(h.statuses.lock().unwrap().contains(&(Status::WatchingChannel, "General".into())));
    assert_eq!(h.token.lock().unwrap().as_ref().unwrap().access_token, "tok-1");
    {
        let forms = tokens.lock().unwrap();
        assert!(forms[0].contains(&("code".into(), "auth-code".into())));
        assert!(forms[0].contains(&("client_secret".into(), "secret".into())));
    }

    // Display names / nicknames never match; usernames match case-insensitively; IDs as strings.
    mock.dispatch("VOICE_STATE_CREATE", json!({"user": {"id": "9", "username": "someone", "global_name": "moderator"}, "nick": "moderator"}));
    mock.dispatch("VOICE_STATE_CREATE", json!({"user": {"id": "10", "username": "MODERATOR"}}));
    until("join", || last_presence(&h) == vec!["maria_k", "moderator"]).await;
    mock.dispatch("VOICE_STATE_CREATE", json!({"user": {"id": 123456789012345678u64, "username": "x"}}));
    until("id join", || last_presence(&h).len() == 3).await;
    assert_eq!(h.alerts.lock().unwrap().as_slice(), ["maria_k", "moderator", "123456789012345678"]);

    // Live target list: someone added while running is matched.
    h.targets.lock().unwrap().push("late_one".into());
    mock.dispatch("VOICE_STATE_CREATE", json!({"user": {"id": "11", "username": "late_one"}}));
    until("live target", || last_presence(&h).len() == 4).await;

    // Leaves keep join order for the rest.
    mock.dispatch("VOICE_STATE_DELETE", json!({"user": {"id": "10", "username": "moderator"}}));
    until("leave", || last_presence(&h) == vec!["maria_k", "123456789012345678", "late_one"]).await;
    assert_eq!(h.lefts.lock().unwrap().as_slice(), ["moderator"], "a real leave is reported");
    // Someone who isn't watched leaving is not.
    mock.dispatch("VOICE_STATE_DELETE", json!({"user": {"id": "556", "username": "stranger"}}));

    // Quiet: alert still posted, no sound.
    h.quiet.store(true, Ordering::SeqCst);
    mock.dispatch("VOICE_STATE_CREATE", json!({"user": {"id": "10", "username": "moderator"}}));
    until("quiet alert", || h.alerts.lock().unwrap().len() == 5).await;
    assert_eq!(h.plays.lock().unwrap().len(), 4);
    // Quiet: a leave is not reported for a sound either.
    mock.dispatch("VOICE_STATE_DELETE", json!({"user": {"id": "11", "username": "late_one"}}));
    until("quiet leave", || !last_presence(&h).contains(&"late_one".to_string())).await;
    assert_eq!(h.lefts.lock().unwrap().len(), 1, "no leave sound in Quiet");
    h.quiet.store(false, Ordering::SeqCst);

    // Moving to no channel clears presence.
    mock.dispatch("VOICE_CHANNEL_SELECT", json!({"channel_id": null}));
    until("idle", || h.statuses.lock().unwrap().iter().any(|s| s.0 == Status::IdleNoChannel)).await;
    assert!(last_presence(&h).is_empty());
    assert_eq!(h.lefts.lock().unwrap().len(), 1, "switching channels yourself is not a leave");

    // PING answered with PONG; only read/subscribe commands were ever sent.
    let cmds = mock.cmds();
    assert!(cmds.contains(&"PONG".to_string()));
    for c in &cmds {
        assert!(c == "PONG" || ALLOWED_COMMANDS.contains(&c.as_str()), "sent a forbidden command: {c}");
    }
    assert!(cmds.iter().filter(|c| *c == "UNSUBSCRIBE").count() == 2);

    // Stop takes effect immediately.
    let t0 = std::time::Instant::now();
    stop.cancel();
    tokio::time::timeout(Duration::from_secs(1), task).await.expect("stop within 1 s").unwrap();
    assert!(t0.elapsed() < Duration::from_millis(500));
    assert_eq!(h.statuses.lock().unwrap().last().unwrap().0, Status::Stopped);
}

#[tokio::test]
async fn cached_token_skips_authorize_and_refresh_works() {
    let mock = Mock::start(Value::Null, &["tok-2"]).await;
    let h = host(&["maria_k"]);
    *h.token.lock().unwrap() = Some(Token {
        client_id: "123".into(),
        scopes: vec!["rpc".into(), "rpc.voice.read".into()],
        access_token: "expired".into(),
        refresh_token: Some("ref-1".into()),
    });
    let tokens = Arc::new(Mutex::new(Vec::new()));
    let stop = CancellationToken::new();
    let dynh: Arc<dyn Host> = h.clone();
    let task = tokio::spawn(run_forever(dynh, creds(), token_fn(tokens.clone()), stop.clone(), Some(mock.path.clone())));
    until("idle", || h.statuses.lock().unwrap().iter().any(|s| s.0 == Status::IdleNoChannel)).await;
    assert!(!mock.cmds().contains(&"AUTHORIZE".to_string()));
    assert_eq!(h.token.lock().unwrap().as_ref().unwrap().access_token, "tok-2");
    assert!(tokens.lock().unwrap()[0].contains(&("grant_type".into(), "refresh_token".into())));
    stop.cancel();
    task.await.unwrap();
}

#[tokio::test]
async fn token_for_other_app_is_ignored() {
    let mock = Mock::start(Value::Null, &["tok-1"]).await;
    let h = host(&["maria_k"]);
    *h.token.lock().unwrap() = Some(Token {
        client_id: "999".into(),
        scopes: vec!["rpc".into(), "rpc.voice.read".into()],
        access_token: "tok-1".into(),
        refresh_token: None,
    });
    let stop = CancellationToken::new();
    let dynh: Arc<dyn Host> = h.clone();
    let task = tokio::spawn(run_forever(dynh, creds(), token_fn(Arc::new(Mutex::new(vec![]))), stop.clone(), Some(mock.path.clone())));
    until("idle", || h.statuses.lock().unwrap().iter().any(|s| s.0 == Status::IdleNoChannel)).await;
    assert!(mock.cmds().contains(&"AUTHORIZE".to_string()));
    stop.cancel();
    task.await.unwrap();
}

#[tokio::test]
async fn already_present_alert_can_be_off_and_cooldown_applies() {
    let channel = json!({"id": "c1", "name": "x", "voice_states": [{"user": {"id": "5", "username": "maria_k"}}]});
    let mock = Mock::start(channel, &["tok-1"]).await;
    let h = host(&["maria_k"]);
    h.present_alert.store(false, Ordering::SeqCst);
    *h.cooldown.lock().unwrap() = 600.0;
    let stop = CancellationToken::new();
    let dynh: Arc<dyn Host> = h.clone();
    let task = tokio::spawn(run_forever(dynh, creds(), token_fn(Arc::new(Mutex::new(vec![]))), stop.clone(), Some(mock.path.clone())));
    until("presence", || last_presence(&h) == vec!["maria_k"]).await;
    assert!(h.alerts.lock().unwrap().is_empty(), "already-present alert is off");
    mock.dispatch("VOICE_STATE_DELETE", json!({"user": {"id": "5", "username": "maria_k"}}));
    mock.dispatch("VOICE_STATE_CREATE", json!({"user": {"id": "5", "username": "maria_k"}}));
    until("first join alert", || h.alerts.lock().unwrap().len() == 1).await;
    mock.dispatch("VOICE_STATE_DELETE", json!({"user": {"id": "5", "username": "maria_k"}}));
    mock.dispatch("VOICE_STATE_CREATE", json!({"user": {"id": "5", "username": "maria_k"}}));
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(h.alerts.lock().unwrap().len(), 1, "second join is inside the cooldown");
    *h.cooldown.lock().unwrap() = 0.0; // cooldown changes apply live
    mock.dispatch("VOICE_STATE_CREATE", json!({"user": {"id": "5", "username": "maria_k"}}));
    until("cooldown off", || h.alerts.lock().unwrap().len() == 2).await;
    stop.cancel();
    task.await.unwrap();
}

#[tokio::test]
async fn unreachable_discord_reconnects_until_stopped() {
    let h = host(&["maria_k"]);
    let stop = CancellationToken::new();
    let dynh: Arc<dyn Host> = h.clone();
    let task = tokio::spawn(run_forever(dynh, creds(), token_fn(Arc::new(Mutex::new(vec![]))), stop.clone(),
        Some(PathBuf::from("/nonexistent/discord-ipc-0"))));
    until("reconnecting", || h.statuses.lock().unwrap().iter().any(|s| s.0 == Status::Reconnecting)).await;
    assert!(h.statuses.lock().unwrap().iter().any(|s| s.1.contains("Is it running?")));
    stop.cancel();
    tokio::time::timeout(Duration::from_secs(1), task).await.unwrap().unwrap();
}

#[tokio::test]
async fn test_connection_reports_username() {
    let mock = Mock::start(Value::Null, &["tok-1"]).await;
    let h = host(&[]);
    let dynh: Arc<dyn Host> = h.clone();
    let name = test_connection(dynh, creds(), token_fn(Arc::new(Mutex::new(vec![]))), Some(mock.path.clone())).await.unwrap();
    assert_eq!(name, "me");
}
