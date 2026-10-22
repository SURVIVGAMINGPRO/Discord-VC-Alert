//! Discord local RPC (IPC) client and the watcher state machine, ported from
//! v1's `IPC` and `Watcher` classes.
//!
//! Hard rules (never break these):
//! - Only the Discord **desktop app's** documented local RPC is used, plus the
//!   standard OAuth2 token exchange. No bot, no user token, no gateway.
//! - Read-only: the only commands ever sent are AUTHORIZE, AUTHENTICATE,
//!   SUBSCRIBE, UNSUBSCRIBE and GET_SELECTED_VOICE_CHANNEL (plus handshake/PONG).
//!   `tests/watcher.rs` checks this against a mock Discord.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

pub const OP_HANDSHAKE: u32 = 0;
pub const OP_FRAME: u32 = 1;
pub const OP_CLOSE: u32 = 2;
pub const OP_PING: u32 = 3;
pub const OP_PONG: u32 = 4;
pub const TOKEN_URL: &str = "https://discord.com/api/oauth2/token";
pub const CONNECT_ERROR: &str =
    "Could not reach the Discord desktop app. Is it running? (The browser version of Discord does not work.)";
/// Commands this program is allowed to send. Anything else is a bug.
pub const ALLOWED_COMMANDS: [&str; 5] =
    ["AUTHORIZE", "AUTHENTICATE", "SUBSCRIBE", "UNSUBSCRIBE", "GET_SELECTED_VOICE_CHANNEL"];

// ------------------------------------------------------------ transport --

pub trait Stream: AsyncRead + AsyncWrite + Unpin + Send + Sync {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + Sync> Stream for T {}

/// Candidate socket paths, in v1's order (plus the OS temp dir as a fallback
/// for apps started from Finder, where TMPDIR may not be set).
#[cfg(unix)]
pub fn candidate_paths(n: u32) -> Vec<PathBuf> {
    let mut bases: Vec<PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .iter()
        .filter_map(|k| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from))
        .collect();
    bases.push(std::env::temp_dir());
    bases.push(PathBuf::from("/tmp"));
    let mut out: Vec<PathBuf> = Vec::new();
    for b in bases {
        let p = b.join(format!("discord-ipc-{n}"));
        if !out.contains(&p) {
            out.push(p);
        }
    }
    if let Some(x) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        let x = PathBuf::from(x);
        out.push(x.join("app/com.discordapp.Discord").join(format!("discord-ipc-{n}")));
        out.push(x.join("snap.discord").join(format!("discord-ipc-{n}")));
    }
    out
}

#[cfg(windows)]
pub fn candidate_paths(n: u32) -> Vec<PathBuf> {
    vec![PathBuf::from(format!(r"\\.\pipe\discord-ipc-{n}"))]
}

#[cfg(unix)]
async fn open_path(p: &Path) -> std::io::Result<Box<dyn Stream>> {
    Ok(Box::new(tokio::net::UnixStream::connect(p).await?))
}

#[cfg(windows)]
async fn open_path(p: &Path) -> std::io::Result<Box<dyn Stream>> {
    let c = tokio::net::windows::named_pipe::ClientOptions::new().open(p.as_os_str())?;
    Ok(Box::new(c))
}

pub async fn connect(override_path: Option<&Path>) -> Result<Box<dyn Stream>, Fail> {
    if let Some(p) = override_path {
        return open_path(p).await.map_err(|_| Fail::Conn(CONNECT_ERROR.into()));
    }
    for n in 0..10 {
        for p in candidate_paths(n) {
            if let Ok(s) = open_path(&p).await {
                return Ok(s);
            }
        }
    }
    Err(Fail::Conn(CONNECT_ERROR.into()))
}

pub async fn write_frame<W: AsyncWrite + Unpin + ?Sized>(w: &mut W, op: u32, payload: &Value) -> std::io::Result<()> {
    let body = serde_json::to_vec(payload)?;
    let mut buf = Vec::with_capacity(8 + body.len());
    buf.extend_from_slice(&op.to_le_bytes());
    buf.extend_from_slice(&(body.len() as u32).to_le_bytes());
    buf.extend_from_slice(&body);
    w.write_all(&buf).await?;
    w.flush().await
}

