# Using Elmapa

Elmapa opens on the atlas shelf. Choose a map card to edit that world. The shelf starts with the example world; use **New atlas** for a blank one or **Open atlas…** to add an existing `.elmapa` file. Cards wrap as the window changes size and the shelf scrolls for larger collections. Settings contain light/dark appearance, reduced motion, and landmark pack toggles. Card, button, and layer hovers ease in and out; reduced motion makes feedback immediate. **Atlases** in the editor returns to the shelf. Each atlas keeps its own editable local snapshot and the center of your last view when you return to the shelf or close normally; its terrain preview updates from that snapshot. Reduced motion skips the book transition.

The canvas is an open, pannable workspace: paint in any direction, including beyond the original map area. Choose a brush in the bottom toolbar and drag on the canvas. A rounded selection bubble glides and stretches between tools when you click or use a shortcut; reduced motion makes selection immediate. Brush size and strength have ready-to-use defaults; additional controls live under the icon-only brush settings button. The canvas fills the workspace. A single-row floating panel at the upper right holds the map name and undo/redo; hover the name to check whether changes are saved. Its animated menu toggle rotates upward while expanded and reveals save/open, new world, SVG export, the example world, and theme switching. The narrower layer stack floats independently below it. Fixed-width layer rows show colored brush icons and a visibility toggle; hover a row for its name and opacity. Select a row to paint on it, use + to add a layer, and open the layer settings for ordering, opacity, and deletion. Long layer stacks scroll without covering the brush bar. Escape closes the extra controls.

## Keyboard and mouse controls

The table uses macOS key names. On Linux and Windows, use Ctrl for ⌘, Shift for ⇧, and Alt for Option. For example, save with Ctrl+S and undo with Ctrl+Z.

| Shortcut / gesture | Action |
| --- | --- |
| Left-click / left-drag | Paint a dab / stroke on the active visible layer |
| Option / Alt + left-click with Land, Water, Forest, or Mountains selected | Open the brush material orbit |
| Hold left-click in place for 1.5 seconds with Land, Water, Forest, or Mountains selected | Open the same orbit |
| `1`–`7` | Land, water, forest, mountains, river, trail, eraser |
| `T` / `8` | Text tool |
| `V` / `9` | Select labels, landmarks, rivers, or trails |
| `L` / `0` | Landmark placement tool |
| Double-click empty canvas (Select) | Draft one label |
| Typing (selected label) | Replace its name directly |
| `⌘D` / `Backspace` (selected landmark) | Duplicate / delete the landmark |
| Hold mouse3 (middle button) + drag | Pan while held; releasing immediately returns to painting with the selected brush |
| `[` / `]` | Decrease / increase brush size |
| `⌘Z` / `⇧⌘Z` | Undo / redo |
| `⌘S` | Save map |
| `⌘O` | Open map |
| `⌘E` | Export SVG |
| `⌘0` | Fit visible artwork in view |
| `⇧⌘R` | Replay this view |
| `⌘Q` | Save the local atlas snapshot and quit |
| `⌘N` | New atlas from the shelf |
| `⌘,` | Shelf settings |
| `⇧⌘H` | Return to the atlas shelf |
| Scroll up / down over the canvas | Zoom in / out around the pointer; no modifier needed (`⌘` + scroll also works) |
| Scroll over the layer list / atlas shelf | Scroll that list |
| `Esc` | Cancel the current stroke or pan gesture and close extra controls |

The floating zoom **− / +** buttons zoom around the canvas center; **Fit** does the same as `⌘0`. The toolbar provides brush selection, brush-size **− / +**, and a brush settings button with strength **− / +**. Layer controls select the painting layer, toggle visibility, add layers, and adjust ordering, opacity, or deletion. These mouse controls need no modifier. A click on empty canvas closes an open map menu before painting. Holding Shift, Control, or Space does not add a canvas gesture; `H`, right click, and pinch have no dedicated canvas action.

## Brush materials

