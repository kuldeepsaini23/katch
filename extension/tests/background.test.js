// background.js in a bare VM context with a fake `chrome` and `fetch`: what it sends to Snag when
// a tab's stream is downloaded, with the subtitle files the tab's player loaded; and that nothing
// is recorded or offered on a site Snag is turned off on.
const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const event = () => {
  const fns = [];
  return { addListener: (fn) => fns.push(fn), fire: (...args) => fns.map((fn) => fn(...args)) };
};

/** `tabUrls`: tab id -> the page it shows. `sync`: what chrome.storage.sync holds to start with. */
function load({ tracks = [], tabUrls = {}, sync = {} } = {}) {
  const sent = [];
  const session = {};
  const badges = {};
  const storageChanged = event();
  const chrome = {
    storage: {
      local: { get: async (keys) => (keys.includes("token") ? { token: "t" } : {}), set: async () => {}, remove: async () => {} },
      sync: {
        get: async (keys) => Object.fromEntries(keys.filter((k) => k in sync).map((k) => [k, sync[k]])),
        set: async (o) => {
          const changes = Object.fromEntries(Object.entries(o).map(([k, v]) => [k, { oldValue: sync[k], newValue: v }]));
          Object.assign(sync, o);
          storageChanged.fire(changes, "sync");
        },
      },
      onChanged: storageChanged,
      session: {
        get: async (key) => (key in session ? { [key]: session[key] } : {}),
        set: async (o) => Object.assign(session, o),
        remove: async () => {},
      },
    },
    // The badge text per tab ("global" when it has none).
    action: { setBadgeBackgroundColor() {}, setBadgeText: ({ tabId, text }) => (badges[tabId ?? "global"] = text), setTitle() {} },
    alarms: { create() {}, onAlarm: event() },
    runtime: { onStartup: event(), onInstalled: event(), onMessage: event() },
    contextMenus: { create() {}, onClicked: event() },
    downloads: { onCreated: event(), cancel: async () => {}, erase: async () => {}, download: async () => {} },
    cookies: { getAll: async () => [] },
    webRequest: { onResponseStarted: event() },
    tabs: {
      onUpdated: event(),
      onRemoved: event(),
      get: async (id) => ({ id, url: tabUrls[id] || "https://site.example/watch/1" }),
      query: async () => Object.entries(tabUrls).map(([id, url]) => ({ id: Number(id), url })),
      // The page's content script answers "track-list" with its <track>s.
      sendMessage: async (_tab, msg) => (msg.type === "track-list" ? tracks : undefined),
    },
  };
  const fetch = async (url, opts = {}) => {
    const route = new URL(url).pathname;
    if (route === "/ping") return { ok: true, json: async () => ({ app: "rdm", paired: true }) };
    sent.push({ route, body: JSON.parse(opts.body) });
    return { ok: true, status: 200, json: async () => ({ id: 1 }) };
  };
  const ctx = vm.createContext({
    chrome,
    fetch,
    console,
    URL,
    AbortSignal,
    // Timers never keep the test run alive.
    setTimeout: (fn, ms) => setTimeout(fn, ms).unref(),
    clearTimeout,
  });
  ctx.importScripts = (...files) => files.forEach((f) => vm.runInContext(fs.readFileSync(path.join(__dirname, "..", f), "utf8"), ctx, { filename: f }));
  vm.runInContext(fs.readFileSync(path.join(__dirname, "..", "background.js"), "utf8"), ctx, { filename: "background.js" });
  const response = (tabId, url, contentType = "", statusCode = 200) =>
    chrome.webRequest.onResponseStarted.fire({ tabId, url, statusCode, responseHeaders: [{ name: "Content-Type", value: contentType }], documentUrl: "https://player.example/e/42" });
  const message = (msg, sender = {}) => new Promise((resolve) => chrome.runtime.onMessage.fire(msg, sender, resolve));
  return { sent, response, message, chrome, badges };
}

// Objects made inside the VM have its prototypes: compare them as plain data.
const plain = (o) => JSON.parse(JSON.stringify(o));
const settle = () => new Promise((r) => setTimeout(r, 20));
const STREAM = "https://cdn.example/hls/master.m3u8?sig=1";

test("the in-page button sends a stream with the subtitles its player loaded", async () => {
  const bg = load({ tracks: [{ url: "https://cdn.example/subs/sintel_de.vtt", lang: "de", label: "Deutsch" }] });
  bg.response(7, STREAM, "application/vnd.apple.mpegurl");
  bg.response(7, "https://cdn.example/subs/sintel_eng.vtt?exp=1", "text/vtt");
  bg.response(7, "https://cdn.example/subs/gone.vtt", "text/html", 404);
  bg.response(7, "https://cdn.example/subs/eng/seg-3.vtt", "text/vtt");
  bg.response(8, "https://other.example/subs/fr.vtt", "text/vtt");
  await settle();
  const choice = { url: STREAM, title: "Sintel", format: { Video: { max_height: 4320 } }, referrer: "https://player.example/e/42" };
  const r = await bg.message({ type: "add-media", choice }, { tab: { id: 7 } });
  assert.strictEqual(r.ok, true);
  assert.strictEqual(bg.sent.length, 1);
  const { route, body } = bg.sent[0];
  assert.strictEqual(route, "/add-media");
  assert.strictEqual(body.url, STREAM);
  assert.strictEqual(body.referrer, "https://player.example/e/42");
  assert.deepStrictEqual(body.subtitles, [
    { url: "https://cdn.example/subs/sintel_de.vtt", lang: "de", label: "Deutsch" },
    { url: "https://cdn.example/subs/sintel_eng.vtt?exp=1", lang: "en" },
  ]);
});