/// Read one raw frame. EOF is `Ok(None)`.
pub async fn read_frame<R: AsyncRead + Unpin + ?Sized>(r: &mut R) -> std::io::Result<Option<(u32, Value)>> {
    let mut header = [0u8; 8];
    match r.read_exact(&mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let op = u32::from_le_bytes(header[0..4].try_into().unwrap());
    let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
    if len > 16 * 1024 * 1024 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut body = vec![0u8; len];
    if len > 0 {
        r.read_exact(&mut body).await?;
    }
    let payload = if body.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&body)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
    };
    Ok(Some((op, payload)))
}

// ---------------------------------------------------------------- errors --

#[derive(Debug, Clone, PartialEq)]
pub enum Fail {
    /// A problem the user has to fix; don't retry.
    Setup(String),
    /// Connection trouble; retry in 5 s.
    Conn(String),
    /// Discord answered a command with an error.
    Rpc { code: i64, message: String },
}

impl std::fmt::Display for Fail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fail::Setup(s) | Fail::Conn(s) => write!(f, "{s}"),
            Fail::Rpc { code, message } => write!(f, "RPC error {code}: {message}"),
        }
    }
}

impl From<std::io::Error> for Fail {
    fn from(e: std::io::Error) -> Self {
        Fail::Conn(format!("Connection to Discord failed: {e}"))
    }
}

// ----------------------------------------------------------------- types --

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Stopped,
    Connecting,
    IdleNoChannel,
    WatchingChannel,
    Reconnecting,
    SetupError,
}

/// A watched person who is in your channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub key: String,
    pub id: String,
    pub username: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub client_id: String,
    pub scopes: Vec<String>,
    pub access_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Creds {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}

impl Creds {
    fn sorted_scopes(&self) -> Vec<String> {
        let mut s = self.scopes.clone();
        s.sort();
        s
    }
}

pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;
/// POSTs the form fields to Discord's token endpoint and returns the JSON.
pub type TokenFn = Arc<dyn Fn(Vec<(String, String)>) -> BoxFut<Result<Value, String>> + Send + Sync>;

/// The real token call (v1 `token_request`).
pub fn http_token_fn() -> TokenFn {
    Arc::new(|form: Vec<(String, String)>| {
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .user_agent("DiscordBot (vc_alert, 2.0)")
                .build()
                .map_err(|e| format!("network error: {e}"))?;
            let resp = client
                .post(TOKEN_URL)
                .form(&form)
                .send()
                .await
                .map_err(|e| format!("network error: {e}"))?;
            let code = resp.status();
            let text = resp.text().await.map_err(|e| format!("network error: {e}"))?;
            if !code.is_success() {
                let snippet: String = text.chars().take(300).collect();
                return Err(format!("HTTP {}: {snippet}", code.as_u16()));
            }
            serde_json::from_str(&text).map_err(|e| format!("bad token response: {e}"))
        })
    })
}

/// Everything the watcher needs from the app. Settings are read live, so
/// changes apply while watching (targets, cooldown, quiet, alert-if-present).
pub trait Host: Send + Sync + 'static {
    fn targets(&self) -> Vec<String>;
    fn cooldown(&self) -> f64;
    fn quiet(&self) -> bool;
    fn alert_if_present(&self) -> bool;
    fn load_token(&self) -> Option<Token>;
    fn save_token(&self, token: &Token);
    fn status(&self, status: Status, detail: &str);
    fn presence(&self, members: Vec<Member>);
    /// A watched person joined (or was already there): flash the UI.
    fn alert(&self, key: &str);
    /// Play this person's sound (not called in Quiet mode).
    fn play(&self, key: &str);
    /// A watched person left your channel (not called in Quiet mode, nor
    /// when you switch channels yourself).
    fn left(&self, _key: &str) {}
    /// Notified when the list of people changes (another list chosen, someone
    /// added or removed): the watcher then re-reads who's in your call, quietly.
    fn wake(&self) -> Option<Arc<tokio::sync::Notify>> {
        None
    }
    fn log(&self, msg: &str);
}

