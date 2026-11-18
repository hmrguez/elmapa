# Parchment roads

Reference study, 17 November 2026. The existing parchment mountains and trees set the local palette and pen weight. Historical references below are early modern engraved atlases, selected for the requested old-atlas feeling rather than as evidence of ancient road cartography.

## Image references

- [John Ogilby, The Road from London to Aberistwith, London–Islip section, 1675, Bodleian Library Vet. A3 b.10](https://digital.bodleian.ox.ac.uk/objects/54cf0c8c-2044-439c-bf64-82440accd9da/). [Full scan](https://iiif.bodleian.ox.ac.uk/iiif/image/54cf0c8c-2044-439c-bf64-82440accd9da/full/1818,/0/default.jpg), [collection manifest and description](https://iiif.bodleian.ox.ac.uk/iiif/manifest/54cf0c8c-2044-439c-bf64-82440accd9da.json). Visually inspected: the route reads as a pale narrow corridor with paired fine edges, locally interrupted or dotted; little pictorial trees and engraved hills occupy the surrounding paper. The library describes the strip format and one-inch-to-one-statute-mile scale.
- [John Ogilby, The Road from Oxford to Cambridge, 1675, Bodleian Library Gough Maps 100](https://digital.bodleian.ox.ac.uk/objects/58dd9d4e-7e97-4afe-be02-68eca145e262/). [Full scan](https://iiif.bodleian.ox.ac.uk/iiif/image/58dd9d4e-7e97-4afe-be02-68eca145e262/full/1818,/0/default.jpg), [collection manifest](https://iiif.bodleian.ox.ac.uk/iiif/manifest/58dd9d4e-7e97-4afe-be02-68eca145e262.json). Visually inspected: the paired road edges have a clear paper interior; angular side-road junctions open naturally into the corridor. Rivers carry heavier, darker ink than roads. Small roadside trees reinforce the scale.
- [Ogilby's Britannia, 1675, David Rumsey Historical Map Collection](https://www.davidrumsey.com/luna/servlet/detail/RUMSEY~8~1~278363~90051243%3ACovers--Britannia%2C-Volume-1%2C-Kingdo). The collection describes linear road maps with mileages and relief shown by sketches. This supports combining roads with pictorial terrain rather than a modern street-map treatment; the road observations above come from the inspected Bodleian scans.

These are reference links, not imported or traced assets. The Bodleian manifests identify their scans as CC BY-NC-SA 3.0; no scan is bundled with the application.

## Interpretation for Elmapa

The following are design decisions inferred from those images and the existing brush family, not claims that the old maps depict worn dirt or animated travel:

- Give the road a continuous, warm, pale earthen bed between two irregular sepia shoulders. Width should respond to brush size so a footpath and a broader road both read clearly.
- Keep the center lighter and quieter than the edges. Add sparse short ruts, stippled grit and small shoulder hachures in the same restrained ink language as the mountain engraving. Variation should remain anchored to distance along the stroke as it grows.
- Break a few edge marks rather than replacing the whole road with a uniform dashed centerline. Avoid paving blocks or evenly spaced transverse bars, which can read as a railway or ladder at map scale.
- Let curves and junctions remain legible; neighboring terrain should retain the strongest silhouettes. Strength can control the weathering and ink while preserving a recognizable road bed.

## Walking the road open

Reveal short sections in stroke order, with a small alternating cadence like successive footsteps. Each newly reached section begins as a narrow pressed track and opens sideways into its final shoulders; the earth and engraved detail settle immediately behind the advancing tip. Keep the rhythm subtle and quick enough to follow the brush. The result should feel like passage making a usable way through the paper, without a literal walking character or persistent footprint decoration.

As with tree growth and mountain uplift, animation belongs only to native presentation: saved strokes, thumbnails and SVG always describe the finished road. Continue the final few sections after pointer release; cancellation, atlas installation and reduced motion must show the appropriate final or removed geometry without replaying old roads.

## Native validation

The release build was exercised on macOS/Metal with the [native road replay](path-native-replay.json), launched through an isolated app bundle. The replay paints a curved 45-point road with growth enabled, captures two live frames and the settled result, saves and exports, undoes/redoes, erases and restores a section, paints with reduced motion, reopens the saved map and returns to the shelf.

The live captures show progressively advancing, narrow sections opening to the completed width. The completed road was inspected beside the existing mountains and trees. Redo reproduced the settled frame byte-for-byte. Erasing changed the visible road; the saved document retained the 45-point path, and SVG contained the completed earth bed and sepia bank geometry. The reopened map and shelf preview both retained the finished road. Native pointer automation reported `noWindowsAvailable`; the app's existing replay harness supplied the gestures through the ordinary Bevy input dispatcher, started with a native keyboard action. No real library was modified.

To repeat, prepare a fresh bundle using `scripts/prepare-native-test-app.py` with `--replay docs/path-native-replay.json` and an isolated library under `/private/tmp`, launch it and press **p**. Captures go to `/tmp/elmapa-road-*.png`; save/export artifacts go to the isolated library. Core tests cover shared road banks at bends, width, short clicks, doubling back, zero strength, deterministic geometry, ordered reveal, release continuation and no replay on reopening. Native mesh tests verify that the final walking-frame transform reproduces export coordinates in multiple directions.
