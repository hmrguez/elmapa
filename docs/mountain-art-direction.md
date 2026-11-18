# Parchment mountain relief

Reference search, 17 November 2026. The existing forest's warm paper and sepia penwork set the local palette. Mountains now use original procedural vector artwork in the same family, rather than green shaded ribbons.

## Image references

- [Ortelius, Theatrum orbis terrarum, Library of Congress](https://www.loc.gov/item/2003683482/): pictorial relief and repeated, overlapping hill symbols. [Africa sheet](https://www.loc.gov/resource/g3200m.gct00126/?sp=18).
- [Mercator / Janssonius, Marocchi Regnum, Amsterdam 1628](https://www.swaen.com/listing/marocchi-regnum/6503): engraved Atlas Mountain chains, pale paper and restrained hand coloring. [Image](https://www.swaen.com/uploads/cache/90142-500x500.jpg).
- [Ortelius, Tempe, 1601 engraving](https://www.ebay.com/itm/185220616588): articulated rocky silhouettes and fall-line hachures. [Image](https://i.ebayimg.com/images/g/GvkAAOSw8xZhvqzZ/s-l1600.jpg).

These are visual references, not imported or traced assets. The implementation combines large weathered peaks, irregular ledges, branching gullies, rock strata, hundreds of short pen marks, a wandering crest, pale parchment faces and overlapping ochre washes. Strength controls shading. A click paints one peak; dragging adds stable peaks at fixed arc-length intervals.

## Uplift

Each mountain has two retained mesh flanks sharing a ground pivot. For 320 ms the flanks converge by at most 2.5 world units apiece while the ridge rises smoothly from 6% height to full relief. There is no camera shake, particle cloud or input lock. New peaks rise during painting and continue after release; existing peaks retain their positions and clocks as the path grows. Animation state is bounded to 2,048 peaks and expired entries are reclaimed.

Animation belongs only to native presentation. Scene geometry, thumbnails, SVG and saved strokes always represent the completed mountain. Undo, redo, cancellation and atlas installation clear active uplift; reduced motion presents final geometry immediately. Mountain and forest compositing follows layer order. On the same layer, a tree draws over a peak when its trunk is south of the peak’s base. A peak draws over trees rooted at or north of its base.
