// Katch turned off on some sites, or paused everywhere. Pure helpers shared by background.js,
// content.js, the popup, the options page and the tests.
//
// Kept in chrome.storage.sync (it follows the user to their other browsers), or in
// chrome.storage.local when sync is missing or refuses the write (quota, policy):
//   offSites  string[]  sites Katch is off on, as siteOf() names them, sorted ("youtube.com")
//   paused    boolean   Katch paused everywhere
// A copy in local storage wins over sync: it is only written when sync failed, and removed as
// soon as a sync write works again.
// Off means: no Download button on the page, the media and subtitles the page loads aren't
// recorded or offered, and the browser keeps the downloads the page starts.

const SITE_TOGGLE_KEYS = ["offSites", "paused"];

/** A site as the user thinks of it: the host without "www." or "m." (m.youtube.com is youtube.com). */
function siteOf(hostname) {
  return (hostname || "").toLowerCase().replace(/\.$/, "").replace(/^(www|m)\./, "");
}

/** The entry of `offSites` that turns Katch off on `hostname` (the site or a parent of it), if any. */
function offEntryFor(hostname, offSites) {
  const host = siteOf(hostname);
  if (!host) return undefined;
  return (offSites || []).find((site) => host === site || host.endsWith(`.${site}`));
}

/** Whether Katch works on `hostname` with `settings` ({ offSites, paused }). */
function isEnabled(hostname, settings) {
  const s = settings || {};
  return !s.paused && !offEntryFor(hostname, s.offSites);
}

function turnOff(offSites, hostname) {
  const site = siteOf(hostname);
  return [...new Set([...(offSites || []), site])].filter(Boolean).sort();
}

/** Back on for `hostname`: drops every entry that covered it (youtube.com for music.youtube.com). */
function turnOn(offSites, hostname) {
  const host = siteOf(hostname);
  return (offSites || []).filter((site) => !(host === site || host.endsWith(`.${site}`)));
}

function tidy(raw) {
  const offSites = Array.isArray(raw.offSites) ? raw.offSites.filter((s) => typeof s === "string" && s) : [];
  return { offSites, paused: raw.paused === true };
}

/** The saved settings from `storage` (chrome.storage): local's copy if there is one, else sync's. */
async function loadSiteToggle(storage) {
  let local = {};
  try {
    local = await storage.local.get(SITE_TOGGLE_KEYS);
  } catch (_) {
    // No local storage either: defaults.
  }
  if (SITE_TOGGLE_KEYS.some((k) => k in local)) return tidy(local);
  try {
    if (storage.sync) return tidy(await storage.sync.get(SITE_TOGGLE_KEYS));
  } catch (_) {
    // Sync turned off by policy, or not there: defaults.
  }
  return tidy({});
}

/** Saves `patch` (some of offSites / paused) to sync, or to local when sync can't take it. */
async function saveSiteToggle(storage, patch) {
  const next = { ...(await loadSiteToggle(storage)), ...patch };
  try {
    if (!storage.sync) throw new Error("no sync storage");
    await storage.sync.set(next);
    await storage.local.remove(SITE_TOGGLE_KEYS).catch(() => {});
  } catch (_) {
    await storage.local.set(next);
  }
  return next;
}

if (typeof module !== "undefined") {
  module.exports = { SITE_TOGGLE_KEYS, siteOf, offEntryFor, isEnabled, turnOff, turnOn, loadSiteToggle, saveSiteToggle };
}
