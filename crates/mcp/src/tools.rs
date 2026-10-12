//! The tools Katch offers an MCP client, each a call or two to Katch's bridge.

use crate::{Error, Katch};
use rdm_core::route::{Route, github_zip, route};
use serde_json::{Value, json};

/// The most playlist videos one `download` call adds when the client didn't pick them.
const MAX_BATCH: usize = 100;
/// The most playlist videos `inspect_link` names.
const MAX_SHOWN: usize = 50;

pub fn list() -> Value {
    json!([
        {
            "name": "download",
            "title": "Download with Katch",
            "description": "Download a link with Katch, the user's download manager. Works for video pages (YouTube, Vimeo, X and 1,800+ more sites) in the quality asked for or as MP3, whole playlists or chosen videos from one, direct file links, GitHub repositories (saved as a ZIP of the code) and magnet links. Returns at once with the download ids; Katch keeps downloading in the background into the user's Downloads folder.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "The page, file, repository or magnet link." },
                    "quality": { "type": "string", "enum": ["best", "2160p", "1440p", "1080p", "720p", "480p", "360p", "audio"], "description": "For videos: the highest resolution to take (the best available up to it), or 'audio' for MP3. Default: best." },
                    "videos": { "type": "array", "items": { "type": "integer", "minimum": 1 }, "description": "For playlists and channels: which videos to take, by position (1 = first) as inspect_link numbers them. Default: all (up to 100)." },
                    "as_file": { "type": "boolean", "description": "Save the link as a plain file, without looking for a video on it." }
                },
                "required": ["url"]
            }
        },
        {
            "name": "inspect_link",
            "title": "Read a link",
            "description": "Read a link with Katch without downloading anything: the video's title, length and the qualities on offer with their sizes, or for a playlist or channel the numbered list of its videos.",
            "inputSchema": {
                "type": "object",
                "properties": { "url": { "type": "string", "description": "The link to read." } },
                "required": ["url"]
            },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "list_downloads",
            "title": "List downloads",
            "description": "Katch's downloads, newest first, with their id, status (queued, running, paused, done, failed), progress, speed and where each finished file is.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "active_only": { "type": "boolean", "description": "Only the ones not finished yet." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200, "description": "How many to show. Default 20." }
                }
            },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "pause_download",
            "title": "Pause a download",
            "description": "Pause one of Katch's downloads by its id (from download or list_downloads). It keeps what it has and can be resumed later.",
            "inputSchema": { "type": "object", "properties": { "id": { "type": "integer" } }, "required": ["id"] }
        },
        {
            "name": "resume_download",
            "title": "Resume a download",
            "description": "Resume a paused or failed download by its id, from where it stopped.",
            "inputSchema": { "type": "object", "properties": { "id": { "type": "integer" } }, "required": ["id"] }
        }
    ])
}

/// Runs tool `name`: `None` if there is no such tool, else its text (Ok) or why it failed (Err).
pub fn call(name: &str, args: &Value, katch: &mut dyn Katch) -> Option<Result<String, String>> {
    Some(match name {
        "download" => download(args, katch),
        "inspect_link" => inspect(args, katch),
        "list_downloads" => list_downloads(args, katch),
        "pause_download" => control(args, "pause", katch),
        "resume_download" => control(args, "resume", katch),
        _ => return None,
    })
}

fn url_arg(args: &Value) -> Result<String, String> {
    let url = args["url"].as_str().map(str::trim).unwrap_or_default();
    if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("magnet:") {
        Ok(url.to_string())
    } else {
        Err(format!("Katch downloads http(s) and magnet links; {url:?} isn't one."))
    }
}

/// Links Katch takes as they are (no video to look for): files, repositories, torrents, galleries.
fn plain(url: &str) -> Option<&'static str> {
    if github_zip(url).is_some() {
        return Some("a GitHub repository: Katch saves its code as a ZIP");
    }
    match route(url) {
        Route::Media => None,
        Route::File => Some("a file link: Katch saves the file as it is"),
        Route::Torrent => Some("a torrent: Katch downloads what it shares"),
        Route::Gallery => Some("a picture gallery: Katch saves its pictures"),
    }
}

