---
name: Elmapa UI
description: Pale pigments, fine paper grain, and crisp ink illustrations for the atlas editor.
colors:
  selection-ink: "#5c3a22"
  linen: "#d8caae"
  olive-paper: "#c9c99e"
  sea-glass-paper: "#b2c8c3"
  sage-paper: "#b7c3a7"
  clay-paper: "#ccbcab"
  blue-paper: "#b2c2ca"
  sand-paper: "#e0c9a2"
  ochre-paper: "#dcbe96"
  terracotta-paper: "#d4b19c"
  rose-paper: "#d9b8ae"
  straw-paper: "#ddcfa2"
typography:
  control-label:
    fontFamily: "Avenir Next"
    fontSize: "12px"
rounded:
  tool-tile: "5px"
spacing:
  tool-gap: "6px"
  contextual-gap: "8px"
components:
  tool-tile:
    width: "44px"
    height: "44px"
    rounded: "{rounded.tool-tile}"
---

## Overview

Elmapa offers two appearances. **Holo** is the default, with translucent floating panels, rounded controls, and tinted line icons. **Paper** takes its cues from small printed swatches laid out on a cartographer’s worktable. Muted pigment and fine paper grain give the controls a hand-printed feel. Crisp illustrations make them easy to scan while drawing. The artwork on the atlas carries the strongest detail.

Choose **Holo** or **Paper** under **Appearance** in shelf Settings or the editor’s map menu. Paper styles the illustrated toolbar, editor header, map menu, brush and placement controls, Layers panel, and Regions controls.

## Paper colors

Use quiet, pale pigments on the faces of controls. In the toolbar, sage identifies Forest, sea glass identifies Water, and rose identifies Eraser. Apply color to a whole control face and let the illustration communicate its purpose as well. When a menu introduces a new role, start with the existing pigments and give the role a consistent hue.

Use selection ink for the active outline. Give a hovered tile a slight lift in brightness and leave enough contrast for a clear active state. On larger panels, give paper colors a quieter role so text, icons, and selection stay easy to read.

## Paper typography

Use Avenir Next for concise control labels, values, and instructions. Let the illustrated icon lead in compact tool rows and use a tooltip to name its action. Give menus visible text whenever choices need to be read or compared. Use the editor’s Baskerville for titles when a panel needs a heading.

## Paper layout

The toolbar is a row of individual square controls. The atlas shows in the spaces between them. Keep the row steady when the active tool changes. Put controls for the current tool in a compact strip above the row, where the relationship is easy to see.

Carry the toolbar’s spacing, icon treatment, and contained grain into adjacent controls. Give related options a shared, quiet surface when that makes the group easier to scan.

## Paper elevation & depth

Use the fine, stationary [paper grain](assets/tool-icons/paper.png) as a face texture. Keep it inside the control edges and subtle enough for the icon to read at its normal size. Use minimal depth for individual controls. A soft offset shadow is appropriate for a floating menu where it helps distinguish the menu from the atlas.

## Paper shapes

Use near-square corners and thin borders. The selected tool gets a two-pixel ink outline that follows selection and snaps into place with Reduced motion.

## Components

The [shared UI components](src/app/ui/components.rs) define the vocabulary for both appearances. Use the same component for the same interaction so layout, styling, and input stay consistent across editor surfaces.

| Component | Role |
| --- | --- |
| Panel | Groups related controls on a surface and blocks pointer input from reaching the canvas underneath. |
| Icon | Shows a symbol, with a tinted glyph or an authored illustration depending on its role. |
| Button | Invokes an action and owns its hit area, hover, tooltip, and selected state. A tool button specializes this for tool selection. |
| Appearance picker | Shows Holo and Paper and highlights the current choice. Settings and the map menu use the same picker. |
| Stepper | Shows a numeric value with buttons for discrete decreases and increases. A slider is the component for direct adjustment along a track when a surface needs one. |

### Toolbar

![Elmapa editor close-up showing the Paper UI toolbar](docs/images/paper-ui-toolbar.png)

