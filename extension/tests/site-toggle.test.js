const test = require("node:test");
const assert = require("node:assert");
const { siteOf, offEntryFor, isEnabled, turnOff, turnOn, loadSiteToggle, saveSiteToggle } = require("../site-toggle.js");

test("a site is its host without www. or m.", () => {
  assert.strictEqual(siteOf("www.youtube.com"), "youtube.com");
  assert.strictEqual(siteOf("M.YouTube.com"), "youtube.com");
  assert.strictEqual(siteOf("music.youtube.com"), "music.youtube.com");
  assert.strictEqual(siteOf("example.org."), "example.org", "a trailing dot is the same host");
  assert.strictEqual(siteOf("mail.example.org"), "mail.example.org", "only a whole m. label goes");
  assert.strictEqual(siteOf(""), "");
  assert.strictEqual(siteOf(undefined), "");
});

test("off on a site covers its subdomains, nothing else", () => {
  const s = { offSites: ["youtube.com"], paused: false };
  assert.ok(!isEnabled("www.youtube.com", s));
  assert.ok(!isEnabled("m.youtube.com", s));
  assert.ok(!isEnabled("music.youtube.com", s));
  assert.ok(isEnabled("notyoutube.com", s), "a lookalike name isn't the same site");
  assert.ok(isEnabled("vimeo.com", s));
  assert.strictEqual(offEntryFor("music.youtube.com", s.offSites), "youtube.com");
  assert.strictEqual(offEntryFor("vimeo.com", s.offSites), undefined);
});

test("pausing turns it off everywhere; nothing saved means on", () => {
  assert.ok(!isEnabled("vimeo.com", { offSites: [], paused: true }));
  assert.ok(isEnabled("vimeo.com", undefined));
  assert.ok(isEnabled("vimeo.com", {}));
  assert.ok(isEnabled("", { offSites: ["youtube.com"] }), "a page with no host is never a listed site");
});

test("turning off and on keeps the list tidy", () => {
  assert.deepStrictEqual(turnOff([], "www.youtube.com"), ["youtube.com"]);
  assert.deepStrictEqual(turnOff(["youtube.com"], "m.youtube.com"), ["youtube.com"], "no duplicates");
  assert.deepStrictEqual(turnOff(["b.com"], "a.com"), ["a.com", "b.com"], "sorted");
  assert.deepStrictEqual(turnOff(undefined, ""), [], "no host, nothing added");
  assert.deepStrictEqual(turnOn(["a.com", "youtube.com"], "www.youtube.com"), ["a.com"]);
  assert.deepStrictEqual(turnOn(["youtube.com"], "music.youtube.com"), [], "the entry that covered it goes");
  assert.deepStrictEqual(turnOn(["music.youtube.com"], "youtube.com"), ["music.youtube.com"], "a subdomain's own entry stays");
});

/** A chrome.storage area in memory; `fail` makes its writes throw (quota, policy). */
function area({ fail = false } = {}) {
  const data = {};
  return {
    data,
    get: async (keys) => Object.fromEntries(keys.filter((k) => k in data).map((k) => [k, data[k]])),
    set: async (o) => {
      if (fail) throw new Error("QUOTA_BYTES_PER_ITEM quota exceeded");
      Object.assign(data, o);
    },
    remove: async (keys) => keys.forEach((k) => delete data[k]),
  };
}

test("settings go to sync storage", async () => {
  const storage = { sync: area(), local: area() };
  assert.deepStrictEqual(await loadSiteToggle(storage), { offSites: [], paused: false });
  await saveSiteToggle(storage, { offSites: ["youtube.com"] });
  await saveSiteToggle(storage, { paused: true });
  assert.deepStrictEqual(storage.sync.data, { offSites: ["youtube.com"], paused: true });
  assert.deepStrictEqual(storage.local.data, {});
  assert.deepStrictEqual(await loadSiteToggle(storage), { offSites: ["youtube.com"], paused: true });
});

test("without sync, or when sync refuses, they go to local storage", async () => {
  const none = { local: area() };
  await saveSiteToggle(none, { paused: true });
  assert.deepStrictEqual(await loadSiteToggle(none), { offSites: [], paused: true });

  const storage = { sync: area({ fail: true }), local: area() };
  storage.sync.data.offSites = ["old.com"];
  await saveSiteToggle(storage, { offSites: ["old.com", "new.com"] });
  assert.deepStrictEqual(storage.local.data.offSites, ["old.com", "new.com"]);
  assert.deepStrictEqual((await loadSiteToggle(storage)).offSites, ["old.com", "new.com"], "local's copy wins");
});

test("once sync works again, local's copy is dropped", async () => {
  const storage = { sync: area(), local: area() };
  storage.local.data.offSites = ["a.com"];
  await saveSiteToggle(storage, { paused: false });
  assert.deepStrictEqual(storage.sync.data, { offSites: ["a.com"], paused: false });
  assert.deepStrictEqual(storage.local.data, {});
});

test("junk in storage reads as the defaults", async () => {
  const storage = { sync: area(), local: area() };
  Object.assign(storage.sync.data, { offSites: "youtube.com", paused: "yes" });
  assert.deepStrictEqual(await loadSiteToggle(storage), { offSites: [], paused: false });
  storage.sync.data.offSites = ["a.com", 3, "", null];
  assert.deepStrictEqual((await loadSiteToggle(storage)).offSites, ["a.com"]);
});