/// The quality asked for: `Some(height)` for video (0 = best), `None` for audio.
fn wanted(args: &Value) -> Result<Option<u32>, String> {
    let q = args["quality"].as_str().unwrap_or("best").trim().to_ascii_lowercase();
    Ok(match q.as_str() {
        "" | "best" => Some(0),
        "audio" | "mp3" | "audio only" => None,
        "4k" => Some(2160),
        "2k" => Some(1440),
        other => match other.trim_end_matches('p').parse::<u32>() {
            Ok(h) if h > 0 => Some(h),
            _ => return Err(format!("quality {q:?}: use best, 2160p, 1440p, 1080p, 720p, 480p, 360p or audio")),
        },
    })
}

/// A video option's height (its `format` is `{"Video": {"max_height": h}}` or `{"Live": …}`).
fn height(format: &Value) -> Option<u64> {
    format.get("Video").or_else(|| format.get("Live")).and_then(|f| f["max_height"].as_u64())
}

/// The probe's option for `want`: the tallest video up to the height asked (else the smallest
/// there is), or the MP3 one.
fn pick(options: &[Value], want: Option<u32>) -> Option<&Value> {
    match want {
        None => options.iter().find(|o| o["format"] == "AudioMp3"),
        Some(h) => {
            let videos = options.iter().filter(|o| height(&o["format"]).is_some());
            let fits = videos.clone().filter(|o| h == 0 || height(&o["format"]) <= Some(u64::from(h))).max_by_key(|o| height(&o["format"]));
            fits.or_else(|| videos.min_by_key(|o| height(&o["format"])))
        }
    }
}

fn download(args: &Value, katch: &mut dyn Katch) -> Result<String, String> {
    let url = url_arg(args)?;
    let want = wanted(args)?;
    if args["as_file"].as_bool() == Some(true) {
        let id = katch.post("/add", json!({ "url": url, "kind": "file" }))?;
        return Ok(format!("Katch is downloading {url} as a file (download #{}).", id["id"]));
    }
    if let Some(what) = plain(&url) {
        let id = katch.post("/add", json!({ "url": url }))?;
        return Ok(format!("Katch is downloading {url}, {what} (download #{}).", id["id"]));
    }
    let info = katch.post("/probe", json!({ "url": url })).map_err(|e| match e {
        Error::Answered(e) => format!("Katch couldn't find a video on {url}: {e}\nIf it's a direct file link, call download again with as_file: true."),
        Error::Unreachable(e) => e,
    })?;
    let title = info["title"].as_str().unwrap_or("video").to_string();
    let entries = info["entries"].as_array().cloned().unwrap_or_default();
    if !entries.is_empty() {
        return playlist(&title, &entries, args, want, katch);
    }
    let options = info["options"].as_array().cloned().unwrap_or_default();
    let Some(option) = pick(&options, want) else {
        let offered: Vec<&str> = options.iter().filter_map(|o| o["label"].as_str()).collect();
        return Err(format!("\"{title}\" has no {} version. Offered: {}.", if want.is_none() { "audio" } else { "video" }, offered.join(", ")));
    };
    let body = json!({ "url": url, "title": title, "format": option["format"], "thumbnail": info["thumbnail"], "duration": info["duration"] });
    let id = katch.post("/add-media", body)?;
    let size = option["size"].as_u64().map(|s| format!(", about {}", bytes(s))).unwrap_or_default();
    Ok(format!("Katch is downloading \"{title}\" as {}{size} (download #{}).", option["label"].as_str().unwrap_or("video"), id["id"]))
}

fn playlist(title: &str, entries: &[Value], args: &Value, want: Option<u32>, katch: &mut dyn Katch) -> Result<String, String> {
    let picked: Vec<(usize, &Value)> = match args["videos"].as_array() {
        Some(positions) => {
            let mut out = Vec::new();
            for p in positions {
                let n = p.as_u64().unwrap_or(0) as usize;
                let e = entries.get(n.wrapping_sub(1)).ok_or_else(|| format!("\"{title}\" has {} videos; there is no video {n}.", entries.len()))?;
                out.push((n, e));
            }
            out
        }
        None => entries.iter().enumerate().take(MAX_BATCH).map(|(i, e)| (i + 1, e)).collect(),
    };
    // Katch's own playlist picker sends the same: one video at a time, at the height asked.
    let format = match want {
        None => json!("AudioMp3"),
        Some(0) => json!({ "Video": { "max_height": 4320 } }),
        Some(h) => json!({ "Video": { "max_height": h } }),
    };
    let mut lines = Vec::new();
    for (n, e) in &picked {
        let name = e["title"].as_str().unwrap_or("video");
        let id = katch.post("/add-media", json!({ "url": e["url"], "title": name, "format": format, "duration": e["duration"] }))?;
        lines.push(format!("{n}. {name} (download #{})", id["id"]));
    }
    let how = match want {
        None => "as MP3".to_string(),
        Some(0) => "in the best quality".to_string(),
        Some(h) => format!("at up to {h}p"),
    };
    let mut text = format!("Katch is downloading {} of the {} videos in \"{title}\" {how}:\n{}", picked.len(), entries.len(), lines.join("\n"));
    if args["videos"].is_null() && entries.len() > MAX_BATCH {
        text.push_str(&format!("\nThat's the first {MAX_BATCH}; pass videos to take others."));
    }
    Ok(text)
}

