//! The Katch running on this computer, reached on its bridge (127.0.0.1, the extension's pairing
//! code). Plain blocking HTTP: the MCP server is one request at a time and needs no runtime.

use crate::{Error, Katch};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long a request may take: reading a long playlist or a slow site can take a while.
const REPLY_TIMEOUT: Duration = Duration::from_secs(300);
/// How long to wait for a Katch this server started to open its bridge.
const START_TIMEOUT: Duration = Duration::from_secs(20);

pub struct Local {
    ports: RangeInclusive<u16>,
    /// The pairing code, read fresh each time (the user can make a new one in Settings).
    token: Box<dyn Fn() -> String>,
    /// Starts Katch when it isn't running (none: just say so).
    launch: Option<Box<dyn Fn() -> std::io::Result<()>>>,
    port: Option<u16>,
}

/// What a bridge said to `/ping`.
enum Ping {
    Paired,
    /// A Katch that didn't take the code (it was regenerated, or another user's Katch).
    Refused,
    Nothing,
}

impl Local {
    pub fn new(ports: RangeInclusive<u16>, token: impl Fn() -> String + 'static) -> Local {
        Local { ports, token: Box::new(token), launch: None, port: None }
    }

    /// Starts Katch with `launch` when no Katch answers.
    pub fn launch_with(mut self, launch: impl Fn() -> std::io::Result<()> + 'static) -> Local {
        self.launch = Some(Box::new(launch));
        self
    }

    /// The port of a Katch that accepts the code, starting Katch if none is running.
    fn connect(&mut self) -> Result<(u16, String), Error> {
        self.find().map_err(Error::Unreachable)
    }

    fn find(&mut self) -> Result<(u16, String), String> {
        let token = (self.token)();
        if token.is_empty() {
            return Err("Katch has no pairing code yet: open Katch once, then try again.".into());
        }
        if let Some(port) = self.port {
            if matches!(ping(port, &token), Ping::Paired) {
                return Ok((port, token));
            }
            self.port = None;
        }
        let mut refused = self.scan(&token)?;
        if self.port.is_none()
            && !refused
            && let Some(launch) = &self.launch
        {
            launch().map_err(|e| format!("Katch isn't running and couldn't be started: {e}"))?;
            let start = Instant::now();
            while self.port.is_none() && start.elapsed() < START_TIMEOUT {
                std::thread::sleep(Duration::from_millis(300));
                refused = self.scan(&token)?;
            }
        }
        match self.port {
            Some(port) => Ok((port, token)),
            None if refused => Err("Katch is running but didn't accept its pairing code. If you made a new code in Katch → Settings → Extension, restart your MCP client.".into()),
            None if self.launch.is_some() => Err("Katch was started but didn't answer in time. Open Katch, then try again.".into()),
            None => Err("Katch isn't running. Open Katch, then try again.".into()),
        }
    }

    /// Looks for Katch on each port; returns whether one refused the code.
    fn scan(&mut self, token: &str) -> Result<bool, String> {
        let mut refused = false;
        for port in self.ports.clone() {
            match ping(port, token) {
                Ping::Paired => {
                    self.port = Some(port);
                    return Ok(false);
                }
                Ping::Refused => refused = true,
                Ping::Nothing => {}
            }
        }
        Ok(refused)
    }

    fn request(&mut self, method: &str, path: &str, body: Option<Value>) -> Result<Value, Error> {
        let (port, token) = self.connect()?;
        let unanswered = |e: Failure| Error::Unreachable(format!("Katch didn't answer: {}", e.why));
        let (status, reply) = match http(port, method, path, &token, body.as_ref(), REPLY_TIMEOUT) {
            Ok(r) => r,
            // Katch quit or restarted between requests: find it again and send once more. Only
            // when the request never reached it: an add it did receive must not be added twice.
            Err(Failure { sent: false, .. }) => {
                self.port = None;
                let (port, token) = self.connect()?;
                http(port, method, path, &token, body.as_ref(), REPLY_TIMEOUT).map_err(unanswered)?
            }
            Err(e) => return Err(unanswered(e)),
        };
        if (200..300).contains(&status) {
            Ok(reply)
        } else {
            Err(Error::Answered(reply["error"].as_str().map(String::from).unwrap_or_else(|| format!("Katch answered {status}"))))
        }
    }
}

impl Katch for Local {
    fn get(&mut self, path: &str) -> Result<Value, Error> {
        self.request("GET", path, None)
    }
    fn post(&mut self, path: &str, body: Value) -> Result<Value, Error> {
        self.request("POST", path, Some(body))
    }
}

fn ping(port: u16, token: &str) -> Ping {
    match http(port, "GET", "/ping", token, None, Duration::from_secs(2)) {
        Ok((200, reply)) if reply["app"] == "rdm" => {
            if reply["paired"] == true {
                Ping::Paired
            } else {
                Ping::Refused
            }
        }
        _ => Ping::Nothing,
    }
}

/// A request that failed, and whether it reached Katch (then it may have been done).
struct Failure {
    why: String,
    sent: bool,
}

