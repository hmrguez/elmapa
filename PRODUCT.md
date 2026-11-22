# Elmapa

<!-- impeccable:product-schema 1 -->

## Platform

Native desktop application for macOS, Linux, and Windows.

## Users

Elmapa serves tabletop game masters and worldbuilders making maps for their settings. They draw places they can use at the table or share with others, then return to the atlas as the setting evolves.

## Product Purpose

Elmapa lets people paint their worlds directly on the canvas and turn them into illustrated parchment atlases. A finished map can be reopened for another session, shared, or printed.

## Positioning

The mapmaker draws the coastline, lays out terrain, and traces routes by hand. Elmapa’s brushes generate the illustrated terrain as they paint. Layers keep the atlas editable as the setting grows, and the same atlas can hold landmarks, place names, and regions.

## Operating Context

The atlas shelf is the home for local maps. Mapmakers can start a blank atlas or explore the bundled example world, paint and revise on a pannable canvas, and switch between atlases over multiple sessions. The shelf keeps local snapshots. A `.elmapa` file stays editable, and a self-contained SVG is the output for sharing or printing.

## Capabilities and Constraints

- Support both Holo and Paper in future editor UI work. Holo is the established default appearance, and Paper is in active development. Verify affected workflows in both appearances.
- Preserve the local atlas library, compatibility with saved `.elmapa` files, and self-contained SVG export.
- Keep painting, selection, and layer editing available on the canvas. Preserve the editor’s undo and redo behavior.

## Evidence on Hand

- The [user guide](docs/user-guide.md) documents the shelf, editor, and export workflows.
- [DESIGN.md](DESIGN.md) describes Holo and Paper and records the existing Paper toolbar treatment.
- The [editor screenshot](docs/images/map-editor.png) and [shelf screenshot](docs/images/atlas-shelf.png) show the app in use.

## Product Principles

- Give the mapmaker direct control of the map’s geography. Let brushes do the repeated illustration work as the mapmaker paints.
- Keep the map in view and common editing actions within reach while the mapmaker works.
- Make it easy to come back to an atlas, revise it, and export the result.

## Accessibility & Inclusion

Preserve Reduced Motion across both appearances. When it is enabled, feedback and artwork should reach the same usable state without waiting for the animation. Preserve documented keyboard shortcuts for common editor actions.
