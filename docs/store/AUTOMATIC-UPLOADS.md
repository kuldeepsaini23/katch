# Automatic store uploads

Publishing a GitHub release sends the extension to the Chrome Web Store, Firefox Add-ons and Microsoft Edge Add-ons (`.github/workflows/stores.yml`, run by `release.yml` after the Windows job attaches the store packages).

- A store is used only when all of its secrets below are set. Without them it is skipped with a notice.
- It only uploads when `extension/manifest.json`'s version differs from the previous release tag's. The stores refuse a version they already have.
- Each upload goes to the store's review. Nothing appears in the store until the review passes.
- A store that fails shows as **failed** in the run's summary (Actions → the release run → Summary). The app release itself is never affected.
- To run it by hand: Actions → **Store uploads** → Run workflow → enter the tag (for example `v1.1.4`). Tick **force** to upload even though the extension version didn't change.

The extension has to be in each store already, published once by hand. The APIs only update an existing item.

## Adding a secret to GitHub

1. Open https://github.com/kuldeepsaini23/snag/settings/secrets/actions (Settings → Secrets and variables → Actions).
2. Click **New repository secret**.
3. Enter the name exactly as written below, paste the value, and click **Add secret**.

Or from a terminal: `gh secret set CWS_CLIENT_ID --repo kuldeepsaini23/snag` (it asks for the value).

## Chrome Web Store

Secrets: `CWS_CLIENT_ID`, `CWS_CLIENT_SECRET`, `CWS_REFRESH_TOKEN`, `CWS_PUBLISHER_ID`, `CWS_EXTENSION_ID`.

The workflow uses version 2 of the Chrome Web Store API, which needs the publisher ID as well as the extension ID.

1. Make sure 2-Step Verification is on for the Google account that owns the extension. The API refuses to publish without it.
2. Open https://console.cloud.google.com/ and create a project (for example "Snag store uploads").
3. Go to **APIs & Services → Library**, search for **Chrome Web Store API**, and click **Enable**.
4. Go to **APIs & Services → OAuth consent screen** (Google Auth Platform).
   1. Choose **External**, enter an app name ("Snag uploads") and your email, and save.
   2. Under **Audience**, add the Google account that owns the extension as a **Test user**.
   3. Click **Publish app** so the status reads **In production**. While the app is in "Testing", Google expires the refresh token after 7 days and uploads stop working. Google doesn't need to verify an app that only you use. You'll see a "Google hasn't verified this app" warning in step 7: click **Advanced → Go to Snag uploads**.
5. Go to **APIs & Services → Credentials → Create credentials → OAuth client ID**.
   1. Application type: **Web application**.
   2. Under **Authorized redirect URIs**, add `https://developers.google.com/oauthplayground`.
   3. Click **Create**, then copy the **Client ID** into the `CWS_CLIENT_ID` secret and the **Client secret** into `CWS_CLIENT_SECRET`.
6. Open https://developers.google.com/oauthplayground.
   1. Click the gear icon (top right), tick **Use your own OAuth credentials**, and paste the client ID and client secret.
   2. In the box under the API list (left), type `https://www.googleapis.com/auth/chromewebstore` and click **Authorize APIs**.
7. Sign in with the Google account that **owns the extension**, which may differ from the account that made the Cloud project, and allow access.
8. Click **Exchange authorization code for tokens**, then copy the **Refresh token** into `CWS_REFRESH_TOKEN`.
9. Open the Developer Dashboard at https://chrome.google.com/webstore/devconsole.
   1. **Publisher ID**: go to **Account** (or **Publisher → Settings**) and copy the publisher ID into `CWS_PUBLISHER_ID`.
   2. **Extension ID**: open the Snag item and copy its 32-letter ID (it is also in the item's address) into `CWS_EXTENSION_ID`.

If you later change the item's visibility in the dashboard, publish once by hand. Until then, API publishing fails.

## Firefox Add-ons (AMO)

Secrets: `AMO_JWT_ISSUER`, `AMO_JWT_SECRET`.

1. Sign in at https://addons.mozilla.org/developers/ with the account that owns the add-on.
2. Open **Tools → Manage API Keys** (https://addons.mozilla.org/developers/addon/api/key/).
3. Click **Generate new credentials**. If credentials already exist, click **Revoke and regenerate credentials**, which stops the old ones.
4. Copy **JWT issuer** into `AMO_JWT_ISSUER` and **JWT secret** into `AMO_JWT_SECRET`. The secret is shown only once.

Notes:
- The upload uses `web-ext sign --channel=listed`, which submits the new version for Mozilla's review and doesn't wait for it.
- The extension is plain JavaScript with no bundler or minifier, so no separate source-code archive is needed. The reviewer note (that it needs the Snag app) goes with each version.

## Microsoft Edge Add-ons

Secrets: `EDGE_PRODUCT_ID`, `EDGE_CLIENT_ID`, `EDGE_API_KEY`.

1. Sign in to Partner Center at https://partner.microsoft.com/dashboard/microsoftedge/ with the account that published the extension.
2. Under **Microsoft Edge**, open **Publish API**.
3. If the page shows "enable the new experience", click **Enable**. This switches to API keys (v1.1).
4. Click **Create API credentials** and wait a few minutes.
5. Copy the **Client ID** into `EDGE_CLIENT_ID` and the **API key** into `EDGE_API_KEY`. Note the key's **Expiry** date. When it expires, create a new key here and update the secret.
6. Go to **Microsoft Edge → Overview** and open Snag. Copy the **Product ID** from **Extension identity** into `EDGE_PRODUCT_ID`. It is also the GUID in the address bar between `microsoftedge/` and `/packages`.

## Checking a run

After a release, open Actions → **Release** → the run, then the **stores / upload** job. Its summary lists each store as "submitted for review", "skipped (no … secret)" or **failed**. For a failure, the store step's log shows the store's own error message. Common causes are an expired key or token, or a version the store already has.
