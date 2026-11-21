//! Shared fixtures for cached scene regression tests.
mod composition;
mod live_material;
mod retention;
mod terrain_finishes;
mod terrain_tiles;

use super::{Scene, SceneCache};
use crate::{
    document::{AtlasStyle, Brush, Document, Point, Stroke},
    scene::build_scene,
};

fn stroke(brush: Brush) -> Stroke {
    Stroke {
        creation_id: None,
        material: None,
        brush,
        points: vec![Point::new(520.0, 390.0)].into(),
        size: 130.0,
        strength: 0.85,
        seed: 421,
    }
}

fn assert_material(scene: &Scene, doc: &Document) {
    let actual: Vec<_> = scene.shapes().cloned().collect();
    let expected = build_scene(doc);
    assert_eq!(
        actual.len(),
        expected.len(),
        "cached shape count differs from export geometry"
    );
    for (mut a, mut b) in actual.into_iter().zip(expected) {
        let alpha_a = std::mem::replace(a.opacity_mut(), 1.);
        let alpha_b = std::mem::replace(b.opacity_mut(), 1.);
        assert!(a == b, "cached geometry must match a full rebuild exactly");
        assert!(
            (alpha_a - alpha_b).abs() < 0.000001,
            "per-shape opacity changed"
        );
    }
}

#[test]
fn child_borders_and_washes_follow_a_parent_edit_in_the_retained_scene() {
    use crate::{
        document::{RegionLayer, RegionNode, RegionParent},
        editor::Editor,
        scene::{
            SceneCache,
            territories::{RegionView, Shoreline, contains, pigment},
        },
    };

    let mut editor = Editor::new(Document::blank()).unwrap();
    assert!(editor.commit_stroke(Stroke {
        creation_id: None,
        brush: Brush::Land,
        material: None,
        points: vec![Point::new(-140., 0.), Point::new(140., 0.)].into(),
        size: 200.,
        strength: 1.,
        seed: 41,
    }));
    let mut cache = SceneCache::default();
    let shore = cache.render(editor.document(), None).shoreline().clone();
    let box_land = |right| {
        vec![vec![
            [-100., -45.],
            [right, -45.],
            [right, 45.],
            [-100., 45.],
        ]]
    };
    let vertical = |x| vec![Point::new(x, -80.), Point::new(x, 80.)];
    let mut continents = RegionLayer::default();
    let first = continents.new_region();
    let second = continents.new_region();
    let land = continents.claim(box_land(100.), first);
    assert!(continents.split(land, vertical(60.), second));
    let view = RegionView::resolve(&continents, &shore).unwrap();
    let owner = view
        .faces
        .iter()
        .find(|face| contains(&face.polygon, Point::new(-70., 0.)))
        .unwrap()
        .owner;
    editor
        .set_regions(None, continents.clone(), &shore)
        .unwrap();
    let parent_layer = editor.active_layer();
    let parent = RegionParent {
        layer: parent_layer,
        owner,
    };

    let mut countries = RegionLayer {
        parent: Some(parent),
        ..RegionLayer::default()
    };
    let first = countries.new_region();
    let second = countries.new_region();
    let land = countries.claim(box_land(60.), first);
    assert!(countries.split(
        land,
        vec![Point::new(-120., 0.), Point::new(100., 0.)],
        second
    ));
    let child_ink = pigment(&countries, first, editor.document().style);
    editor.set_regions(None, countries.clone(), &shore).unwrap();
    let child_layer = editor.active_layer();
    assert!(editor.set_regions(None, countries.clone(), &shore).is_err());
    assert!(!editor.remove_layer(parent_layer));
    let check_border = |scene: &crate::scene::Scene, end: f32, wash_end: f32| {
        let view = scene.region_view(child_layer).unwrap();
        let border = &view.borders[0].visible[0];
        let right = border
            .iter()
            .map(|point| point.x)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((right - end).abs() < 0.01, "border ended at {right}");
        let right = scene.shapes()
            .filter(|shape| matches!(shape, crate::scene::Shape::RegionSurface { fill, granulated: true, .. } if *fill == child_ink))
            .filter_map(crate::scene::Shape::bounds)
            .map(|bounds| bounds.max.x)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((right - wash_end).abs() < 0.01, "wash ended at {right}");
    };
    check_border(&cache.render(editor.document(), None), 60., 60.);

    let RegionNode::Cut { path, .. } = &mut continents.areas[0].nodes[0] else {
        panic!("continental border");
    };
    path.insert(1, Point::new(-15., 0.));
    editor
        .set_regions(Some(parent_layer), continents, &shore)
        .unwrap();
    check_border(&cache.render(editor.document(), None), -15., 27.1875);
    assert_eq!(
        &**editor.document().region_data(child_layer).unwrap(),
        &countries
    );
    let ancestors = countries
        .ancestors(|id| editor.document().region_data(id))
        .unwrap();
    let scope = Shoreline::scoped(&shore, &ancestors).unwrap();
    assert!(scope.island_at(Point::new(0., 3.)).is_none());
    assert!(scope.island_at(Point::new(-30., 15.)).is_some());

    assert!(editor.move_layer(child_layer, -1));
    let reopened: Document =
        serde_json::from_slice(&serde_json::to_vec(editor.document()).unwrap()).unwrap();
    reopened.validate().unwrap();
    check_border(
        &SceneCache::default().render(&reopened, None),
        -15.,
        27.1875,
    );
    let mut invalid = reopened.clone();
    let child = invalid
        .layer_mut(child_layer)
        .unwrap()
        .territories
        .as_mut()
        .unwrap();
    std::sync::Arc::make_mut(child).parent = Some(RegionParent {
        layer: child_layer,
        owner: first,
    });
    assert!(invalid.validate().is_err());
}

