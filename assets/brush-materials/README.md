# Brush materials

The [manifest](manifest.json) supplies material order, swatches and texture settings for the brush orbit. The first entry for a brush is its default finish. Land defaults to Parchment, Water defaults to Ocean, Forest defaults to Mixed Woodland, and Mountains defaults to Engraved Peaks.

To add a Land or Water finish, put an opaque, tileable PNG beside the manifest and append an entry for it. The app embeds the referenced PNG when it builds.

| Field | Value |
| --- | --- |
| `id` | Stable identifier with up to 64 lowercase ASCII letters, digits and hyphens |
| `brush` | `land`, `water`, `forest`, or `mountain` |
| `name` | Display name for the swatch |
| `color` | Classic swatch color as `#rrggbb` |
| `woodland_color` | Swatch color and raster grading target for the woodland palette |
| `woodland_shoreline_ink` | Optional shoreline ink in the woodland palette |
| `texture` | PNG filename beside the manifest |
| `tile_size` | Edge length of one Land or Water texture repeat in map units |
| `shoreline_ink` | Optional `#rrggbb` ink for intermittent ripples along the painted shoreline of a Water finish |
| `forest` | Optional sprite family for the Forest brush. Supported families are `fir` and `oak` |
| `sprite_frames` | Number of equally sized frames laid out left to right in a transparent mountain PNG |
| `source` | Optional source URL for attribution |

Maps store the material ID with each stroke that uses an alternate finish or a textured default. Keep IDs stable when renaming or reordering finishes. Strokes with a missing or unavailable material ID use the fallback brush artwork for the atlas palette. When no palette is saved, Elmapa uses Classic. New Water strokes default to Ocean.

Forest finishes select families of trunk-centered SVG sprites. Add original SVG fragments to [forest/](forest/) and register them with the specimen list in `src/scene/terrain/forest/specimens.rs`. The canvas, swatches, shelf previews, and SVG export draw from the same artwork.

Mountain finishes use transparent sprites in a single horizontal PNG strip per finish. Match the scale of the existing strips, align each silhouette to the bottom center of its frame, and leave a few transparent pixels on every edge to avoid sampling neighboring artwork. The first frame supplies the swatch. Each painted peak selects a stable frame from its stroke seed. Older mountain strokes use the engraved default.

## Texture sources

Ocean, Shallow Ocean, and Lake use original procedural Elmapa tiles. The three Land texture PNGs are warm sepia, reduced-resolution adaptations of diffuse maps available under the [Poly Haven CC0 license](https://polyhaven.com/license).

- Ochre Sands uses [Sand 01](https://polyhaven.com/a/sand_01).
- Lichen Heath uses [Forest Ground 01](https://polyhaven.com/a/forrest_ground_01).
- Weathered Shale uses [Dry Ground Rocks](https://polyhaven.com/a/dry_ground_rocks).

The woodland palette grades the same seamless tiles and feathers the mountain sprite bases at their original frame dimensions. The six mountain sprites are original ImageGen artwork guided by [historical mountain relief](../../docs/mountain-art-direction.md).