With **Land**, **Water**, **Forest**, or **Mountains** selected, choose a finish under **Material** in brush settings or open the material orbit on the canvas. Land offers **Parchment**, **Ochre Sands**, **Lichen Heath**, and **Weathered Shale**. Water offers **Ocean**, **Shallow Ocean**, and **Lake**. Forest offers **Mixed Woodland**, **Fir Grove**, and **Oak Grove**. Mountains offers **Engraved Peaks**, **Alpine Crags**, **Ochre Escarpments**, and **Weathered Highlands**. Hover a swatch for its name. The gesture opens the picker without placing a dab. Choose a swatch to set the finish for subsequent strokes, or press Escape or click outside the orbit to dismiss it. Paint over an existing Land or Water stroke to change its finish. Forest and mountain finishes set the artwork of newly placed trees and peaks. Materials follow layer order and opacity and carry through to saves, shelf previews, and SVG exports. Water finishes apply to painted strokes. The ocean backdrop and older Water strokes retain their original washes. See the [brush material manifest guide](../assets/brush-materials/README.md) to add another finish.

## Text and landmarks

Choose **Text** (or press `T` / `8`) to add map names. Click the canvas and type the name directly on the map. Press Return or click the checkmark in the orbit to place it. Clicking the next map point also places a valid name and starts the next draft in the same click. The draft’s orbit lets you change typeface, ink and typography while seeing the name update live. The current style carries forward to the next label. You can also set it ahead of time with the **Aa** button. Escape cancels the current draft or returns to Select when the canvas is ready for another name. New labels follow the active layer.

Choose **Landmarks** (or press `L` / `0`) to place map symbols. The toolbar shows the symbol ready to place. Click its thumbnail to browse enabled packs and choose a different symbol. The cursor previews the current artwork at placement size. Each map click places one landmark and keeps the tool active. Use the toolbar **− / +** or `[` / `]` to change placement size. The icon and size carry across tool switches within the current atlas. Escape returns to Select.

Use **Select** (or `V` / `9`) to edit an existing landmark. Drag to move it, resize with **− / +** or `[` / `]`, or choose **Change icon** from its orbit to open the catalog for that landmark. The orbit also offers Duplicate and Delete (`⌘D` and Backspace). Each placement, icon change, size adjustment and completed drag is one undo step. Settings → Landmark packs controls the available artwork. Turning a pack off leaves placed landmarks visible and editable. Landmarks follow layer visibility, opacity and order, and erasing terrain leaves them intact. Reduced Motion shows finished artwork immediately. Saves, shelf previews and SVG use finished geometry. The [SVG registry guide](../assets/landmarks/README.md) describes how to install and author packs and their animation instructions.

Choose **Select** (or press `V` / `9`) to select labels across visible layers. Double-click empty canvas to draft one label and stay in Select when you finish. Drag a name to move it, drag any corner of its selection frame to resize it proportionally from the opposite corner, or drag the round handle beneath the frame to rotate. Hold Shift while rotating to snap to 15° increments. Handles keep their screen size at every zoom, and each completed move, resize, or rotation is one undo step. Escape cancels an active gesture. Use **Regions** to select land for division.

Click a river or trail with **Select** to show its nodes. Drag a node to bend the nearby course, or drag an endpoint to move where it begins or ends. The orbit adds or removes editing handles for finer or broader adjustments. Release to commit one undo step. Press Escape during a drag to cancel it. Click empty canvas or choose **Done** in the orbit to deselect.

The selected name has five actions in its first ring. Clockwise from the top, they are **Typeface**, **Ink**, **Duplicate**, **Delete**, and **Advanced typography**. Typeface, Ink, and Advanced typography each open a second ring while keeping the first ring visible and highlighting the active action. The second ring fills clockwise with a staggered reveal. Reduced Motion shows it immediately. Hover a bubble for its action name. Click an open parent again or use the outer ring’s close button to close that level. Clicking empty canvas deselects.

**Advanced typography** contains letter spacing, opacity, curvature, **Use style for new labels**, and **Reset style**. In **Curvature**, enter an angle from −120° to 120° or use **− / +** to adjust it by 10°. Positive angles arch the name upward, negative angles bow it downward, and 0° gives a straight baseline. Settings open a small contextual card near the label. Enter or click outside the card to apply the value and close it. Escape discards the numeric draft and closes the card. Use the canvas handles to resize or rotate a label.

