// The options page: the global pause switch and the sites Snag is off on (site-toggle.js).
const $ = (id) => document.getElementById(id);

async function render() {
  const s = await loadSiteToggle(chrome.storage);
  $("pause-switch").setAttribute("aria-checked", String(s.paused));
  const list = $("sites");
  list.textContent = "";
  $("empty").hidden = s.offSites.length > 0;
  for (const site of s.offSites) {
    const item = document.createElement("li");
    const name = document.createElement("span");
    name.textContent = site;
    const on = document.createElement("button");
    on.textContent = "Turn on";
    on.setAttribute("aria-label", `Turn Snag on for ${site}`);
    on.addEventListener("click", async () => {
      const now = await loadSiteToggle(chrome.storage);
      await saveSiteToggle(chrome.storage, { offSites: now.offSites.filter((x) => x !== site) });
    });
    item.append(name, on);
    list.append(item);
  }
}

$("pause-switch").addEventListener("click", async () => {
  const s = await loadSiteToggle(chrome.storage);
  await saveSiteToggle(chrome.storage, { paused: !s.paused });
});

// Changed here, in the popup or in another browser (sync): always show what's saved.
chrome.storage.onChanged.addListener((changes) => {
  if (SITE_TOGGLE_KEYS.some((k) => k in changes)) render();
});

render();
