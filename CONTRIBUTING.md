# Contributing to Elmapa

Start by building the editor with the [README quickstart](README.md#get-started). Open **The Stillwater Isles**, paint a stroke, undo it, and export an SVG. This exercises the same document and scene code used by the native editor and headless export.

## Build prerequisites

Install Rust 1.95 or newer. The development checks also use Python 3 and the Rust components installed by `rustup component add clippy rustfmt`.

- **macOS** requires the Xcode Command Line Tools. Run `xcode-select --install` if you have not installed them.
- **Windows** requires the MSVC Rust toolchain and Visual Studio Build Tools with the **Desktop development with C++** workload, including MSVC and the Windows SDK.
- **Linux** uses X11 and a graphics driver supported by wgpu. When building on Ubuntu 24.04, install the packages below. Run the editor in a desktop session with an X11 display or XWayland. Native file dialogs use the desktop portal.

```sh
sudo apt-get install build-essential pkg-config libwayland-dev libxkbcommon-dev libx11-dev libxi-dev libxrandr-dev libxcursor-dev fonts-dejavu-core
```

## Choose where to work

| Change | Start here | Verify |
| --- | --- | --- |
| A brush finish | [Material manifest and artwork formats](assets/brush-materials/README.md) | Select the swatch, paint a stroke, reopen the saved atlas, and compare the SVG. |
| A placeable landmark | [Landmark pack format](assets/landmarks/README.md) | Enable the pack in Settings, place an item, then inspect its normal and Reduced Motion appearance. |
| Document state or undo | [Document model](src/document), [editor operations](src/editor) | Exercise an edit, undo and redo, then round-trip a saved file. Preserve existing version 1 documents. |
| Rendering or interaction | [Scene generation](src/scene), [native application](src/app), [architecture guide](docs/architecture.md) | Exercise the changed workflow in the window. Compare canvas and exported geometry when artwork changes. |
| MCP tools | [Server](src/mcp), [stdio integration tests](tests/mcp_stdio.rs), [client setup](docs/mcp.md) | Run the integration tests and check the result with an isolated output directory. |

Browse [open issues](https://github.com/hmrguez/elmapa-public/issues) for work already discussed. For a large change or file-format proposal, describe a concrete use case before implementing it.

## Make a first change

1. Fork the repository. In your local clone, start a fresh worktree and branch from current upstream `main`. For example, after updating local `main`, run `git worktree add -b codex/new-water-finish ../elmapa-water-finish main`. Work inside that new directory.
2. To try an artwork contribution, create an opaque, tileable water texture and append its entry to the brush manifest. Use a new material ID so existing maps keep their appearance. Record the source and redistribution terms for any third-party artwork. The [material guide](assets/brush-materials/README.md) describes the fields and image requirements.
3. Rebuild the app to embed the new texture. Open a blank atlas, select **Water**, choose the new swatch under **Material**, and paint a stroke. Save and reopen the atlas, then export an SVG and inspect the same area. Use the same approach for other changes by choosing the shortest native workflow that exercises them.
4. Run the [validation commands](README.md#validation) and applicable checks from [CI](.github/workflows/ci.yml). Include a regression test when an existing test misses the changed behavior. For interactive changes, record the platform, steps, and result. A screenshot helps reviewers evaluate visual changes.
5. Commit the change, push the branch to your fork, and open a pull request against the upstream `dev` branch. Describe what a mapmaker or contributor can now do, how you verified it, and any checks that still need a different platform. Maintainers review changes on `dev` and promote them to `main` for stable releases. See the [release guide](docs/releases.md).

To keep test atlases separate from your own shelf, start Elmapa with `ELMAPA_LIBRARY_DIR` pointing to a new directory. For example, on macOS or Linux:

```sh
ELMAPA_LIBRARY_DIR="$(mktemp -d)" cargo run --release --locked
```

To create a local macOS app bundle, run `./scripts/bundle-macos.sh --release` and open `target/Elmapa.app`. This development bundle carries an ad-hoc signature.

## Working with Codex

The optional [iconpack skill](.agents/skills/iconpack/SKILL.md) covers artwork, manifests, placement and animation for landmark packs. The [brush-material skill](.agents/skills/brush-material/SKILL.md) covers texture and sprite finishes. For example, ask Codex to “use $brush-material to add a silty water finish.” Both workflows use the same asset guides, app and tests as a manual contribution. Repository agents also follow the [worktree instructions](AGENTS.md).

## Feedback and reports

Use an issue for reproducible bugs, usability feedback, or feature requests. Include the platform and an example atlas when you can share it. For sensitive details follow the [security policy](SECURITY.md). The [code of conduct](CODE_OF_CONDUCT.md) describes how we work together.