Start typing while a label is selected to replace its name. The first keystroke replaces the old name and shows a caret directly on the map. Arrow keys, Shift selection, Home/End, Backspace, and macOS copy, cut, paste, and select-all work as text editing keys. Canvas shortcuts are suspended while the label is selected. Enter commits the name and keeps the label selected. Escape or a click on empty canvas commits the name and deselects it.

Name edits preview live. New names descend gently and settle over 300 ms; Reduced Motion skips this. The animation never changes saved or exported geometry. SVG exports include font outlines, so recipients need no installed fonts. Older version 1 atlases open with no labels and retain their existing terrain.

## Regions

Choose **Regions** in the bottom toolbar and click land. The outline shows the selected piece. **Make region** claims the selection, while **Draw border** lets you drag from one edge of the selection to another. Start and finish at or just beyond the edges. Draw a single crossing at a time. Elmapa creates a Regions layer on the first edit. Once the split is in place, grab its shared border and drag to reshape both sides. Escape cancels the active gesture and each completed edit is one undo step.

With land selected, choose **Split into…** for a draft of 2–16 regions. Use **− / +** to change the count, then drag the dashed borders to reshape the preview. Changing the count redraws the draft. **Apply split** saves the subdivision as one undo step. **Cancel** or Escape discards it. Offshore islands outside the selection stay unassigned.

Select a region and choose **Add islands**, then click each unclaimed island you want it to own. Choose **Done** to return to selection. The mainland and islands share a wash and region name. Regions layers can be hidden, reordered, and faded like other layers. Use **New Regions layer** in the layer stack while the tool is active to draw another set of boundaries over the same land. Region edits persist in atlas saves and SVG exports. Coast edits clip the visible washes and borders to the new shoreline while preserving the underlying ownership.

When a claimed region is selected, its orbit lets you rename it, change its fill color, or toggle the map label. Choose a swatch or enter a custom hex color. The change applies to all islands owned by that region.

## Documents and terrain

Choose **Replay this view** from the map actions menu to animate the finished artwork in the current canvas view. Surviving terrain animates in brush order, and names and landmarks arrive at their saved positions. Land and water use a feathered ink wash with approximate timing for strokes that merge into one region. Atlases saved before this feature use the stored order within each layer for older strokes. **Stop replay** or Escape restores the canvas immediately. Replay follows Reduced Motion.

Maps save locally as versioned `.elmapa` JSON documents containing editable brush paths, landmarks and labels, layer settings, and stable variation seeds. Saving replaces the destination atomically. Switching atlases starts a separate editing session; Undo stays within the current atlas. Undo history is not stored in the file. Shelf snapshots, camera settings and appearance preferences share a local library directory:

| Platform | Default library directory |
| --- | --- |
| macOS | `~/Library/Application Support/Elmapa` |
| Windows | `%LOCALAPPDATA%\Elmapa` |
| Linux | `$XDG_DATA_HOME/Elmapa`, or `~/.local/share/Elmapa` when unset |

Save (`⌘S`) still writes the chosen external `.elmapa` file; returning to the shelf or quitting preserves a separate library copy without overwriting that external file. Existing files appear after opening them once. `ELMAPA_LIBRARY_DIR` overrides the library directory for isolated native testing. SVG export uses the same terrain geometry as the canvas and frames visible brushwork with a margin. Saved document dimensions remain the initial view for an empty world; they do not limit painting.

Layers composite from bottom to top. Within each layer, the latest Land or Water stroke wins wherever they overlap. Water cuts bays or lakes into land, and Land fills water back in. The shared shoreline and its shallow wash on the water side update during the drag. Rivers stop at painted water and open the shoreline where the channel flows into a lake or out to sea, including across visible layers. Terrain surfaces and rivers render first. Mountains and trees on the same layer draw north to south by each peak’s base and each tree’s trunk. A tree covers a peak if its trunk is farther south. At equal depth the peak draws on top. Landmarks and text labels draw after terrain within each layer. Trails draw above rivers and below mountains and trees, including across different layers. The eraser removes earlier brushwork on the active layer; paint added afterward survives. A hidden layer cannot be painted, and the final layer cannot be deleted.
