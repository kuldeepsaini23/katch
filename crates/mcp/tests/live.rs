//! The MCP server against a real Katch download manager and its bridge.

use rdm_core::Manager;
use rdm_mcp::{Local, serve};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::{Arc, Mutex};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

/// A Katch with a known pairing code, adding downloads paused (the test stays offline).
async fn katch(dir: &Path) -> Manager {
    let m = Manager::start(dir.join("state.json"));
    let mut s = m.snapshot().await.settings;
    s.extension_token = TOKEN.into();
    s.start_immediately = false;
    s.download_dir = dir.join("dl");
    m.update_settings(s).await;
    m
}

/// Runs `messages` through the MCP server; the replies, one per request.
fn talk(local: &mut Local, messages: &[Value]) -> Vec<Value> {
    let input: String = messages.iter().map(|m| format!("{m}\n")).collect();
    let mut out = Vec::new();
    serve(input.as_bytes(), &mut out, local).unwrap();
    String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn call(id: u64, name: &str, arguments: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": name, "arguments": arguments } })
}

fn text(reply: &Value) -> &str {
    reply["result"]["content"][0]["text"].as_str().unwrap_or_default()
}

#[test]
fn claude_downloads_lists_and_pauses_through_katch() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let m = rt.block_on(katch(dir.path()));
    rt.block_on(rdm_bridge::start(m.clone(), 48931..=48935)).unwrap();

    let mut local = Local::new(48931..=48935, || TOKEN.into());
    let replies = talk(
        &mut local,
        &[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } } }),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            call(2, "download", json!({ "url": "https://example.com/files/setup.exe" })),
            call(3, "list_downloads", json!({})),
        ],
    );
    assert_eq!(replies.len(), 3, "{replies:?}");
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "katch");
    assert!(text(&replies[1]).contains("a file link") && text(&replies[1]).contains("download #"), "{}", replies[1]);
    let items = rt.block_on(m.snapshot()).items;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].url, "https://example.com/files/setup.exe");
    assert!(text(&replies[2]).starts_with(&format!("#{} paused", items[0].id.0)), "{}", replies[2]);
    assert!(text(&replies[2]).ends_with("setup.exe"), "{}", replies[2]);

    let id = items[0].id.0;
    let replies = talk(&mut local, &[call(4, "resume_download", json!({ "id": id })), call(5, "pause_download", json!({ "id": 999 }))]);
    assert_eq!(text(&replies[0]), format!("Download #{id} resumed."));
    assert_ne!(rt.block_on(m.snapshot()).items[0].status, rdm_core::Status::Paused);
    assert_eq!(replies[1]["result"]["isError"], true);
    assert!(text(&replies[1]).contains("no download with id 999"), "{}", replies[1]);
}

#[test]
fn a_wrong_code_is_explained_not_retried_forever() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let m = rt.block_on(katch(dir.path()));
    rt.block_on(rdm_bridge::start(m, 48936..=48940)).unwrap();
    let started = Arc::new(Mutex::new(0));
    let count = started.clone();
    let mut local = Local::new(48936..=48940, || "not-the-code".into()).launch_with(move || {
        *count.lock().unwrap() += 1;
        Ok(())
    });
    let replies = talk(&mut local, &[call(1, "list_downloads", json!({}))]);
    assert_eq!(replies[0]["result"]["isError"], true);
    assert!(text(&replies[0]).contains("didn't accept its pairing code"), "{}", replies[0]);
    assert_eq!(*started.lock().unwrap(), 0, "a Katch is running: don't start another");
}

#[test]
fn katch_is_started_when_it_isnt_running() {
    let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let dir = tempfile::tempdir().unwrap();
    let m = rt.block_on(katch(dir.path()));
    // "Starting Katch" here opens its bridge a moment later, as the real app does.
    let (rt2, m2) = (rt.clone(), m.clone());
    let mut local = Local::new(48941..=48945, || TOKEN.into()).launch_with(move || {
        let (rt3, m3) = (rt2.clone(), m2.clone());
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(700));
            rt3.block_on(rdm_bridge::start(m3, 48941..=48945)).unwrap();
        });
        Ok(())
    });
    let replies = talk(&mut local, &[call(1, "download", json!({ "url": "https://github.com/kuldeepsaini23/katch" }))]);
    assert!(text(&replies[0]).contains("GitHub repository"), "{}", replies[0]);
    let items = rt.block_on(m.snapshot()).items;
    assert_eq!(items[0].url, "https://github.com/kuldeepsaini23/katch/archive/HEAD.zip");
}
