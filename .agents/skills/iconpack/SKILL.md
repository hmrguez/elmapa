---
name: iconpack
description: Create or extend placeable Elmapa landmark icon packs, from online reference research to parchment SVG artwork and spawn animations. Use for $iconpack or /iconpack followed by a theme, or for requests to add Elmapa map symbols.
---

# Elmapa landmark icon packs

Treat the words after `iconpack` as the brief. A broad theme such as “About nature” is enough to start. Choose a fitting name and a set of distinct map symbols. Honor any stated region, era, subjects, or art direction. When extending an existing pack, keep its saved IDs.

## Research and select subjects

1. Read the [landmark pack guide](../../../assets/landmarks/README.md) for the current JSON/SVG contract. Inspect representative SVGs and manifests in `assets/landmarks/packs/` for scale, palette, and construction. Follow the repo's worktree and merge instructions for implementation.
2. Research the theme online. Start with sources that can establish recognizable forms and context, such as museum and archive collections, heritage authorities, field guides, or botanical institutions. Open the original entries and inspect reference images where available. For a region or period brief, check that the features you intend to draw fit that brief. Keep a short record of the source links and the details that informed the art. Look at the reference notes in the [Nature pack](../../../assets/landmarks/packs/nature/README.md) for an example.
3. Select subjects that add different map uses or silhouettes. Compare candidates with existing landmarks and terrain brushes. Prefer fewer distinct specimens when the theme yields close variants, and give each specimen a recognizable feature drawn from the brief. For a nature theme, a spring, a sea arch, and a solitary yew could mark three different places. Let the theme and legibility in the orbit determine the count.

## Draw and animate

4. Draw original, lightly oblique vignettes that look at home beside Elmapa's trees, relief, and rivers. Use believable forms simplified for map scale and a monochromatic parchment and sepia palette. Leave broad lit faces pale, shade receding planes, and use restrained directional hatching. Carry depth through overlap and exposed side faces. Anchor each specimen to its footprint with a quiet wash or broken ground marks where appropriate. Use source images as guidance for shapes and materials, and ship original vectors. Check a representative specimen at normal map zoom and orbit-bubble size early enough to adjust the silhouette and density of detail across the set.
5. Write the SVGs and update `manifest.json` under `assets/landmarks/packs/`. Give a new pack stable ASCII IDs and a simple silhouette for its pack icon. Use the pack guide for supported SVG geometry, width and anchor, and part group order. Reuse an item's full SVG as its orbit preview where it reads clearly. Provide a dedicated small preview where the full artwork loses its identifying silhouette. Record external references and the observed features in a short pack note when the illustrations depend on them.
6. Give each specimen a short assembly sequence that reflects its form. Use independent pivots for parts that grow, settle, or turn separately. Keep back-to-front drawing order separate from arrival timing. A tree might rise from its roots before its crown opens, while an outcrop can lift as the ground wash settles. Let the native ground-ink effect supply placement feedback. Reuse the existing motions and add a reusable motion only when the specimen calls for a distinct movement. Finish in under one second, preserve the exact final pose in Reduced Motion, and keep saves and exports tied to finished geometry.

## Verify in Elmapa

7. Run the pack/registry and round-trip tests plus the checks required by the repo. In an isolated native library, confirm the pack appears in **Settings → Landmark packs**, then open it from the **Landmarks** orbit. Check that every specimen can be reached and identified, both as a nested pack and when it is the only enabled pack. Compare the finished art beside terrain at ordinary map zoom. Exercise placement, spawn animation, Reduced Motion, save/reopen, and SVG export. The [native landmark replay](../../../docs/landmark-native-replay.json) is a starting point for repeatable checks. Use the new `pack-id:item-id` values for replay selection and verify the picker itself in the native app.
8. Summarize the subjects, the reference-driven details and placement motions worth calling out, and the checks and merge result. Show a preview if it helps the user review the set.
