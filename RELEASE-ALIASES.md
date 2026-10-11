# Release files after the rename to Katch

Snag 1.1.3 and older update themselves from the GitHub release marked latest. They look for their
own platform's file **by its old name** and check it against `SHA256SUMS-<version>.txt`:

| Platform | What Snag ≤ 1.1.3 downloads | What Katch downloads (falls back to the Snag name) |
| -------- | --------------------------- | -------------------------------------------------- |
| Windows  | `Snag-Setup-<version>.exe`  | `Katch-Setup-<version>.exe`                        |
| Linux    | `Snag-x86_64.AppImage`      | `Katch-x86_64.AppImage`                            |
| macOS    | `Snag-macOS.zip`            | `Katch-macOS.zip`                                  |

So every release from now on carries each of those files twice: under Katch's name, and a
byte-identical copy under Snag's. The build scripts already make the copies; the release workflow
only has to upload them. A release without the copies breaks nothing, but Snag users then never
hear of it.

The copies are the same files, so they work as they are for the old apps: the Windows installer
keeps the AppId, the install folder and `snag.exe`, and updates Snag in place; the macOS zip holds
`Katch.app`, which the old updater puts where its `Snag.app` is (it takes whatever one `.app` the
zip holds); the AppImage replaces the old one under the old file's name.

## What `.github/workflows/release.yml` must upload

```yaml
  windows:   # after: bash installer/build.sh
    run: |
      version="${TAG#v}"
      ext_version=$(node -p "require('./extension/manifest.json').version")
      gh release upload "$TAG" \
        "target/installer/Katch-Setup-$version.exe" target/installer/Katch-Setup.exe \
        "target/installer/Snag-Setup-$version.exe" target/installer/Snag-Setup.exe \
        target/store/*-"$ext_version".zip --clobber

  linux:     # after: bash packaging/linux/build.sh
    run: |
      gh release upload "$TAG" target/linux/Katch-x86_64.AppImage target/linux/Snag-x86_64.AppImage \
        "target/linux/katch_${TAG#v}_amd64.deb" --clobber

  macos:     # after: bash packaging/macos/build.sh
    run: |
      gh release upload "$TAG" target/macos/Katch-macOS.zip target/macos/Katch-macOS.dmg \
        target/macos/Snag-macOS.zip --clobber
```

The `checksums` job stays as it is: `sha256sum -- *` over every file of the release lists the
copies too, which is what the old apps look up. It must keep running after all three uploads.

Every file of such a release:

```
Katch-Setup-<version>.exe     Snag-Setup-<version>.exe   (copy, for Snag's updater)
Katch-Setup.exe               Snag-Setup.exe             (copy, for old links to latest/download)
Katch-x86_64.AppImage         Snag-x86_64.AppImage       (copy, for Snag's updater)
katch_<version>_amd64.deb
Katch-macOS.zip               Snag-macOS.zip             (copy, for Snag's updater)
Katch-macOS.dmg
katch-chrome-<ext>.zip + the Firefox package (named by the extension's manifest)
SHA256SUMS-<version>.txt      (lists all of the above)
```

Also rename in release.yml's comments and step names ("Build Snag.app…" → "Build Katch.app…").

## The repository rename

- Katch asks `api.github.com/repos/kuldeepsaini23/katch/releases/latest`: rename the repository
  to `katch` **before** publishing the first Katch release, or Katch's update check finds nothing
  until it is renamed.
- Snag ≤ 1.1.3 asks `…/repos/kuldeepsaini23/snag/releases/latest`; GitHub answers a renamed
  repository with a redirect, which the updater follows. Never create a new repository called
  `snag` on this account: that ends the redirect, and old copies stop finding updates.

## How long

Keep the copies until hardly anyone runs Snag 1.1.3 or older: Snag checks every hour while it
runs, so most installs move within days, but a PC that's off for months only checks when it next
starts. A few months, or the next several releases, is safe. The `.deb` has no copy: no updater
installs it (`katch` replaces the `snag` package: Conflicts/Replaces/Provides: snag).
