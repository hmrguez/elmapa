# Bevy runtime validation protocol

Use release builds and isolated libraries. Do not run fixtures against the normal user library.

## Required portable checks

```sh
python3 scripts/export-icons.py --check
python3 -B scripts/test_performance_trace.py
cargo fmt --all -- --check
cargo test --offline --locked
cargo clippy --offline --locked --all-targets -- -D warnings
cargo run --offline --locked --bin elmapa -- --export-demo /private/tmp/elmapa-demo.svg
cargo run --offline --locked --example runtime_work
```

Core geometry tests remain framework independent. Adapter coverage checks retained world-coordinate tessellation, exact specimen atlas pixels/cropping/pivots, camera anchoring, ordinary Bevy keyboard/pointer dispatch, panel occlusion, release over chrome, revision-safe asynchronous saving, checkpointing, camera/preferences, active-atlas history and atomic/cancelled export. These checks supplement displayed native validation, not replace it.

## Native capture

Prepare each fixture outside the measured interval. `--warm` is optional: large SVG-based thumbnail generation can hit usvg's node limit, even when native forest rendering is legal. Record that failure; prepare the same fixture without `--warm` for both renderers and allow background work to settle before measured phases. Do not shrink it silently.

```sh
cargo build --offline --locked --release --bin elmapa
cp target/release/elmapa /private/tmp/elmapa-measured
cargo run --offline --locked --release --example prepare_performance_library -- /private/tmp/elmapa-bevy-native 1 dense-forest 1 --warm
python3 scripts/prepare-native-test-app.py /private/tmp/elmapa-measured /private/tmp/ElmapaBevyTest.app \
  --library /private/tmp/elmapa-bevy-native --trace /private/tmp/elmapa-bevy-trace.json \
  --replay docs/forest-native-replay.json --name 'Elmapa Bevy Test'
```

Launch the fresh bundle through Launch Services/CUA, keep it foreground, and press **p**. The replay waits until preparation is ready, enables growth, settles workers, zooms, paints a sustained forest gesture, waits for completion, measures dense static rendering and quits through the checkpointing path. Its pointer events enter the ordinary Bevy input dispatcher and retain all movements between frames. It modifies only its isolated library. Poll the trace file for completion; do not relaunch a closed app by querying it through CUA. Reuse immutable executable copies, never overwrite a running Mach-O file.

Record hardware, RAM, OS, Rust/profile, native logical window size, physical scale, display refresh, power/thermal state and competing load. Stop compilers and other measured apps during captures. Perform three cold launches for repeated comparisons, then test larger fixture sizes explicitly. Compare final saved documents, not merely trace labels or tree counts. Retain raw traces, summaries, replay, binary hashes and any rejected runs.

```sh
python3 scripts/analyze-performance.py /private/tmp/elmapa-bevy-trace.json --output /private/tmp/elmapa-bevy-summary.json
python3 scripts/compare-renderers.py --gpui /private/tmp/gpui-1.json /private/tmp/gpui-2.json /private/tmp/gpui-3.json \
  --bevy /private/tmp/bevy-1.json /private/tmp/bevy-2.json /private/tmp/bevy-3.json --output /private/tmp/comparison.json
```

## Metric meanings

Only positive Metal drawable `presentedTime` values determine displayed-frame intervals/FPS. The optional process-local observer attaches a presentation callback and does not change submitted drawing commands. Callback observation can trail phase markers; allow a settling boundary. It is not input-to-photon timing.

`bevy_main_schedule` measures the main ECS schedule, excluding render-thread/GPU completion. `bevy_frame_interval` measures frame-loop cadence. Neither is displayed FPS or GPU execution. GPUI's old `metal_submit`/`gpu_execution` scopes have no renamed Bevy equivalent. Native drawable intervals are the common comparison. Tracing itself adds overhead; results are observed on this machine, not portable guarantees.

The trace is bounded to 131,072 events. Reject overflow or incomplete replays. `compare-renderers.py` also rejects phases with fewer than 100 actual presentations. Frame-count-driven replays use the same ordered inputs, but a faster renderer completes the gesture sooner. They compare the same work, not a fixed-duration pointer recording. Validate final document equality, and separately exercise ordinary native input.

`cpu_resident_bytes` and peak RSS cover native/runtime allocations as well as Rust. Allocation counters are cumulative gross allocations, not live memory. A 192 MiB atlas tier can overlap a prior tier during GPU retirement; payload sizes are not hard process-memory caps. Preview failures must not discard document data.

## Native workflow and visual review

Exercise shelf reflow/scrolling, book transition/reduced motion, all brushes, pan and pointer-anchored zoom, UI occlusion, release over panels, layers/visibility/order/opacity/deletion, undo/redo, light/dark appearance, external save/open, SVG export/cancellation, return/reentry and graceful close/quit. Verify outputs by reopening the files. Use the same camera and window geometry for old/new visual review.

Check Avenir Next/Baskerville text, small icon edges, logical panel widths, radii, line heights, translucency, two-layer shadows, gradients and clipping. Font hinting and gamma-space compositing are explicit design requirements; default linear compositing changes partially covered text/icon contrast. Confirm physical render size and UI device scale on Retina and ordinary displays when available. Different font rasterizers can still produce different glyph coverage; do not claim a zero-pixel image diff without that evidence.

The 60 Hz acceptance target is sustained presented interval p95 below 16.7 ms and p99 below 33.3 ms for the measured supported interactions. State results per workload and size. Averages alone cannot hide stutters, and a pass on one Mac is not a universal guarantee.
