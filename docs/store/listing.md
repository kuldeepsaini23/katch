# Katch extension — store listing kit

Copy-paste material for the Chrome Web Store, Microsoft Edge Add-ons and Firefox Add-ons. The extension was listed as "Snag — Download Manager" up to 1.1.x: the same listings are renamed (same item and add-on IDs), not new ones.

- **Packages:** `katch-chrome-<version>.zip` (Chrome, Edge) and the Firefox package web-ext names after the manifest's name (`katch_download_manager-<version>.zip` for "Katch Download Manager"), from `target/store/` after `bash installer/build.sh`, or attached to the GitHub release.
- **Privacy policy URL:** https://katch.kuldeepsaini.dev/privacy (the stores were first given https://github.com/kuldeepsaini23/snag/blob/master/PRIVACY.md; both stay in sync)
- **Homepage:** https://katch.kuldeepsaini.dev
- **Support:** https://github.com/kuldeepsaini23/katch/issues
- **Category:** Chrome: *Productivity → Tools*, Firefox: *Download Management*, Edge: *Productivity*
- **License:** MIT. **Source:** https://github.com/kuldeepsaini23/katch (plain JavaScript, no bundler or minifier: answer **No** to "does your add-on use minified/obfuscated code").

## Name (Chrome ≤ 45 characters)

Katch — Download Manager

## Short description (Chrome ≤ 132 characters, Firefox summary ≤ 250)

Send downloads, videos and pictures to the Katch app: pick a quality, catch what a page plays, download 8× faster.

(114 characters.)

## Full description

Katch is a free, open-source download manager for Windows, macOS and Linux. This extension connects your browser to it. (Katch was called Snag until version 1.1.3: same app, same extension, new name.)

What it does
• A Download button on video pages: pick 1080p, 720p or MP3, and Katch downloads it.
• Catches what the page's player is playing and starts it at once, even on sites other tools can't read.
• Hands your browser's downloads to Katch, which downloads big files over up to 8 connections and resumes after a pause or restart.
• Right-click a link, picture or page → Download with Katch. Save a whole page as one offline file.
• "Grab all": every link and picture on a page, filtered, in one go.
• Links sent while Katch is closed wait and go through when it opens.

Private by design
• It talks only to the Katch app on your own computer (127.0.0.1), and only after you click Connect and allow it in Katch.
• No account, no analytics, no ads, and nothing sent to the developer or anyone else.

Needs the Katch app for Windows, macOS or Linux (free): https://katch.kuldeepsaini.dev

## Single purpose (Chrome)

Send the user's downloads — files, videos, audio and pictures from the pages they visit — to the Katch download manager app running on the same computer.

## Permission justifications (Chrome "Privacy practices" tab; also useful for Firefox/Edge reviewers)

| Permission | Why |
|---|---|
| `downloads` | When "Catch downloads" is on, a download the browser starts is cancelled in the browser and handed to Katch instead (and given back to the browser if Katch isn't running). |
| `cookies` | When the user sends a download, the cookies of that one site go with it to Katch on the same computer, so files behind a login (or age checks) download as they do in the browser. Never read in the background, never sent anywhere else. |
| `webRequest` | Reads the address, type and size of media a tab's video player loads, to offer those streams in the Download menu ("what's playing"). Requests are only observed, never changed or blocked. |
| `scripting` | "Grab all from this page": runs a small script in the current tab, after the user clicks it, to list the page's links and pictures. |
| `contextMenus` | The right-click items "Download with Katch" and "Save this page with Katch". |
| `storage` | Remembers the connection code, the user's choices (button position, hidden sites, catch downloads) and links waiting for Katch to open. |
| `alarms` | Once a minute, hands links saved while Katch was closed to Katch when it's running again. |
| Host access `<all_urls>` | The Download button and stream detection work on any video site, and the site's cookies are needed when sending a download. |
| Host `http://127.0.0.1/*` | The only place the extension sends data: the Katch app on the same computer. |

**Remote code:** No. All code is in the package; nothing is downloaded or evaluated at run time.

## Data use disclosure (Chrome form)

- **Data handled:** *Web history* (addresses of the pages and media the user sends to Katch) and *Authentication information* (the site's cookies, only when sending a download).
- **Use:** only for the single purpose above, and only on the user's own computer (sent to 127.0.0.1).
- Check all three certifications: not sold to third parties; not used or transferred for purposes unrelated to the single purpose; not used for creditworthiness or lending.

## Firefox notes

- The manifest declares `data_collection_permissions: { required: ["none"] }`: nothing leaves the device.
- Firefox asks users to allow "Access your data for all websites" (Manifest V3 makes host access optional). Until they do, the Download button and stream detection stay off; the popup and right-click menu still work.
- Reviewer note to paste: "Needs the Katch desktop app (Windows, macOS or Linux, free, open source: https://github.com/kuldeepsaini23/katch/releases/latest) running on the same computer; the extension talks only to http://127.0.0.1:47321–47326. To test without the app, the popup shows 'Katch isn't running'."
