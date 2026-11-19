# Elmapa Atlas Icons · 1.0.0

46 original, MIT-licensed SVG icons for a quiet cartographic instrument panel.
Individual SVGs in this directory are the editable source of truth; the native
app embeds those exact files. Map terrain is still procedural geometry.

## Design contract

- 24 × 24 grid, two-unit strokes, round joins and caps, transparent backgrounds.
- `currentColor`, no fonts, raster images, external references or dependencies.
- Designed for the app's 17 px icons; use at 17, 24, 32 or 48 px.
- An irregular coastline means land; waves mean open water; a grove means forest;
  ridged peaks mean mountains; continuous meandering banks mean river; a dashed
  meander means trail. The eraser is a physical eraser. These seven drawings are
  distinct in the toolbar and the material swatches in the layer panel.
- An open atlas book returns to the atlas shelf. Checkmarks indicate enabled
  settings; a cross closes settings; a right chevron opens a book.
- Stacks are reserved for layers. A folded map means a world; a map with a plus
  creates a world. A stack with a plus creates a layer. Shared borders mark the
  Regions tool, and a divided stack with a plus creates a Regions layer. Fit
  artwork uses a small frame within framing corners. SVG export uses vector
  nodes and an outward arrow.
- Lettering actions have dedicated pictograms: a pencil edits, overlapping labels
  duplicate, an ellipsis opens More, and a letter with a fine adjustment slider
  opens Advanced typography. Tracking uses opposing arrows, opacity a half-filled
  disc, and reset a circular arrow around a letter. A text-plus adds another name.
  Nested selection menus never borrow the same icon for different actions.
- Standard controls keep familiar pictograms, redrawn to the same optical weight.

`manifest.json` lists every icon, its label, category and actual control uses.
Reuse the same material icon wherever that material appears. When adding a new
control, add its semantic target here instead of borrowing an unrelated symbol.

## Export and reuse

Run from the repository root:

```sh
python3 scripts/export-icons.py
python3 scripts/export-icons.py --check
```

The generator writes local exports to `assets/icon-pack/`:

- `elmapa-atlas-icons-1.0.0.zip`: standalone pack with `svg/`, sprite, manifest,
  MIT license, this guide, and a self-contained visual catalog.
- `sprite.svg`: symbols named `elmapa-land`, `elmapa-forest`, etc.
- `catalog.html`: offline catalog with light/dark surfaces and 17/24/40 px samples.

Inline an SVG and inherit the surrounding text color, or use the sprite:

```html
<svg width="24" height="24" role="img" aria-label="Forest">
  <use href="sprite.svg#elmapa-forest" />
</svg>
```

For an icon-only button, give the button an accessible name; treat its SVG as
decorative. For `<img>` usage, SVGs render black by default; inline or sprite use
supports theme colors. Avoid changing stroke weight on just one icon.

## Pack audit and visual references

Reviewed on 2026-11-16:

| Candidate | Finding |
| --- | --- |
| [Lucide](https://lucide.dev/icons/trees) | Excellent general UI language and an actual forest symbol. The installed pack's forest mapping was wrong. [Route](https://lucide.dev/icons/route) represents navigation/waypoints; the map brush draws an unmarked dashed trail. Land, open water and river need a more explicit family. |
| [Tabler](https://tabler.io/icons) | Strong consistent 24 px outline system. Its broad catalog still requires judgment/substitutes for the exact terrain brush outputs. |
| [Game-icons nature sheet](https://game-icons.net/images/montages/tags/nature-icons.png) | Rich terrain silhouettes, but its dense game-art language does not fit a compact 17 px native toolbar. |

No single reviewed pack met the complete semantic and small-size requirements,
so this pack was drawn from scratch. Reference imagery: the
[USGS 2025 recreational topo legend](https://www.usgs.gov/media/images/25k-recreational-topo-example-legend-2025)
for water bodies, stream courses and dashed trails, and the
[Lucide forest design discussion](https://github.com/lucide-icons/lucide/issues/599)
for tree silhouette readability at small sizes. These are visual references;
no source SVG paths or image assets are included or traced.

The previous Lucide files have been replaced. `LICENSE` applies to all current
drawings and the generated pack; the pack has no third-party artwork.