// --------------------------------------------------------------- session --

struct Session<'a> {
    io: Box<dyn Stream>,
    /// Bytes read but not yet taken as a whole frame. Reading only appends
    /// here, so a read interrupted by the poll timer loses nothing.
    buf: Vec<u8>,
    pending: VecDeque<Value>,
    last_poll: Instant,
    host: &'a Arc<dyn Host>,
    creds: &'a Creds,
    token_fn: &'a TokenFn,
    state: &'a mut WatchState,
}

/// What survives between reconnects (alert cooldowns) and what must be
/// cleaned up after a session ends (presence).
#[derive(Default)]
pub struct WatchState {
    channel_id: Option<String>,
    present: Vec<Member>,
    last_alert: HashMap<String, Instant>,
    /// Group and DM calls: Discord may not send join/leave events for them
    /// (or may refuse the subscription), so we ask who's here every
    /// `POLL_EVERY` and work out the difference.
    polling: bool,
}

/// How often a group or DM call is checked for who's in it.
pub const POLL_EVERY: Duration = Duration::from_secs(2);

/// Take one whole frame out of `buf`, if it holds one.
fn take_frame(buf: &mut Vec<u8>) -> std::io::Result<Option<(u32, Value)>> {
    if buf.len() < 8 {
        return Ok(None);
    }
    let op = u32::from_le_bytes(buf[0..4].try_into().unwrap());
    let len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
    if len > 16 * 1024 * 1024 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large"));
    }
    if buf.len() < 8 + len {
        return Ok(None);
    }
    let body: Vec<u8> = buf.drain(..8 + len).skip(8).collect();
    let payload = if body.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&body).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
    };
    Ok(Some((op, payload)))
}

fn has_frame(buf: &[u8]) -> bool {
    buf.len() >= 8 && buf.len() >= 8 + u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize
}