fn inspect(args: &Value, katch: &mut dyn Katch) -> Result<String, String> {
    let url = url_arg(args)?;
    if let Some(what) = plain(&url) {
        return Ok(format!("{url} is {what}. Nothing to choose: download takes it as it is."));
    }
    let info = katch.post("/probe", json!({ "url": url })).map_err(|e| match e {
        Error::Answered(e) => format!("Katch couldn't read {url}: {e}"),
        Error::Unreachable(e) => e,
    })?;
    let title = info["title"].as_str().unwrap_or("Untitled");
    let entries = info["entries"].as_array().cloned().unwrap_or_default();
    if !entries.is_empty() {
        let mut text = format!("Playlist \"{title}\", {} videos:\n", entries.len());
        for (i, e) in entries.iter().take(MAX_SHOWN).enumerate() {
            let length = e["duration"].as_f64().map(|d| format!(" ({})", clock(d))).unwrap_or_default();
            text.push_str(&format!("{}. {}{length}\n", i + 1, e["title"].as_str().unwrap_or("?")));
        }
        if entries.len() > MAX_SHOWN {
            text.push_str(&format!("…and {} more.\n", entries.len() - MAX_SHOWN));
        }
        text.push_str("Qualities: best, 2160p, 1440p, 1080p, 720p, 480p, 360p or audio (MP3), up to what each video has.");
        return Ok(text);
    }
    let mut text = format!("\"{title}\"");
    if let Some(d) = info["duration"].as_f64() {
        text.push_str(&format!(", {}", clock(d)));
    }
    if info["live"] == true {
        text.push_str(" (live now: Katch records it)");
    }
    text.push_str("\nQualities:");
    for o in info["options"].as_array().into_iter().flatten() {
        let size = o["size"].as_u64().map(|s| format!(" (about {})", bytes(s))).unwrap_or_default();
        text.push_str(&format!("\n- {}{size}", o["label"].as_str().unwrap_or("?")));
    }
    Ok(text)
}

fn list_downloads(args: &Value, katch: &mut dyn Katch) -> Result<String, String> {
    let reply = katch.get("/list")?;
    let active_only = args["active_only"].as_bool() == Some(true);
    let limit = args["limit"].as_u64().unwrap_or(20).clamp(1, 200) as usize;
    let items: Vec<&Value> = reply["items"].as_array().into_iter().flatten().filter(|i| !active_only || i["status"] != "done").take(limit).collect();
    if items.is_empty() {
        return Ok(if active_only { "Nothing downloading right now." } else { "No downloads yet." }.into());
    }
    Ok(items.iter().map(|i| line(i)).collect::<Vec<_>>().join("\n"))
}

/// One download as a line: `#12 running 45% · 120 MB of 260 MB · 8.4 MB/s · name`.
fn line(i: &Value) -> String {
    let status = i["status"].as_str().unwrap_or("?");
    let done = i["downloaded"].as_u64().unwrap_or(0);
    let mut parts = vec![format!("#{} {status}", i["id"])];
    match (status, i["total"].as_u64()) {
        ("done", _) => {}
        (_, Some(total)) if total > 0 => parts.push(format!("{}% · {} of {}", done * 100 / total, bytes(done), bytes(total))),
        _ if done > 0 => parts.push(bytes(done)),
        _ => {}
    }
    if status == "running" {
        parts.push(format!("{}/s", bytes(i["speed"].as_u64().unwrap_or(0))));
    }
    parts.push(i["name"].as_str().unwrap_or("?").to_string());
    if let Some(e) = i["error"].as_str() {
        parts.push(format!("error: {e}"));
    }
    if status == "done"
        && let Some(p) = i["path"].as_str()
    {
        parts.push(format!("saved to {p}"));
    }
    parts.join(" · ")
}

