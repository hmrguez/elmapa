# Use Elmapa from Codex

Build Elmapa with the [native development prerequisites](../CONTRIBUTING.md#build-prerequisites). Then register its local MCP server with the absolute path to the binary and a folder for generated atlases. On macOS and Linux, run the following in the project root.

```sh
cargo build --release --locked
codex mcp add elmapa -- "$PWD/target/release/elmapa" --mcp "$HOME/Documents/Elmapa Maps"
```

On Windows, use PowerShell from the project root:

```powershell
cargo build --release --locked
codex mcp add elmapa -- "$PWD\target\release\elmapa.exe" --mcp "$env:USERPROFILE\Documents\Elmapa Maps"
```

In the desktop app, you can also add a STDIO server under **Settings → MCP
servers**. Use the built executable as the command and pass two arguments,
`--mcp` and the output directory. Restart Codex after adding the server.

Try “Use Elmapa to create a parchment map
of a crescent island with an inland mountain ridge and two settlements connected
by a trail. Preview it, move the north settlement closer to the bay, then export
an SVG.”

- `create_map` saves an editable atlas in one call from named areas, strokes,
  landmarks and labels. Each feature is a native layer. Plans draw bottom to top
  and use world coordinates with x right and y down. The default planning canvas
  is 1600 × 1000. An area takes an outline and a stroke takes waypoints, including
  the names of earlier features. Rivers and trails start with the native editor’s
  brush size and strength. Use `size` to change brush size in world units. Set a
  river stroke’s `material` to a Water finish such as `lake` to match its channel
  color. A label accepts optional clockwise `rotation` in degrees from −180 to 180.
- `inspect_map` returns layer IDs and anchors. Omit the map ID to discover
  materials and landmark assets.
- `preview_map` returns a PNG of the full map or an area around a chosen layer.
  `edit_map` applies a batch to the expected revision. To reroute after a move,
  replace the trail and reference the updated feature names.
- `export_map` writes a self-contained SVG for the current revision. Open the
  returned `.elmapa` file via **Open atlas…** to continue working in Elmapa.
  The server manages files in its output folder. Close or reopen a native editing
  session to pick up external revisions, and avoid saving over the same file from
  the app while Codex is editing it.
## Choose how an area is painted

An area can include a `drawing` object. The default `{"approach":"scanline"}`
paints horizontal bands from top to bottom. Use `breadth_first` to begin near a
point inside the smoothed outline and paint neighboring spans in waves.

```json
{
  "name": "Atlantic",
  "art": {
    "kind": "area",
    "brush": "water",
    "material": "ocean",
    "outline": [[0, 0], [1600, 0], [1600, 1000], [0, 1000]],
    "drawing": {"approach": "breadth_first", "from": [420, 560]}
  }
}
```

Pass the feature to `create_map` or `edit_map` with a `put` change. Coordinates
use world units. Starting points outside the smoothed outline are rejected.
When an outline has more than one paintable piece, the closest remaining piece
starts its own wave. Each area stays on one editable layer. Elmapa records the
brush stroke order for **Replay this view**. Breadth-first land and water use
one continuous wash across tile edges. The wash spreads by distance from
the first stroke through the painted area.

For a hand-drawn reveal, supply `brush_paths` on a land or water area. Each
path is a separate gesture with the same brush radius in world units. List
gesture points in the order the brush should travel. The outline clips the
artwork, so the brush can sweep across a coast without changing its shape.
Portions of the area outside the gestures fade in at the end.

```json
"drawing": {
  "approach": "brush_paths",
  "radius": 100,
  "paths": [
    [[190, 140], [250, 90], [340, 125], [370, 200], [305, 250], [220, 230]],
    [[330, 270], [410, 320], [485, 280]]
  ]
}
```