test("the popup's Download sends the tab's subtitles too; other links go without", async () => {
  const bg = load();
  bg.response(3, STREAM, "application/vnd.apple.mpegurl");
  bg.response(3, "https://cdn.example/subs/English.srt", "application/x-subrip");
  await settle();
  // The popup has no tab of its own: it names the tab.
  await bg.message({ type: "send", url: STREAM, kind: "media", referrer: "https://site.example/watch/1", tabId: 3 });
  assert.deepStrictEqual(bg.sent[0].body.subtitles, [{ url: "https://cdn.example/subs/English.srt", lang: "en" }]);
  // A link the tab never played (a page, a file elsewhere): no subtitles field at all.
  await bg.message({ type: "send", url: "https://files.example/setup.exe", kind: "file", referrer: "https://site.example/", tabId: 3 });
  assert.ok(!("subtitles" in bg.sent[1].body), JSON.stringify(bg.sent[1].body));
  await bg.message({ type: "add-media", choice: { url: "https://site.example/watch/1", title: "x", format: { AudioMp3: null } } }, { tab: { id: 3 } });
  assert.ok(!("subtitles" in bg.sent[2].body));
});

test("a stream with no subtitles is sent exactly as before", async () => {
  const bg = load();
  bg.response(5, STREAM, "application/vnd.apple.mpegurl");
  await settle();
  await bg.message({ type: "add-media", choice: { url: STREAM, title: "Clip", format: { Video: { max_height: 4320 } } } }, { tab: { id: 5 } });
  assert.deepStrictEqual(Object.keys(bg.sent[0].body).sort(), ["cookies", "format", "title", "url"]);
});

test("on a site Snag is off on, the tab's media and subtitles aren't recorded or offered", async () => {
  const bg = load({ tabUrls: { 4: "https://www.youtube.com/watch?v=1", 6: "https://vimeo.com/1" }, sync: { offSites: ["youtube.com"] } });
  bg.response(4, STREAM, "application/vnd.apple.mpegurl");
  bg.response(4, "https://cdn.example/subs/en.vtt", "text/vtt");
  bg.response(6, STREAM, "application/vnd.apple.mpegurl");
  await settle();
  const off = await bg.message({ type: "media-list" }, { tab: { id: 4 } });
  assert.deepStrictEqual(plain(off), { items: [], off: true });
  assert.strictEqual((await bg.message({ type: "media-list" }, { tab: { id: 6 } })).items.length, 1, "other sites still work");

  // Back on (popup or options page): the switch works without a reload, from the next request.
  await bg.chrome.storage.sync.set({ offSites: [] });
  await settle();
  assert.strictEqual((await bg.message({ type: "media-list" }, { tab: { id: 4 } })).items.length, 0, "what played while off was never recorded");
  bg.response(4, STREAM, "application/vnd.apple.mpegurl");
  await settle();
  assert.strictEqual((await bg.message({ type: "media-list" }, { tab: { id: 4 } })).items.length, 1);
});

test("turned off after the stream played: it isn't offered, and its subtitles don't go along", async () => {
  const bg = load({ tabUrls: { 4: "https://m.youtube.com/watch?v=1" } });
  bg.response(4, STREAM, "application/vnd.apple.mpegurl");
  bg.response(4, "https://cdn.example/subs/en.vtt", "text/vtt");
  await settle();
  await bg.chrome.storage.sync.set({ offSites: ["youtube.com"] });
  await settle();
  assert.deepStrictEqual(plain(await bg.message({ type: "media-list" }, { tab: { id: 4 } })), { items: [], off: true });
  assert.strictEqual(bg.badges[4], "off", "the tab's badge says so");
  // The popup's explicit Download still sends, just without anything the page played.
  await bg.message({ type: "send", url: STREAM, kind: "media", referrer: "https://m.youtube.com/watch?v=1", tabId: 4 });
  assert.ok(!("subtitles" in bg.sent[0].body));
});

test("paused everywhere: nothing is recorded on any site, and the toolbar badge says off", async () => {
  const bg = load({ tabUrls: { 2: "https://vimeo.com/1" }, sync: { paused: true } });
  bg.response(2, STREAM, "application/vnd.apple.mpegurl");
  await settle();
  assert.deepStrictEqual(plain(await bg.message({ type: "media-list" }, { tab: { id: 2 } })), { items: [], off: true });
  assert.strictEqual(bg.badges.global, "off");

  await bg.chrome.storage.sync.set({ paused: false });
  await settle();
  assert.strictEqual(bg.badges.global, "", "nothing waiting: no badge");
  assert.strictEqual(bg.badges[2], "", "the tab's badge is its (empty) count again");
});

test("downloads a page starts on a site Snag is off on stay in the browser", async () => {
  const bg = load({ sync: { offSites: ["files.example"] } });
  const fire = (item) => Promise.all(bg.chrome.downloads.onCreated.fire({ id: 1, fileSize: 50e6, totalBytes: 50e6, mime: "application/zip", state: "in_progress", startTime: new Date().toISOString(), ...item }));
  await fire({ url: "https://cdn.example/a.zip", referrer: "https://files.example/get" });
  assert.deepStrictEqual(bg.sent, [], "off on the page's site");
  await fire({ url: "https://cdn.example/b.zip", referrer: "https://other.example/get" });
  assert.deepStrictEqual(bg.sent.map((s) => s.body.url), ["https://cdn.example/b.zip"], "on elsewhere");
});