fn control(args: &Value, action: &str, katch: &mut dyn Katch) -> Result<String, String> {
    let id = args["id"].as_u64().ok_or("id: the download's number, from download or list_downloads")?;
    katch.post("/control", json!({ "id": id, "action": action }))?;
    Ok(format!("Download #{id} {}.", if action == "pause" { "paused" } else { "resumed" }))
}

fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 { format!("{n} B") } else { format!("{v:.1} {}", UNITS[u]) }
}

fn clock(seconds: f64) -> String {
    let s = seconds.round() as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::Fake;

    fn video(options: Value) -> Value {
        json!({ "title": "Big Buck Bunny", "duration": 596.0, "playlist": 0, "entries": [], "options": options, "thumbnail": "https://img/t.jpg" })
    }

    fn options() -> Value {
        json!([
            { "label": "2160p", "size": 3_000_000_000u64, "format": { "Video": { "max_height": 2160 } } },
            { "label": "1080p", "size": 1_200_000_000u64, "format": { "Video": { "max_height": 1080 } } },
            { "label": "720p", "size": null, "format": { "Video": { "max_height": 720 } } },
            { "label": "Audio only (MP3)", "size": 9_000_000u64, "format": "AudioMp3" }
        ])
    }

    fn run(k: &mut Fake, name: &str, args: Value) -> Result<String, String> {
        call(name, &args, k).expect("a known tool")
    }

    #[test]
    fn a_video_downloads_in_the_best_quality_up_to_the_one_asked() {
        let mut k = Fake { probe: Some(Ok(video(options()))), ..Default::default() };
        let text = run(&mut k, "download", json!({ "url": "https://vimeo.com/1", "quality": "1440p" })).unwrap();
        let (path, body) = k.sent.last().unwrap();
        assert_eq!(path, "/add-media");
        assert_eq!(body["format"], json!({ "Video": { "max_height": 1080 } }), "1440p isn't offered: the best below it");
        assert_eq!(body["title"], "Big Buck Bunny");
        assert!(text.contains("\"Big Buck Bunny\" as 1080p, about 1.1 GB"), "{text}");

        let mut k = Fake { probe: Some(Ok(video(options()))), ..Default::default() };
        run(&mut k, "download", json!({ "url": "https://vimeo.com/1" })).unwrap();
        assert_eq!(k.sent.last().unwrap().1["format"], json!({ "Video": { "max_height": 2160 } }), "best by default");

        let mut k = Fake { probe: Some(Ok(video(options()))), ..Default::default() };
        run(&mut k, "download", json!({ "url": "https://vimeo.com/1", "quality": "240p" })).unwrap();
        assert_eq!(k.sent.last().unwrap().1["format"], json!({ "Video": { "max_height": 720 } }), "nothing that small: the smallest there is");
    }

    #[test]
    fn audio_is_the_mp3_option() {
        let mut k = Fake { probe: Some(Ok(video(options()))), ..Default::default() };
        let text = run(&mut k, "download", json!({ "url": "https://vimeo.com/1", "quality": "audio" })).unwrap();
        assert_eq!(k.sent.last().unwrap().1["format"], "AudioMp3");
        assert!(text.contains("Audio only (MP3)"), "{text}");
        let mut k = Fake { probe: Some(Ok(video(json!([{ "label": "720p", "format": { "Video": { "max_height": 720 } } }])))), ..Default::default() };
        let err = run(&mut k, "download", json!({ "url": "https://vimeo.com/1", "quality": "mp3" })).unwrap_err();
        assert!(err.contains("no audio version"), "{err}");
    }

    #[test]
    fn a_playlist_adds_each_video_or_the_ones_picked() {
        let entries: Vec<Value> = (1..=4).map(|n| json!({ "url": format!("https://www.youtube.com/watch?v={n}"), "title": format!("Part {n}"), "duration": 60.0 })).collect();
        let info = json!({ "title": "Course", "playlist": 4, "entries": entries, "options": options() });
        let mut k = Fake { probe: Some(Ok(info.clone())), ..Default::default() };
        let text = run(&mut k, "download", json!({ "url": "https://www.youtube.com/playlist?list=x", "quality": "720p" })).unwrap();
        let adds: Vec<&Value> = k.sent.iter().filter(|(p, _)| p == "/add-media").map(|(_, b)| b).collect();
        assert_eq!(adds.len(), 4);
        assert_eq!(adds[2]["url"], "https://www.youtube.com/watch?v=3");
        assert_eq!(adds[2]["title"], "Part 3");
        assert_eq!(adds[2]["format"], json!({ "Video": { "max_height": 720 } }));
        assert!(text.starts_with("Katch is downloading 4 of the 4 videos in \"Course\" at up to 720p"), "{text}");

        let mut k = Fake { probe: Some(Ok(info.clone())), ..Default::default() };
        run(&mut k, "download", json!({ "url": "https://www.youtube.com/playlist?list=x", "videos": [2, 4], "quality": "audio" })).unwrap();
        let adds: Vec<&Value> = k.sent.iter().filter(|(p, _)| p == "/add-media").map(|(_, b)| b).collect();
        assert_eq!(adds.iter().map(|b| b["title"].as_str().unwrap()).collect::<Vec<_>>(), ["Part 2", "Part 4"]);
        assert_eq!(adds[0]["format"], "AudioMp3");

        let mut k = Fake { probe: Some(Ok(info)), ..Default::default() };
        let err = run(&mut k, "download", json!({ "url": "https://www.youtube.com/playlist?list=x", "videos": [9] })).unwrap_err();
        assert!(err.contains("has 4 videos; there is no video 9"), "{err}");
        assert!(!k.sent.iter().any(|(p, _)| p == "/add-media"), "nothing added when a position is wrong");
    }

    #[test]
    fn a_long_playlist_takes_the_first_hundred_unless_picked() {
        let entries: Vec<Value> = (1..=150).map(|n| json!({ "url": format!("https://v.example/{n}"), "title": format!("V{n}") })).collect();
        let mut k = Fake { probe: Some(Ok(json!({ "title": "Channel", "entries": entries, "options": [] }))), ..Default::default() };
        let text = run(&mut k, "download", json!({ "url": "https://www.youtube.com/@x/videos" })).unwrap();
        assert_eq!(k.sent.iter().filter(|(p, _)| p == "/add-media").count(), 100);
        assert!(text.contains("That's the first 100"), "{text}");
    }

    #[test]
    fn files_repositories_and_magnets_go_straight_to_katch() {
        for url in ["https://github.com/kuldeepsaini23/katch", "https://example.com/setup.exe", "magnet:?xt=urn:btih:abc"] {
            let mut k = Fake::default();
            let text = run(&mut k, "download", json!({ "url": url })).unwrap();
            assert_eq!(k.sent, [("/add".to_string(), json!({ "url": url }))], "no probe for {url}");
            assert!(text.contains("download #1"), "{text}");
        }
        let mut k = Fake::default();
        let text = run(&mut k, "download", json!({ "url": "https://github.com/a/b" })).unwrap();
        assert!(text.contains("as a ZIP"), "{text}");
    }

    #[test]
    fn a_page_without_a_video_says_so_and_can_be_saved_as_a_file() {
        let mut k = Fake { probe: Some(Err("Unsupported URL: https://site.example/get?id=3".into())), ..Default::default() };
        let err = run(&mut k, "download", json!({ "url": "https://site.example/get?id=3" })).unwrap_err();
        assert!(err.contains("Unsupported URL") && err.contains("as_file: true"), "{err}");
        let mut k = Fake::default();
        run(&mut k, "download", json!({ "url": "https://site.example/get?id=3", "as_file": true })).unwrap();
        assert_eq!(k.sent, [("/add".to_string(), json!({ "url": "https://site.example/get?id=3", "kind": "file" }))]);
    }

    #[test]
    fn katch_not_running_is_said_as_is_not_as_a_page_problem() {
        let mut k = Fake { unreachable: true, ..Default::default() };
        let err = run(&mut k, "download", json!({ "url": "https://vimeo.com/1" })).unwrap_err();
        assert_eq!(err, "Katch isn't running. Open Katch, then try again.");
        let mut k = Fake { unreachable: true, ..Default::default() };
        assert_eq!(run(&mut k, "inspect_link", json!({ "url": "https://vimeo.com/1" })).unwrap_err(), "Katch isn't running. Open Katch, then try again.");
    }

    #[test]
    fn bad_input_is_explained() {
        let mut k = Fake::default();
        assert!(run(&mut k, "download", json!({ "url": "file:///etc/passwd" })).unwrap_err().contains("http(s) and magnet"));
        assert!(run(&mut k, "download", json!({ "url": "https://vimeo.com/1", "quality": "ultra" })).unwrap_err().contains("use best, 2160p"));
        assert!(run(&mut k, "pause_download", json!({})).unwrap_err().contains("id"));
        assert!(k.sent.is_empty());
    }

    #[test]
    fn inspect_names_qualities_or_numbers_a_playlist() {
        let mut k = Fake { probe: Some(Ok(video(options()))), ..Default::default() };
        let text = run(&mut k, "inspect_link", json!({ "url": "https://vimeo.com/1" })).unwrap();
        assert!(text.starts_with("\"Big Buck Bunny\", 9:56\nQualities:"), "{text}");
        assert!(text.contains("- 2160p (about 2.8 GB)") && text.contains("- 720p\n") && text.contains("- Audio only (MP3)"), "{text}");
        assert!(!k.sent.iter().any(|(p, _)| p.starts_with("/add")), "reading never downloads");

        let entries: Vec<Value> = (1..=60).map(|n| json!({ "url": format!("https://v.example/{n}"), "title": format!("V{n}"), "duration": 3725.0 })).collect();
        let mut k = Fake { probe: Some(Ok(json!({ "title": "Talks", "entries": entries, "options": [] }))), ..Default::default() };
        let text = run(&mut k, "inspect_link", json!({ "url": "https://www.youtube.com/playlist?list=t" })).unwrap();
        assert!(text.starts_with("Playlist \"Talks\", 60 videos:\n1. V1 (1:02:05)\n"), "{text}");
        assert!(text.contains("50. V50") && !text.contains("51. V51") && text.contains("…and 10 more."), "{text}");

        let mut k = Fake::default();
        let text = run(&mut k, "inspect_link", json!({ "url": "https://github.com/a/b" })).unwrap();
        assert!(text.contains("GitHub repository") && k.sent.is_empty(), "{text}");
    }

    #[test]
    fn list_shows_progress_and_where_finished_files_are() {
        let list = json!({ "items": [
            { "id": 3, "name": "movie.mp4", "status": "running", "downloaded": 524288000u64, "total": 1048576000u64, "speed": 8808038u64, "error": null, "path": null },
            { "id": 2, "name": "song.mp3", "status": "failed", "downloaded": 0, "total": null, "speed": 0, "error": "HTTP 403", "path": null },
            { "id": 1, "name": "katch.zip", "status": "done", "downloaded": 7, "total": 7, "speed": 0, "error": null, "path": "C:\\Users\\me\\Downloads\\Katch\\katch.zip" }
        ]});
        let mut k = Fake { list: list.clone(), ..Default::default() };
        let text = run(&mut k, "list_downloads", json!({})).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "#3 running · 50% · 500.0 MB of 1000.0 MB · 8.4 MB/s · movie.mp4");
        assert_eq!(lines[1], "#2 failed · song.mp3 · error: HTTP 403");
        assert_eq!(lines[2], "#1 done · katch.zip · saved to C:\\Users\\me\\Downloads\\Katch\\katch.zip");
        let mut k = Fake { list, ..Default::default() };
        assert_eq!(run(&mut k, "list_downloads", json!({ "active_only": true, "limit": 1 })).unwrap(), lines[0]);
        let mut k = Fake { list: json!({ "items": [] }), ..Default::default() };
        assert_eq!(run(&mut k, "list_downloads", json!({})).unwrap(), "No downloads yet.");
    }

    #[test]
    fn pause_and_resume_send_the_id() {
        let mut k = Fake::default();
        assert_eq!(run(&mut k, "pause_download", json!({ "id": 12 })).unwrap(), "Download #12 paused.");
        assert_eq!(run(&mut k, "resume_download", json!({ "id": 12 })).unwrap(), "Download #12 resumed.");
        assert_eq!(k.sent[1], ("/control".to_string(), json!({ "id": 12, "action": "resume" })));
    }
}
