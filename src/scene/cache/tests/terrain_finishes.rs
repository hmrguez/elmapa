use super::{assert_material, stroke};
use crate::document::{Brush, Document, Point};
use crate::scene::{Scene, SceneCache, Shape};
use std::sync::Arc;

#[test]
fn land_materials_follow_paint_order_and_erasure_in_live_and_cold_scenes() {
    use crate::{brush_materials, scene::geometry::contains};

    let mut doc = Document::blank();
    let mut cache = SceneCache::default();
    let id = doc.layers[0].id;
    let at = |scene: &Scene, x| {
        scene
            .shapes()
            .filter_map(|shape| {
                let Shape::RegionSurface {
                    contours,
                    material: Some(index),
                    ..
                } = shape
                else {
                    return None;
                };
                let inside = contours
                    .iter()
                    .filter(|ring| contains(ring, Point::new(x, 390.)))
                    .count();
                (inside % 2 == 1).then_some(brush_materials::artwork_catalog()[*index].id.as_str())
            })
            .collect::<Vec<_>>()
    };

    for (material, brush, x, size, expected) in [
        (
            Some("ochre-sands"),
            Brush::Land,
            520.,
            270.,
            vec!["ochre-sands"],
        ),
        (
            Some("lichen-heath"),
            Brush::Land,
            520.,
            170.,
            vec!["lichen-heath"],
        ),
        (None, Brush::Land, 520., 100., vec![]),
        (
            Some("weathered-shale"),
            Brush::Land,
            520.,
            140.,
            vec!["weathered-shale"],
        ),
        (None, Brush::Erase, 520., 90., vec![]),
    ] {
        let mut stroke = stroke(brush);
        stroke.material = material.map(Arc::from);
        stroke.size = size;
        stroke.points = vec![Point::new(x, 390.)].into();
        let live = cache.render(&doc, Some((id, &stroke)));
        assert_eq!(at(&live, 520.), expected);
        assert_eq!(at(&live, 625.), ["ochre-sands"]);
        doc.layers[0].strokes.push(stroke);
        let cold = SceneCache::default().render(&doc, None);
        assert_eq!(at(&cold, 520.), expected);
        assert_eq!(
            live.shapes().collect::<Vec<_>>(),
            cold.shapes().collect::<Vec<_>>()
        );
        let released = cache.render(&doc, None);
        assert!(
            live.batches()
                .zip(released.batches())
                .all(|(a, b)| Arc::ptr_eq(a, b))
        );
    }
    let serialized = serde_json::to_string(&doc).unwrap();
    let reopened: Document = serde_json::from_str(&serialized).unwrap();
    assert_eq!(reopened, doc);
    assert_eq!(crate::scene::to_svg(&doc), crate::scene::to_svg(&reopened));
}

