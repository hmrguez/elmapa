---
name: brush-material
description: Add or revise Elmapa Land, Water, Forest or Mountains brush finishes, including texture tiles, sprite artwork, swatches and material manifest entries.
---

# Author a brush finish

Read the [material guide](../../../assets/brush-materials/README.md) and compare the requested finish with an existing entry for the same brush in the [manifest](../../../assets/brush-materials/manifest.json). Check the source and redistribution terms of any external artwork and record the credit beside the existing texture credits.

Preserve saved material IDs and the default finish at the head of each brush group. Append a new finish to its group. Use opaque seamless tiles for painted ground or water, trunk-centered vector artwork for forest specimens and bottom-aligned transparent frames for mountain sprites. For a new forest family, trace the existing family through the material registry and the specimen list before editing. For a texture change, inspect the repeated pattern at normal map scale. For specimens, check their silhouette at swatch size and their ground alignment beside existing artwork.

Rebuild to embed the assets. In an isolated atlas library, choose the finish from the material orbit and paint overlapping strokes on a visible layer. Check the swatch, native result, shelf preview, saved and reopened atlas, and SVG. When revising an existing finish, open a document containing the previous ID. Run the [repository checks](../../../README.md#validation) and report which surfaces and saved-file behavior you verified.