#[test]
fn woodland_style_round_trips_and_keeps_saved_relief_anchors() {
    let mut classic = crate::document::Document::demo();
    classic.style = AtlasStyle::Classic;
    let mut explicit = classic.layers[1].strokes[0].clone();
    explicit.material = Some("weathered-highlands".into());
    classic.layers[1].strokes.push(explicit);
    let old_file = serde_json::to_string(&classic).unwrap();
    assert!(!old_file.contains("\"style\""));
    let mut editor = crate::editor::Editor::new(serde_json::from_str(&old_file).unwrap()).unwrap();
    let mut cache = SceneCache::default();
    let before = cache.render(editor.document(), None);
    assert_eq!(before.style(), AtlasStyle::Classic);
    let classic_svg = crate::scene::to_svg(editor.document());

    editor.set_atlas_style(AtlasStyle::Woodland);
    cache.begin_generation();
    let after = cache.render(editor.document(), None);
    assert_eq!(after.style(), AtlasStyle::Woodland);
    assert!(after.forest_trees().count() > 0);
    assert!(
        before
            .forest_trees()
            .all(|tree| after.forest_trees().any(|same| same == tree))
    );
    assert!(before.mountain_parts().count() > 0);
    assert!(before.mountain_parts().all(|part| {
        after.mountain_parts().any(|same| {
            part.id == same.id && crate::scene::geometry::distance(part.base, same.base) < 0.001
        })
    }));
    cache.finish_generation(false);
    assert!(cache.committed_scene() == before);
    let retried = cache.render(editor.document(), None);
    assert_eq!(retried.style(), after.style());
    assert!(retried.shapes().eq(after.shapes()));
    assert_eq!(editor.document().layers, classic.layers);
    let reopened: crate::document::Document =
        serde_json::from_slice(&serde_json::to_vec(editor.document()).unwrap()).unwrap();
    assert_eq!(&reopened, editor.document());
    let woodland_svg = crate::scene::to_svg(&reopened);
    assert_ne!(woodland_svg, classic_svg);
    assert!(woodland_svg.contains("#777f5c"));
    editor.undo();
    assert_eq!(crate::scene::to_svg(editor.document()), classic_svg);
    editor.redo();
    assert_eq!(crate::scene::to_svg(editor.document()), woodland_svg);
}
