# Release builds

Each successful push to `dev` publishes a preview on GitHub. The title uses the version in `Cargo.toml`, the UTC date the CI run was created and the build number. For example, a preview can be titled `Elmapa v0.2.0 Preview (2026-11-19, build 42)`. Regular contributions can share the same Cargo version across many previews.

A successful promotion to `main` publishes a stable release titled with the version alone, such as `v0.2.0`. CI allows one stable release per Cargo version, including versions shipped under the older release titles. A tag such as `dev-42-a1b2c3d4e5f6` identifies the channel, CI run number and source commit. The newest successful main run owns GitHub’s **Latest** label, even if two builds finish out of order. Find earlier builds by tag on the [releases page](https://github.com/hmrguez/elmapa-public/releases).

| Download | Build target | Contents |
| --- | --- | --- |
| Linux `.tar.gz` | Ubuntu 24.04, x86-64 | `Elmapa/elmapa` |
| macOS `.zip` | macOS 15, Apple Silicon | `Elmapa/Elmapa.app` |
| Windows `.zip` | Windows, x86-64 MSVC | `Elmapa/elmapa.exe` |

Extract the archive before launching Elmapa. On Ubuntu the editor needs an X11 or XWayland desktop, a Vulkan driver and the X11 keyboard library (`sudo apt install libxkbcommon-x11-0`). The macOS build uses ad hoc signing without Apple notarization. The Windows executable is unsigned. Each archive includes the project license, asset notices and Rust dependency licenses for its target. On macOS these notices travel with the application inside `Elmapa.app/Contents/Resources`.

## Package locally

From a source checkout with the platform’s Rust build prerequisites installed, use Python 3.12 or newer and cargo-about 0.9.3. Install cargo-about and build the package from the repository root.

```sh
cargo install cargo-about --locked --version 0.9.3 --features cli
make package
```

On Windows, run `python scripts/release/package.py` directly from a developer shell. Packaging uses the host toolchain. It builds the locked release profile, collects notices, extracts the finished archive and runs the enclosed executable to export a demo SVG. On macOS it also verifies the extracted app signature. The verified archive appears under `target/packages/`. Once the dependencies are cached, `make package PACKAGE_FLAGS=--offline` works without network access. Online builds can retrieve missing copyright notices from dependency repositories.

To check the native workflow, launch the extracted app, create an atlas, paint a stroke and add a label. Save, close, reopen the atlas and export it to SVG. Check that the artwork and label appear both in the reopened atlas and in the export. For an automated desktop check, run `make package PACKAGE_FLAGS=--native` with a graphical session available. It uses two fresh launches with an isolated library, paints a trail, saves and quits, then reopens the atlas and compares the two SVG exports. File paths are supplied by the replay. CI also runs this check using Xvfb and Mesa on Linux, WARP on Windows, and on macOS runners that expose a Metal display. The headless export check runs on all three systems.

## Prepare and promote a stable release

Maintainers using Codex can invoke the [release skill](../.agents/skills/release/SKILL.md) to prepare a package or follow a channel run.

1. Open a release PR against `dev`. Choose a Cargo version that has never shipped as stable. Update `Cargo.toml`, run `cargo update --workspace` to refresh `Cargo.lock`, and include user-facing release notes in the PR.
2. Merge the release PR and wait for its `dev` preview to pass CI. Review the preview and select the exact commit to promote.
3. Fast-forward local `main` to that verified commit. Review the commit and push destination, then run `make release CHANNEL=main` from the `main` checkout.
4. Verify the stable release on GitHub and copy the release notes from the PR into its description.

CI reuses the three archives from the successful `dev` push when they are still available. It relabels the filenames and publishes their checksums. The archive contents are byte-for-byte identical. A different commit, a failed preview or expired artifacts trigger the full platform matrix. Actions retains these artifacts for seven days.

Merging a pull request into upstream `dev` triggers preview publication. To publish from the local `dev` checkout, use `make release CHANNEL=dev`. Both release commands push the committed tip of the checked-out channel branch to its matching remote branch. They use `origin` by default and accept `REMOTE=public` for a remote named `public`. CI creates the release tag. Archive filenames include the Cargo version, the channel release tag and the Rust target triple.

Follow the commit in the [Actions runs](https://github.com/hmrguez/elmapa-public/actions/workflows/ci.yml). CI runs for pull requests and pushes to `dev` or `main`. Pull request and channel push runs build the same archives. On a channel push the publication job waits for all checks, downloads the three artifacts, writes `SHA256SUMS`, uploads to a draft and publishes it. Stable publication rejects a version already published on a different run. Only publication and the serialized stable-label update receive `contents: write` through the repository’s automatic `GITHUB_TOKEN`. Enable Actions and allow the workflow that permission in the destination repository.

For a transient failure, rerun failed jobs on the original push. A retry can finish a draft with a partial upload. A published release for that run is complete. If a fix changes source or packaging after a stable release, prepare a release PR with a new version.

Download `SHA256SUMS` beside the archive. Compare the checksum line for your platform against the output of `sha256sum <archive>` on Linux, `shasum -a 256 <archive>` on macOS or `Get-FileHash <archive> -Algorithm SHA256` in PowerShell.