impl<'a> Session<'a> {
    /// Read more bytes. Cancel-safe: `read_buf` only appends what it read.
    async fn fill(&mut self) -> Result<(), Fail> {
        if self.io.read_buf(&mut self.buf).await? == 0 {
            return Err(Fail::Conn("Connection to Discord closed.".into()));
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Value, Fail> {
        loop {
            let (op, payload) = loop {
                if let Some(f) = take_frame(&mut self.buf)? {
                    break f;
                }
                self.fill().await?;
            };
            match op {
                OP_PING => write_frame(&mut self.io, OP_PONG, &payload).await?,
                OP_PONG => {}
                OP_CLOSE => return Err(Fail::Conn(format!("Discord closed the connection: {payload}"))),
                _ => return Ok(payload),
            }
        }
    }

    async fn command(&mut self, cmd: &str, args: Value, evt: Option<&str>) -> Result<Value, Fail> {
        debug_assert!(ALLOWED_COMMANDS.contains(&cmd));
        let nonce = uuid::Uuid::new_v4().to_string();
        let mut msg = json!({ "cmd": cmd, "nonce": nonce, "args": args });
        if let Some(e) = evt {
            msg["evt"] = json!(e);
        }
        write_frame(&mut self.io, OP_FRAME, &msg).await?;
        loop {
            let m = self.recv().await?;
            if m.get("nonce").and_then(Value::as_str) == Some(nonce.as_str()) {
                if m.get("evt").and_then(Value::as_str) == Some("ERROR") {
                    let d = m.get("data").cloned().unwrap_or(json!({}));
                    return Err(Fail::Rpc {
                        code: d.get("code").and_then(Value::as_i64).unwrap_or(0),
                        message: d.get("message").and_then(Value::as_str).unwrap_or("").to_string(),
                    });
                }
                return Ok(m.get("data").cloned().unwrap_or(Value::Null));
            }
            if m.get("cmd").and_then(Value::as_str) == Some("DISPATCH") {
                self.pending.push_back(m); // handled later by the main loop, in order
            }
        }
    }

    /// Returns the READY payload's `user` (used by "Test connection").
    async fn handshake(&mut self) -> Result<Value, Fail> {
        write_frame(&mut self.io, OP_HANDSHAKE, &json!({ "v": 1, "client_id": self.creds.client_id })).await?;
        loop {
            let m = self.recv().await?;
            if m.get("cmd").and_then(Value::as_str) == Some("DISPATCH")
                && m.get("evt").and_then(Value::as_str) == Some("READY")
            {
                return Ok(m.pointer("/data/user").cloned().unwrap_or(Value::Null));
            }
            if m.get("evt").and_then(Value::as_str) == Some("ERROR") {
                let d = m.get("data").cloned().unwrap_or(Value::Null);
                return Err(Fail::Setup(format!("Discord rejected the handshake: {d}")));
            }
        }
    }

    async fn authenticate_with(&mut self, access_token: &str) -> Result<(), Fail> {
        self.command("AUTHENTICATE", json!({ "access_token": access_token }), None).await.map(|_| ())
    }

    async fn token_request(&self, extra: Vec<(&str, String)>) -> Result<Token, String> {
        let mut form = vec![
            ("client_id".to_string(), self.creds.client_id.clone()),
            ("client_secret".to_string(), self.creds.client_secret.clone()),
        ];
        form.extend(extra.into_iter().map(|(k, v)| (k.to_string(), v)));
        let v = (self.token_fn)(form).await?;
        let access = v.get("access_token").and_then(Value::as_str).ok_or("no access_token in reply")?;
        Ok(Token {
            client_id: self.creds.client_id.clone(),
            scopes: self.creds.sorted_scopes(),
            access_token: access.to_string(),
            refresh_token: v.get("refresh_token").and_then(Value::as_str).map(str::to_string),
        })
    }

    /// v1 `authenticate`: saved token → refresh → AUTHORIZE popup + exchange.
    async fn authenticate(&mut self) -> Result<(), Fail> {
        let tok = self.host.load_token().filter(|t| {
            t.client_id == self.creds.client_id && t.scopes == self.creds.sorted_scopes()
        });
        if let Some(tok) = tok {
            match self.authenticate_with(&tok.access_token).await {
                Ok(()) => return Ok(()),
                Err(Fail::Rpc { .. }) => {}
                Err(e) => return Err(e),
            }
            if let Some(refresh) = tok.refresh_token.clone() {
                if let Ok(new) = self
                    .token_request(vec![("grant_type", "refresh_token".into()), ("refresh_token", refresh)])
                    .await
                {
                    self.host.save_token(&new);
                    match self.authenticate_with(&new.access_token).await {
                        Ok(()) => return Ok(()),
                        Err(Fail::Rpc { .. }) => {}
                        Err(e) => return Err(e),
                    }
                }
            }
        }
        self.host.log("Discord will ask you to approve this app (a popup inside Discord).");
        let data = match self
            .command(
                "AUTHORIZE",
                json!({ "client_id": self.creds.client_id, "scopes": self.creds.scopes }),
                None,
            )
            .await
        {
            Ok(d) => d,
            Err(e @ Fail::Rpc { .. }) => {
                return Err(Fail::Setup(format!(
                    "Discord refused the authorization request ({e}). Check the Application ID, \
                     and that your account is on the app's App Testers list."
                )))
            }
            Err(e) => return Err(e),
        };
        let code = data.get("code").and_then(Value::as_str).unwrap_or("").to_string();
        let tok = self
            .token_request(vec![
                ("grant_type", "authorization_code".into()),
                ("code", code),
                ("redirect_uri", self.creds.redirect_uri.clone()),
            ])
            .await
            .map_err(|e| {
                Fail::Setup(format!(
                    "Token exchange failed ({e}). Check the Client Secret, and that {} is listed \
                     under OAuth2 → Redirects.",
                    self.creds.redirect_uri
                ))
            })?;
        self.host.save_token(&tok);
        match self.authenticate_with(&tok.access_token).await {
            Ok(()) => Ok(()),
            Err(e @ Fail::Rpc { .. }) => Err(Fail::Setup(format!("Discord rejected the new token: {e}"))),
            Err(e) => Err(e),
        }
    }

    /// v1 `Watcher.match`: the numeric ID, else the unique username
    /// (case-insensitive). Display names and nicknames are never looked at.
    fn matches(&self, user: Option<&Value>) -> Option<String> {
        let user = user?;
        let keys = self.host.targets();
        let uid = match user.get("id") {
            Some(Value::String(s)) => s.trim().to_string(),
            Some(Value::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        if !uid.is_empty() && keys.iter().any(|k| k.trim() == uid) {
            return Some(uid);
        }
        let uname = user.get("username").and_then(Value::as_str).unwrap_or("").trim().to_lowercase();
        if !uname.is_empty() && keys.iter().any(|k| k.trim().to_lowercase() == uname) {
            return Some(uname);
        }
        None
    }

    fn remember(&mut self, key: &str, user: &Value) {
        let m = Member {
            key: key.to_string(),
            id: match user.get("id") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Number(n)) => n.to_string(),
                _ => String::new(),
            },
            username: user.get("username").and_then(Value::as_str).unwrap_or("").to_string(),
            avatar: user.get("avatar").and_then(Value::as_str).map(str::to_string),
        };
        // Insertion-ordered: re-setting keeps the position.
        if let Some(existing) = self.state.present.iter_mut().find(|p| p.key == key) {
            *existing = m;
        } else {
            self.state.present.push(m);
        }
    }

    fn publish(&self) {
        self.host.presence(self.state.present.clone());
    }

    fn alert(&mut self, key: &str, why: &str) {
        let now = Instant::now();
        let cooldown = self.host.cooldown().clamp(0.0, 600.0);
        if let Some(last) = self.state.last_alert.get(key) {
            if now.duration_since(*last).as_secs_f64() < cooldown {
                return;
            }
        }
        self.state.last_alert.insert(key.to_string(), now);
        self.host.log(&format!("ALERT: target {key} {why}."));
        self.host.alert(key);
        if self.host.quiet() {
            return; // Quiet mode: the UI still flashes, no sound
        }
        self.host.play(key);
    }

    async fn switch_to(&mut self, channel_id: Option<String>) -> Result<(), Fail> {
        if channel_id == self.state.channel_id {
            return Ok(());
        }
        if let Some(old) = self.state.channel_id.clone() {
            for evt in ["VOICE_STATE_CREATE", "VOICE_STATE_DELETE"] {
                match self.command("UNSUBSCRIBE", json!({ "channel_id": old }), Some(evt)).await {
                    Ok(_) | Err(Fail::Rpc { .. }) => {}
                    Err(e) => return Err(e),
                }
            }
        }
        self.state.channel_id = channel_id.clone();
        self.state.present.clear();
        self.state.polling = false;
        let Some(id) = channel_id else {
            self.host.log("Not in a voice channel.");
            self.host.status(Status::IdleNoChannel, "");
            self.publish();
            return Ok(());
        };
        // Group and DM calls may refuse these; then we fall back to polling.
        let mut subscribed = true;
        for evt in ["VOICE_STATE_CREATE", "VOICE_STATE_DELETE"] {
            match self.command("SUBSCRIBE", json!({ "channel_id": id }), Some(evt)).await {
                Ok(_) => {}
                Err(Fail::Rpc { code, message }) => {
                    self.host.log(&format!("Discord refused {evt} for this call ({code}: {message})."));
                    subscribed = false;
                }
                Err(e) => return Err(e),
            }
        }
        // Who is already here? Always read: the overlay needs it even when the
        // "already present" beep is off.
        let info = self.command("GET_SELECTED_VOICE_CHANNEL", json!({}), None).await?;
        let in_server = matches!(info.get("guild_id"), Some(Value::String(g)) if !g.is_empty());
        self.state.polling = !subscribed || !in_server;
        self.last_poll = Instant::now();
        let mut name = info.get("name").and_then(Value::as_str).unwrap_or("").trim().to_string();
        if name.is_empty() && !in_server {
            // DM calls have no channel name.
            name = if info.get("type").and_then(Value::as_i64) == Some(3) { "Group call" } else { "Call" }.into();
        }
        self.host.log("Watching your current voice channel.");
        self.host.status(Status::WatchingChannel, &name);
        if let Some(Value::Array(states)) = info.get("voice_states") {
            for vs in states.clone() {
                if let Some(hit) = self.matches(vs.get("user")) {
                    self.remember(&hit, &vs["user"]);
                    if self.host.alert_if_present() {
                        self.alert(&hit, "already in the channel");
                    }
                }
            }
        }
        self.publish();
        Ok(())
    }

    async fn handle(&mut self, m: Value) -> Result<(), Fail> {
        if m.get("cmd").and_then(Value::as_str) != Some("DISPATCH") {
            return Ok(());
        }
        let data = m.get("data").cloned().unwrap_or(json!({}));
        match m.get("evt").and_then(Value::as_str) {
            Some("VOICE_CHANNEL_SELECT") => {
                let id = match data.get("channel_id") {
                    Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
                    _ => None,
                };
                self.switch_to(id).await?;
            }
            Some("VOICE_STATE_CREATE") => {
                if let Some(hit) = self.matches(data.get("user")) {
                    self.remember(&hit, &data["user"]);
                    self.publish();
                    self.alert(&hit, "joined");
                }
            }
            Some("VOICE_STATE_DELETE") => {
                if let Some(hit) = self.matches(data.get("user")) {
                    let before = self.state.present.len();
                    self.state.present.retain(|p| p.key != hit);
                    if self.state.present.len() != before {
                        self.publish();
                        if !self.host.quiet() {
                            self.host.left(&hit);
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn run(&mut self) -> Result<(), Fail> {
        self.handshake().await?;
        self.authenticate().await?;
        self.state.channel_id = None;
        self.command("SUBSCRIBE", json!({}), Some("VOICE_CHANNEL_SELECT")).await?;
        let current = self.command("GET_SELECTED_VOICE_CHANNEL", json!({}), None).await?;
        let id = current.get("id").and_then(Value::as_str).map(str::to_string);
        if id.is_none() {
            // v1 bug: switch_to(None) is a no-op here, so the status stayed
            // "connecting" when you weren't in a voice channel at Start.
            self.host.status(Status::IdleNoChannel, "");
            self.publish();
        }
        self.switch_to(id).await?;
        self.host.log(&format!("Ready. Watching for {} people.", self.host.targets().len()));
        loop {
            while let Some(m) = self.pending.pop_front() {
                self.handle(m).await?;
            }
            let due = if self.state.polling {
                POLL_EVERY.saturating_sub(self.last_poll.elapsed())
            } else {
                Duration::from_secs(3600)
            };
            if due.is_zero() {
                self.sync(true).await?;
                continue;
            }
            if !has_frame(&self.buf) {
                // Wait for Discord, the next poll, or a changed list. `fill`
                // is cancel-safe, so whichever comes first loses nothing.
                let wake = self.host.wake();
                tokio::select! {
                    r = self.fill() => r?,
                    _ = tokio::time::sleep(due) => continue,
                    _ = async {
                        match &wake {
                            Some(n) => n.notified().await,
                            None => std::future::pending().await,
                        }
                    } => {
                        self.sync(false).await?;
                        continue;
                    }
                }
                if !has_frame(&self.buf) {
                    continue;
                }
            }
            let m = self.recv().await?;
            self.handle(m).await?;
        }
    }

    /// Ask who's in your call and turn the difference into joins and leaves.
    /// `announce`: as if Discord had sent them (group and DM calls, polled);
    /// otherwise quietly (your list changed: no beeps, no leave sounds).
    async fn sync(&mut self, announce: bool) -> Result<(), Fail> {
        self.last_poll = Instant::now();
        if self.state.channel_id.is_none() && !announce {
            return Ok(());
        }
        let info = self.command("GET_SELECTED_VOICE_CHANNEL", json!({}), None).await?;
        let id = info.get("id").and_then(Value::as_str).map(str::to_string);
        if id != self.state.channel_id {
            return self.switch_to(id).await;
        }
        let mut here: Vec<(String, Value)> = Vec::new();
        if let Some(Value::Array(states)) = info.get("voice_states") {
            for vs in states {
                if let Some(hit) = self.matches(vs.get("user")) {
                    if !here.iter().any(|(k, _)| *k == hit) {
                        here.push((hit, vs["user"].clone()));
                    }
                }
            }
        }
        let gone: Vec<String> = self
            .state
            .present
            .iter()
            .filter(|p| !here.iter().any(|(k, _)| *k == p.key))
            .map(|p| p.key.clone())
            .collect();
        let new: Vec<(String, Value)> =
            here.into_iter().filter(|(k, _)| !self.state.present.iter().any(|p| p.key == *k)).collect();
        if gone.is_empty() && new.is_empty() {
            return Ok(());
        }
        self.state.present.retain(|p| !gone.contains(&p.key));
        for (k, u) in &new {
            self.remember(k, u);
        }
        self.publish();
        if !announce {
            return Ok(());
        }
        for (k, _) in &new {
            self.alert(k, "joined");
        }
        if !self.host.quiet() {
            for k in &gone {
                self.host.left(k);
            }
        }
        Ok(())
    }
}

/// v1 `run_forever`: keep a session going, reconnecting every 5 s, until
/// `stop` is cancelled (which takes effect immediately on every platform).
pub async fn run_forever(
    host: Arc<dyn Host>,
    creds: Creds,
    token_fn: TokenFn,
    stop: CancellationToken,
    ipc_path: Option<PathBuf>,
) {
    let mut state = WatchState::default();
    loop {
        if stop.is_cancelled() {
            break;
        }
        host.status(Status::Connecting, "");
        let result = {
            let attempt = async {
                let io = connect(ipc_path.as_deref()).await?;
                let mut s = Session {
                    io,
                    buf: Vec::new(),
                    pending: VecDeque::new(),
                    last_poll: Instant::now(),
                    host: &host,
                    creds: &creds,
                    token_fn: &token_fn,
                    state: &mut state,
                };
                s.run().await
            };
            tokio::select! {
                r = attempt => Some(r),
                _ = stop.cancelled() => None,
            }
        };
        state.channel_id = None;
        if !state.present.is_empty() {
            state.present.clear();
            host.presence(Vec::new()); // nobody is "in the channel" any more
        }
        match result {
            None => break,
            Some(Ok(())) => {}
            Some(Err(Fail::Setup(msg))) => {
                host.log(&format!("Setup problem: {msg}"));
                host.status(Status::SetupError, &msg);
                return;
            }
            Some(Err(e)) => {
                if stop.is_cancelled() {
                    break;
                }
                host.log(&format!("Disconnected ({e}). Retrying in 5 s..."));
                host.status(Status::Reconnecting, &e.to_string());
            }
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
            _ = stop.cancelled() => break,
        }
    }
    host.status(Status::Stopped, "");
}

/// Connect, handshake and log in once, then disconnect. Returns the Discord
/// username this app is talking to. (Settings → Connection → Test.)
pub async fn test_connection(
    host: Arc<dyn Host>,
    creds: Creds,
    token_fn: TokenFn,
    ipc_path: Option<PathBuf>,
) -> Result<String, Fail> {
    let mut state = WatchState::default();
    let io = connect(ipc_path.as_deref()).await?;
    let mut s = Session {
        io,
        buf: Vec::new(),
        pending: VecDeque::new(),
        last_poll: Instant::now(),
        host: &host,
        creds: &creds,
        token_fn: &token_fn,
        state: &mut state,
    };
    let user = s.handshake().await?;
    s.authenticate().await?;
    Ok(user.get("username").and_then(Value::as_str).unwrap_or("").to_string())
}