#[test]
fn water_finishes_survive_overlap_erasing_history_and_round_trips() {
    use crate::{
        brush_materials,
        editor::Editor,
        library::Camera,
        scene::{geometry::contains, thumbnail::thumbnail, to_svg},
    };

    let center = Point::new(520., 390.);
    let mut editor = Editor::blank();
    editor.set_atlas_style(crate::document::AtlasStyle::Classic);
    let id = editor.active_layer();
    let mut cache = SceneCache::default();
    let mut paint = |editor: &mut Editor, brush, material: Option<&str>, size| {
        let mut stroke = stroke(brush);
        stroke.material = material.map(Arc::from);
        stroke.size = size;
        stroke.strength = 1.;
        stroke.points = vec![center].into();
        let live = cache.render(editor.document(), Some((id, &stroke)));
        assert!(editor.commit_stroke(stroke));
        let cold = SceneCache::default().render(editor.document(), None);
        let released = cache.render(editor.document(), None);
        assert_eq!(
            live.shapes().collect::<Vec<_>>(),
            cold.shapes().collect::<Vec<_>>()
        );
        assert_eq!(
            released.shapes().collect::<Vec<_>>(),
            cold.shapes().collect::<Vec<_>>()
        );
        cold
    };
    let legacy = paint(&mut editor, Brush::Water, None, 370.);
    let legacy_svg = to_svg(editor.document());
    assert!(legacy_svg.contains("fill=\"#e6e5d3\""));
    assert!(
        legacy
            .shapes()
            .any(|s| matches!(s, Shape::Polygon { fill: 0xe6e5d3, .. }))
    );
    assert!(!legacy_svg.contains("water-material-"));

    paint(&mut editor, Brush::Water, Some("ocean"), 310.);
    paint(&mut editor, Brush::Water, Some("shallow-ocean"), 235.);
    let lake = paint(&mut editor, Brush::Water, Some("lake"), 150.);
    let material_at = |scene: &Scene, x| {
        let q = Point::new(x, center.y);
        scene
            .shapes()
            .filter_map(|shape| {
                let Shape::RegionSurface {
                    contours,
                    material: Some(index),
                    ..
                } = shape
                else {
                    return None;
                };
                let inside = contours.iter().filter(|ring| contains(ring, q)).count() % 2 == 1;
                inside.then_some(brush_materials::artwork_catalog()[*index].id.as_str())
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(material_at(&lake, 520.), ["lake"]);
    assert_eq!(material_at(&lake, 619.), ["shallow-ocean"]);
    assert_eq!(material_at(&lake, 659.), ["ocean"]);
    assert!(material_at(&lake, 689.).is_empty());

    let before_erase = editor.document().clone();
    let erased = paint(&mut editor, Brush::Erase, None, 70.);
    assert!(material_at(&erased, 520.).is_empty());
    assert_eq!(material_at(&erased, 575.), ["lake"]);
    assert!(erased.shapes().any(|s| matches!(
        s,
        Shape::Polygon { fill: 0x897e69, .. } | Shape::RegionSurface { fill: 0x897e69, .. }
    )));
    assert!(
        !erased
            .shapes()
            .any(|s| matches!(s, Shape::Polygon { fill: 0xf4ecd5, .. }))
    );

    let svg = to_svg(editor.document());
    for material in ["ocean", "shallow-ocean", "lake"] {
        assert!(svg.contains(&format!("fill=\"url(#water-material-{material})\"")));
    }
    assert_eq!(svg.matches("data:image/png;base64,").count(), 3);
    assert!(editor.undo());
    assert_eq!(editor.document(), &before_erase);
    let undone = SceneCache::default().render(editor.document(), None);
    assert_eq!(
        undone.shapes().collect::<Vec<_>>(),
        lake.shapes().collect::<Vec<_>>()
    );
    assert!(editor.redo());
    assert_eq!(to_svg(editor.document()), svg);

    let reopened: Document =
        serde_json::from_str(&serde_json::to_string(editor.document()).unwrap()).unwrap();
    assert_eq!(to_svg(&reopened), svg);
    let camera = Camera::for_document(&reopened);
    let preview = thumbnail(&reopened, camera).unwrap();
    assert_eq!(preview, thumbnail(editor.document(), camera).unwrap());
    assert_ne!(preview, thumbnail(&before_erase, camera).unwrap());
}

#[test]
fn water_cuts_a_shared_shoreline_live_and_later_land_reclaims_it() {
    use crate::{brush_materials, scene::geometry::contains};

    fn shoreline(scene: &Scene) -> Vec<&[Point]> {
        scene
            .shapes()
            .flat_map(|shape| match shape {
                Shape::Polygon {
                    points,
                    fill: 0x777d65,
                    ..
                } => std::slice::from_ref(points),
                Shape::RegionSurface {
                    contours,
                    fill: 0x777d65,
                    ..
                } => contours,
                _ => &[],
            })
            .map(Vec::as_slice)
            .collect()
    }

    let mut doc = Document::blank();
    doc.style = crate::document::AtlasStyle::Classic;
    let id = doc.layers[0].id;
    let mut land = stroke(Brush::Land);
    land.size = 270.;
    land.material = Some(Arc::from("ochre-sands"));
    doc.layers[0].strokes.push(land);
    let mut cache = SceneCache::default();
    let original = cache.render(&doc, None);
    let mut water = stroke(Brush::Water);
    water.size = 105.;
    water.material = Some(Arc::from("lake"));
    water.points = vec![Point::new(555., 390.)].into();

    let at = |scene: &Scene, x| {
        let q = Point::new(x, 390.);
        scene
            .shapes()
            .filter_map(|shape| match shape {
                Shape::Polygon { points, fill, .. } if contains(points, q) => Some((*fill, None)),
                Shape::RegionSurface {
                    contours,
                    material,
                    fill,
                    ..
                } if contours.iter().filter(|ring| contains(ring, q)).count() % 2 == 1 => Some((
                    *fill,
                    material.map(|i| brush_materials::artwork_catalog()[i].id.as_str()),
                )),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let check_bank = |scene: &Scene| {
        let coasts = shoreline(scene);
        let bank = coasts
            .iter()
            .flat_map(|coast| coast.iter().zip(coast.iter().cycle().skip(1)))
            .filter_map(|edge| {
                let (a, b) = edge;
                ((a.y > 390.) != (b.y > 390.))
                    .then(|| a.x + (b.x - a.x) * (390. - a.y) / (b.y - a.y))
            })
            .filter(|x| (470. ..540.).contains(x))
            .min_by(f32::total_cmp)
            .expect("shore crosses the middle of the cut");
        assert_eq!(at(scene, bank - 4.), [(0xf4ecd5, Some("ochre-sands"))]);
        assert_eq!(at(scene, bank + 4.), [(0xe6e5d3, Some("lake"))]);
        let q = Point::new(bank + 4., 390.);
        let wash: Vec<_> = scene
            .shapes()
            .filter_map(|shape| {
                let Shape::ShoreWash(wash) = shape else {
                    return None;
                };
                (wash
                    .contours
                    .iter()
                    .filter(|ring| contains(ring, q))
                    .count()
                    % 2
                    == 1)
                    .then(|| wash.vertex_color(q)[3])
            })
            .collect();
        assert_eq!(
            wash.len(),
            1,
            "a live lake or bay should have one shallows wash"
        );
        assert!(wash[0] > 0.);
    };

    let lake = cache.render(&doc, Some((id, &water)));
    check_bank(&lake);
    water.points.push(Point::new(720., 390.));
    let bay = cache.render(&doc, Some((id, &water)));
    check_bank(&bay);
    assert_eq!(at(&bay, 555.), [(0xe6e5d3, Some("lake"))]);
    assert!(cache.render(&doc, None) == original);

    doc.layers[0].strokes.push(water.clone());
    let released = cache.render(&doc, None);
    assert_material(&bay, &doc);
    assert!(bay.shapes().eq(released.shapes()));

    let mut reclaimed = water;
    reclaimed.brush = Brush::Land;
    reclaimed.material = None;
    let live = cache.render(&doc, Some((id, &reclaimed)));
    assert_eq!(at(&live, 555.), [(0xf4ecd5, None)]);
    doc.layers[0].strokes.push(reclaimed);
    assert_material(&live, &doc);
    assert!(live == cache.render(&doc, None));
}
