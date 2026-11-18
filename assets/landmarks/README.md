# Atlas landmark packs

Original, project-authored vector illustrations for Elmapa. Historical scans are references only; no third-party artwork is bundled.

Landmarks are loaded from **JSON/SVG packs**. `build.rs` discovers bundled pack folders automatically and embeds their JSON/SVG files in the executable. [Elmapa Basic Landmarks](packs/elmapa-basic/manifest.json) uses sepia engraving and parchment washes. [Brillanto's Brilliant Pack](packs/brillanto/README.md) shares that palette and uses flatter planes and geometric silhouettes inspired by wayfinding graphics.

To install another pack, put its folder, containing `manifest.json` and the SVG files, in the `landmark-packs/` directory of your [local atlas library](../../docs/user-guide.md) and restart Elmapa. With `ELMAPA_LIBRARY_DIR`, use that library's `landmark-packs/` directory instead. Packs are loaded once, including their validated, tessellated geometry. Invalid packs appear as unavailable in Settings and do not prevent valid packs from loading. Folder names do not define identity; the manifest ID does.

Settings → Landmark packs enables/disables packs in the picker. With one enabled pack the first orbit ring shows its landmarks directly; with several it shows packs, and clicking a pack opens its landmarks in the second ring. Escape or the close bubble closes the outer ring. Disabling a pack never removes or changes existing landmarks. If all packs are off, placement asks the user to enable a pack; existing landmarks can still be selected, moved, resized, duplicated and deleted.

A minimal manifest (all timing values are seconds):

```json
{
  "schema_version": 1,
  "id": "my.atlas",
  "name": "My Atlas Landmarks",
  "description": "Hand-drawn observatories.",
  "icon": "pack.svg",
  "items": [
    {
      "id": "observatory",
      "name": "Observatory",
      "description": "A hilltop observatory.",
      "svg": "observatory.svg",
      "icon": "observatory-icon.svg",
      "width": 160,
      "anchor": [80, 104],
      "parts": [
        {"id": "body", "delay": 0, "duration": 0.3, "motion": "rise", "pivot": [80, 104]}
      ]
    }
  ]
}
```

Pack `icon` is optional: without it, settings and orbit show the first letter of the trimmed pack name, uppercased. An item's `icon` is also optional and defaults to its complete SVG. Icons are SVGs and should remain legible in a small orbit bubble; pack icons use the same SVG rasterizer but render as theme-tinted masks so they stay readable in both appearances. An explicitly supplied icon must be a valid local SVG.

Pack and item IDs use ASCII letters, numbers, dots, underscores and hyphens (up to 96 bytes each); duplicate IDs are rejected. Saved landmarks use `pack-id:item-id`, independently of display names. Legacy `pin`, `town`, `castle` and `port` IDs load as `elmapa.basic:…`. A missing/uninstalled pack preserves its saved IDs and displays the basic pin as a fallback until that pack is restored. Packs do not embed themselves in saved maps, so recipients need the same packs. Keep IDs stable when editing artwork. Renaming a pack or item does not change its identity.

`width` is the nominal artwork width used to scale geometry to the placed landmark size; `anchor` is its insertion/ground point in SVG coordinates. The basic illustrations use a 160 × 128 viewBox, width 160 and ground anchor **(80, 104)**; new packs can choose their own geometry dimensions and anchors. Pale parchment faces, finer sepia contours, directional roof/side hatching and broken masonry marks match the existing terrain.

Use self-contained solid-color vector paths, polygons, ellipses and rectangles. External references, fonts, images, gradients, filters, clips and masks are unsupported. SVG paths must sit inside named `<g id="…">` animation parts; unlabelled wrapper groups are allowed. Part groups must be disjoint and match the manifest's `parts` order exactly. That order controls permanent painting/occlusion, while each part's independent `delay` controls its arrival. Keep visible geometry inside the SVG viewport. SVG paths are relative to the pack folder; absolute paths and traversal outside it are rejected.

Available motions:

| Motion | Placement behavior | Shader accent |
| --- | --- | --- |
| `rise` | Builds upward from the part's ground pivot, with a restrained settle. | Short ink hatches lift from the pivot. |
| `drop` | Descends gently into the insertion point. | A flattened impact ripple and a few dust flecks appear on contact. |
| `settle` | A small descent and scale settle, suited to ground washes and boats. | A soft wash blooms out from the pivot. |
| `turn` | A small rotation into the final orientation, available for ornaments that need to rotate into place. | Tapered ink arcs sweep around the pivot. |
| `unfurl` | Extends from its pivot, used for timber piers. | A pair of fine lines travels in the direction of the reveal. |
| `stamp` | Expands from its pivot, briefly overshoots, and snaps into place. | A double ink impression spreads and fades. |

Keep the complete assembly under one second. The native renderer draws each shader accent at its part's pivot during placement, duplication, specimen changes and Replay this view. The accents use temporary GPU materials, and saved and exported artwork uses the finished geometry. Reduced Motion renders the final pose immediately. Reopening, dragging and undo/redo do not replay the arrival.

To add another landmark, author its SVG and add an item to its pack's manifest. To add another bundled pack, add another folder under `packs/`. Neither requires a Rust enum, SVG match arm or icon registration. Run the pack/round-trip tests and full README checks, then exercise placement in the native app beside the existing terrain.

In Codex, use `$iconpack` followed by a theme to research and build a landmark pack. [Iconpack skill](../../.agents/skills/iconpack/SKILL.md)

The action orbit, layer rendering, bounds, selection, saving, thumbnails and SVG export consume this registry/shared geometry. The native UI rasterizes one preview per specimen at startup. Canvas artwork stays vector geometry with retained meshes per part; only temporary component transforms and material alpha update while it assembles. `tool.svg` is the compact UI/cursor glyph and is not placeable artwork.

Independently occluding structures use separate parts, each with its own foundation pivot. The town's houses are ordered from back to front; the harbor separates warehouses, cargo and crane, whose pedestal sits on the clear right-hand end of the quay. The castle has matched outer towers, matched taller shoulders and a dominant central keep. Preserve broad staged assembly while assigning each new part its own short delay. Existing atlases migrate their basic landmark IDs automatically and retain their finished artwork.

Roof construction uses a single oblique frame for each building: the ridge stays parallel to the eave; downslope marks follow the projected pitch, with sparse cross-course marks. Gable walls use quiet masonry instead of roof texture. Chimneys penetrate the same roof plane. The church tower and nave share a foundation, and the steeple starts at the tower cornice. Keep SVG group order identical to registry order: orbit previews rasterize the SVG directly, while the canvas uses registry batches. The tower draws behind foreground cottages but retains its later arrival delay.

For depth, draw the underlying wash/quay first, then order structural parts from rear foundations to nearer foundations. Do the same inside any multi-object group. Do not use arrival delay as drawing order: a rear building may arrive later and must still remain behind a foreground roof. Avoid intersecting ground footprints; where silhouettes overlap, the nearer opaque face must hide rear foundations and braces. The regression tests sample those actual painted overlaps in town and harbor geometry.
