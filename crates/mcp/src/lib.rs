//! Katch as an MCP server: `katch --mcp` speaks the Model Context Protocol (JSON-RPC, one message
//! per line) on stdin and stdout, so Claude or any other MCP client can read links and download
//! them with the Katch that is running. It goes through the same local bridge as the browser
//! extension, with the same pairing code, and can't do more than the extension can: add, read,
//! list, pause and resume. Removing or deleting stays in Katch's window.

pub mod local;
mod tools;

use serde_json::{Value, json};
use std::io::{BufRead, Write};

pub use local::Local;

/// The running Katch, as the MCP server sees it: its bridge's HTTP API.
pub trait Katch {
    fn get(&mut self, path: &str) -> Result<Value, Error>;
    fn post(&mut self, path: &str, body: Value) -> Result<Value, Error>;
}

/// Why a request to Katch failed.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// Katch answered, with this error (a page it can't read, an unknown download…).
    Answered(String),
    /// Katch couldn't be reached: not running, wrong pairing code…
    Unreachable(String),
}

impl From<Error> for String {
    fn from(e: Error) -> String {
        match e {
            Error::Answered(s) | Error::Unreachable(s) => s,
        }
    }
}

/// Protocol versions this server speaks, newest first. A client asking for one of them gets it;
/// any other gets the newest.
const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Katch is the user's download manager. Use download for anything they want saved \
(videos in a chosen quality or as MP3, whole playlists, files, GitHub repositories, magnet links); it returns \
at once and Katch downloads in the background. Use inspect_link first when you need a link's qualities or a \
playlist's videos, and list_downloads to report progress.";

/// Answers MCP messages from `input` on `output` until the input ends.
pub fn serve(input: impl BufRead, mut output: impl Write, katch: &mut dyn Katch) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = handle_line(&line, katch) {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// One incoming message → the reply to write, if any (notifications get none).
pub fn handle_line(line: &str, katch: &mut dyn Katch) -> Option<Value> {
    let msg: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
    };
    let id = msg.get("id").cloned()?; // no id: a notification (initialized, cancelled…)
    let method = msg["method"].as_str().unwrap_or_default();
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => {
            let asked = params["protocolVersion"].as_str().unwrap_or_default();
            let version = VERSIONS.iter().find(|v| **v == asked).unwrap_or(&VERSIONS[0]);
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "katch", "title": "Katch", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools::list() }),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or_default();
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            match tools::call(name, &args, katch) {
                Some(Ok(text)) => json!({ "content": [{ "type": "text", "text": text }] }),
                // A tool that ran and failed: the model reads why and can try again.
                Some(Err(text)) => json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
                None => return Some(error(id, -32602, &format!("unknown tool: {name}"))),
            }
        }
        _ => return Some(error(id, -32601, &format!("method not found: {method}"))),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// What to paste into an MCP client's config (Claude Desktop's `claude_desktop_config.json`)
/// to use this Katch: `exe` started with `--mcp`.
pub fn client_config(exe: &str) -> String {
    let config = json!({ "mcpServers": { "katch": { "command": exe, "args": ["--mcp"] } } });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

/// The same for Claude Code, as one command to run in a terminal.
pub fn claude_code_command(exe: &str) -> String {
    format!("claude mcp add katch -- \"{exe}\" --mcp")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in Katch: answers from canned replies and records every request.
    #[derive(Default)]
    pub(crate) struct Fake {
        pub probe: Option<Result<Value, String>>,
        pub list: Value,
        pub sent: Vec<(String, Value)>,
        pub unreachable: bool,
    }

    impl Katch for Fake {
        fn get(&mut self, path: &str) -> Result<Value, Error> {
            self.sent.push((format!("GET {path}"), Value::Null));
            if self.unreachable {
                return Err(Error::Unreachable("Katch isn't running. Open Katch, then try again.".into()));
            }
            Ok(self.list.clone())
        }
        fn post(&mut self, path: &str, body: Value) -> Result<Value, Error> {
            self.sent.push((path.to_string(), body));
            if self.unreachable {
                return Err(Error::Unreachable("Katch isn't running. Open Katch, then try again.".into()));
            }
            match path {
                "/probe" => self.probe.clone().unwrap_or_else(|| Err("no probe".into())).map_err(Error::Answered),
                _ => Ok(json!({ "id": self.sent.len() })),
            }
        }
    }

    fn ask(katch: &mut dyn Katch, msg: Value) -> Value {
        handle_line(&msg.to_string(), katch).expect("a reply")
    }

    #[test]
    fn initialize_agrees_on_a_protocol_version() {
        let mut k = Fake::default();
        let r = ask(&mut k, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}));
        assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(r["result"]["serverInfo"]["name"], "katch");
        assert!(r["result"]["capabilities"]["tools"].is_object());
        let r = ask(&mut k, json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}));
        assert_eq!(r["result"]["protocolVersion"], VERSIONS[0], "an unknown version gets the newest");
    }

    #[test]
    fn notifications_get_no_reply_and_unknown_methods_an_error() {
        let mut k = Fake::default();
        assert!(handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#, &mut k).is_none());
        let r = ask(&mut k, json!({"jsonrpc":"2.0","id":7,"method":"resources/list"}));
        assert_eq!(r["error"]["code"], -32601);
        assert_eq!(r["id"], 7);
        let r = handle_line("{not json", &mut k).unwrap();
        assert_eq!(r["error"]["code"], -32700);
    }

    #[test]
    fn the_tools_are_listed_with_schemas() {
        let mut k = Fake::default();
        let r = ask(&mut k, json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}));
        let names: Vec<&str> = r["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["download", "inspect_link", "list_downloads", "pause_download", "resume_download"]);
        for t in r["result"]["tools"].as_array().unwrap() {
            assert_eq!(t["inputSchema"]["type"], "object", "{t}");
            assert!(t["description"].as_str().unwrap().len() > 20);
        }
    }

    #[test]
    fn an_unknown_tool_is_a_protocol_error() {
        let mut k = Fake::default();
        let r = ask(&mut k, json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"format_disk","arguments":{}}}));
        assert_eq!(r["error"]["code"], -32602);
    }

    #[test]
    fn serve_answers_line_by_line() {
        let mut k = Fake::default();
        let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n";
        let mut out = Vec::new();
        serve(input.as_bytes(), &mut out, &mut k).unwrap();
        let lines: Vec<Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1]["id"], 2);
    }

    #[test]
    fn config_snippets_name_the_exe_with_mcp() {
        let c: Value = serde_json::from_str(&client_config(r"C:\Program Files\Katch\snag.exe")).unwrap();
        assert_eq!(c["mcpServers"]["katch"]["command"], r"C:\Program Files\Katch\snag.exe");
        assert_eq!(c["mcpServers"]["katch"]["args"], json!(["--mcp"]));
        assert_eq!(claude_code_command("/usr/bin/katch"), "claude mcp add katch -- \"/usr/bin/katch\" --mcp");
    }
}