The [editor toolbar](src/app/ui/editor/toolbar.rs) defines tool order, labels, actions, and contextual controls. The [toolbar components](src/app/ui/components/toolbar.rs) render Holo's shared tray and moving highlight or Paper's tinted tiles and moving ink outline. Paper tiles use the [illustrated tool symbols](assets/tool-icons/symbols.svg) and a fine grain. Both appearances use the same selection behavior and respect Reduced motion.

Use the toolbar as the reference when adding Paper treatments to dialogs and the atlas shelf. Check both appearances over real atlas artwork as each surface gains a Paper treatment.

### Map menu and header

The editor header, Atlases button, and map menu use warm paper surfaces with fine grain and a soft offset shadow. The atlas title uses Baskerville. Menu actions use Avenir Next labels and ink-tinted symbols. A sand-colored face and a two-pixel ink outline mark the open menu button and selected appearance.

The [Paper component styling](src/app/ui/components/paper.rs) supplies the panel and button treatments. The editor owns the [header layout](src/app/ui/editor/mod.rs) and [menu actions and animation](src/app/ui/editor/menu.rs). Reduced Motion opens and closes the menu immediately.

### Brush and placement controls

The contextual strip sits eight pixels above the illustrated tiles and groups the current tool’s controls on a single paper surface. Advanced brush settings and the New label style card use the same warm grain and offset shadow, with Baskerville headings and Avenir Next control labels. On Paper the two cards share a width at the editor’s supported window sizes. The Advanced and Material buttons echo the active brush tile, and ochre connects the Text tile, style button, and typeface choices. Ink choices use distinct faces alongside their printed ink swatches. Sea glass identifies Advanced typography, and numeric steppers stay on the quiet panel surface.

Brush Orbit, landmark choices, and lettering choices use small paper buttons that follow the existing rings around the pointer or selection. Sage, ochre, and blue distinguish Core, Desert, and Alpine. Landmark packs use straw, terracotta, and sage. Ochre, blue, and sea glass identify Typeface, Ink, and Advanced typography in the lettering orbit. A light border defines each quiet face. Hover lifts the pigment or adds a sand wash, and a two-pixel selection ink border marks an open branch or the chosen option. Material artwork, landmark illustrations, and ink swatches retain their own colors. Reduced motion reveals each ring immediately.

The [shared orbit button](src/app/ui/components/orbit.rs) supplies the face and hover treatment. The [editor toolbar](src/app/ui/editor/toolbar.rs) owns the strip and Advanced settings layout. The [overlay renderer](src/app/ui/overlays/orbit.rs) owns ring positions and reveal timing, and the [lettering overlay](src/app/ui/overlays/text.rs) lays out the style cards.

### Layers and Regions

The Layers panel uses a warm, grained surface. A sand wash and two-pixel ink outline mark the active layer and selected region. Indentation and chevrons show the hierarchy. Artwork rows reuse the toolbar’s illustrations and pigments at a smaller size, so a layer’s brushes, text, and landmarks are recognizable by shape and color. Straw identifies Regions layers and linen identifies empty artwork layers. Hidden layer contents fade and the crossed-out eye gets a rose face. The tree scrolls above the fixed add and settings controls. Blue groups layer order, sea glass groups opacity, and rose marks deletion. The full panel shields the canvas from pointer input.

The Regions strip uses the same paper surface and can wrap its controls at narrow widths. Sand marks border drawing, sea glass marks island assignment, sage marks creation and completion, and straw marks splitting and child regions. In the style picker and edit card, ochre identifies renaming and blue identifies color editing. Region swatches keep their atlas pigments in the tree, strip, and picker. The edit card uses a Baskerville heading and an ink outline around the focused field.

The [shared Layers components](src/app/ui/components/layers.rs) style rows and their controls. The [Layers panel](src/app/ui/editor/layers.rs) and [region tree](src/app/ui/editor/layers/regions.rs) own the composition and actions.