/// One request to the bridge on `port`: its status and JSON body.
fn http(port: u16, method: &str, path: &str, token: &str, body: Option<&Value>, timeout: Duration) -> Result<(u16, Value), Failure> {
    let not_sent = |e: std::io::Error| Failure { why: e.to_string(), sent: false };
    let sent = |why: &str| Failure { why: why.to_string(), sent: true };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(500)).map_err(not_sent)?;
    stream.set_read_timeout(Some(timeout)).map_err(not_sent)?;
    let body = body.map(Value::to_string).unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-RDM-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    // Katch acts on a request only once it has all of it, so a failed write sent nothing.
    stream.write_all(request.as_bytes()).map_err(not_sent)?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(|e| sent(&e.to_string()))?;
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or_else(|| sent("not an HTTP reply"))?;
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let mut content = raw[split + 4..].to_vec();
    if head.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        content = unchunk(&content);
    }
    let status = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or_else(|| sent("not an HTTP reply"))?;
    Ok((status, serde_json::from_slice(&content).unwrap_or(Value::Null)))
}

/// A chunked HTTP body put back together.
fn unchunk(mut data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(end) = data.windows(2).position(|w| w == b"\r\n") {
        let size = usize::from_str_radix(String::from_utf8_lossy(&data[..end]).split(';').next().unwrap_or("").trim(), 16).unwrap_or(0);
        if size == 0 || data.len() < end + 2 + size {
            break;
        }
        out.extend_from_slice(&data[end + 2..end + 2 + size]);
        data = &data[(end + 4 + size).min(data.len())..];
    }
    out
}

/// The program an MCP client should start: the AppImage itself on Linux (the binary inside it
/// lives in a temporary mount), else this executable.
pub fn katch_exe() -> PathBuf {
    std::env::var_os("APPIMAGE").map(PathBuf::from).or_else(|| std::env::current_exe().ok()).unwrap_or_default()
}

/// Starts Katch in the background (tray only), fully apart from this process: it must outlive the
/// MCP server and not hold its stdin/stdout.
pub fn launch_detached(exe: &Path) -> std::io::Result<()> {
    use std::process::{Command, Stdio};
    let exe_text = exe.to_string_lossy();
    let mut cmd = if let Some(bundle) = exe_text.find(".app/Contents/MacOS/").map(|i| &exe_text[..i + 4]) {
        // macOS: through Launch Services, so it is a normal app (Dock, menu bar) and not our child.
        let mut c = Command::new("open");
        c.args(["-g", "-a", bundle, "--args", "--background"]);
        c
    } else {
        let mut c = Command::new(exe);
        c.arg("--background");
        c
    };
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Windows hands every inheritable handle to a child, Stdio::null or not: Katch would hold
        // the MCP client's pipes open for as long as it runs, and the client would never see this
        // server's output end. Our own stdin/stdout/stderr stay ours.
        unsafe extern "system" {
            fn GetStdHandle(which: u32) -> isize;
            fn SetHandleInformation(handle: isize, mask: u32, flags: u32) -> i32;
        }
        const HANDLE_FLAG_INHERIT: u32 = 1;
        for which in [-10i32, -11, -12] {
            // SAFETY: plain Win32 calls on this process's own standard handles; a missing or
            // invalid handle makes SetHandleInformation fail, which is harmless here.
            unsafe {
                let handle = GetStdHandle(which as u32);
                if handle != 0 && handle != -1 {
                    SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0);
                }
            }
        }
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        // Out of the MCP client's job, so closing the client doesn't close Katch; a job that
        // forbids breaking away refuses this, and then Katch starts inside it.
        if cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB).spawn().is_ok() {
            return Ok(());
        }
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunked_bodies_are_put_back_together() {
        assert_eq!(unchunk(b"4\r\n{\"a\"\r\n3\r\n:1}\r\n0\r\n\r\n"), b"{\"a\":1}");
        assert_eq!(unchunk(b""), b"");
    }

    #[test]
    fn an_add_that_reached_katch_is_never_sent_twice() {
        // A Katch that answers /ping but drops the connection after reading an /add.
        let listener = std::net::TcpListener::bind("127.0.0.1:48993").unwrap();
        let adds = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = adds.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                if request.starts_with("GET /ping") {
                    let body = r#"{"app":"rdm","paired":true}"#;
                    let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len());
                } else {
                    seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            }
        });
        let mut k = Local::new(48993..=48993, || "code".into());
        let err = k.post("/add", serde_json::json!({ "url": "https://example.com/a.zip" })).unwrap_err();
        assert!(matches!(err, Error::Unreachable(ref why) if why.contains("didn't answer")), "{err:?}");
        assert_eq!(adds.load(std::sync::atomic::Ordering::SeqCst), 1, "not resent: Katch may have added it");
    }

    #[test]
    fn nothing_running_and_no_way_to_start_it_is_said_plainly() {
        // Ports nothing listens on.
        let mut k = Local::new(48991..=48992, || "code".into());
        assert_eq!(k.get("/list").unwrap_err(), Error::Unreachable("Katch isn't running. Open Katch, then try again.".into()));
        let mut k = Local::new(48991..=48992, String::new);
        assert!(String::from(k.get("/list").unwrap_err()).contains("no pairing code"));
    }
}
