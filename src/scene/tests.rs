use super::*;
use super::{
    geometry::{area, p},
    terrain::{contours, region_field},
};
use crate::document::{Brush, Document, Stroke};

#[test]
fn export_is_deterministic() {
    let doc = Document::demo();
    assert_eq!(to_svg(&doc), to_svg(&doc));
}
#[test]
fn hidden_layers_have_no_geometry() {
    let mut doc = Document::demo();
    for layer in &mut doc.layers {
        layer.visible = false;
    }
    let hidden = to_svg(&doc);
    doc.layers.clear();
    assert_eq!(hidden, to_svg(&doc));
}
#[test]
fn eraser_only_removes_earlier_paint() {
    let mut doc = Document::blank();
    let layer = &mut doc.layers[0];
    let stroke = |brush| Stroke {
        creation_id: None,
        material: None,
        brush,
        points: vec![p(200.0, 200.0)].into(),
        size: 100.0,
        strength: 1.0,
        seed: 1,
    };
    layer.strokes = vec![stroke(Brush::Land), stroke(Brush::Erase)].into();
    assert!(region_field(p(200.0, 200.0), layer, Brush::Land) < 0.0);
    layer.strokes.push(stroke(Brush::Land));
    assert!(region_field(p(200.0, 200.0), layer, Brush::Land) > 0.0);
}

#[test]
fn strength_changes_region_coverage_and_zero_adds_no_terrain() {
    let mut doc = Document::blank();
    let empty = to_svg(&doc);
    for brush in [Brush::Land, Brush::Water] {
        doc.layers[0].strokes = vec![Stroke {
            creation_id: None,
            material: None,
            brush,
            points: vec![p(200.0, 200.0)].into(),
            size: 150.0,
            strength: 1.0,
            seed: 8,
        }]
        .into();
        let strong = contours(&doc.layers[0], brush);
        let strong_svg = to_svg(&doc);
        doc.layers[0].strokes[0].strength = 0.16;
        let weak = contours(&doc.layers[0], brush);
        assert!(area(&strong[0]).abs() > area(&weak[0]).abs() * 2.0);
        assert_ne!(strong_svg, to_svg(&doc));
        doc.layers[0].strokes[0].strength = 0.0;
        assert_eq!(empty, to_svg(&doc));
    }
}

#[test]
fn clicking_every_paint_brush_produces_geometry() {
    let mut doc = Document::blank();
    let empty = to_svg(&doc);
    for brush in [
        Brush::Land,
        Brush::Water,
        Brush::Forest,
        Brush::Mountain,
        Brush::River,
        Brush::Path,
    ] {
        doc.layers[0].strokes = vec![Stroke {
            creation_id: None,
            material: None,
            brush,
            points: vec![p(200.0, 200.0)].into(),
            size: 90.0,
            strength: 1.0,
            seed: 12,
        }]
        .into();
        assert_ne!(empty, to_svg(&doc), "{} click must paint", brush.label());
    }
}

#[test]
fn distant_islands_keep_local_detail_in_every_direction() {
    let mut doc = Document::blank();
    for brush in [Brush::Land, Brush::Water, Brush::Forest] {
        doc.layers[0].strokes = [p(-50000., -70000.), p(80000., 90000.)]
            .into_iter()
            .map(|q| Stroke {
                creation_id: None,
                material: None,
                brush,
                points: vec![q].into(),
                size: 90.,
                strength: 1.,
                seed: 12,
            })
            .collect();
        let loops = contours(&doc.layers[0], brush);
        assert_eq!(loops.len(), 2, "{} must render both islands", brush.label());
        for points in loops {
            assert!(area(&points).abs() > 1000.);
            let minx = points.iter().map(|q| q.x).fold(f32::INFINITY, f32::min);
            let maxx = points.iter().map(|q| q.x).fold(f32::NEG_INFINITY, f32::max);
            assert!(maxx - minx < 200.);
        }
        let scene = build_scene(&doc);
        assert!(!scene.is_empty());
        assert!(
            scene.len() < 20_000,
            "empty space must not generate terrain"
        );
    }
}

#[test]
fn svg_frames_and_preserves_artwork_beyond_all_former_edges() {
    let mut doc = Document::blank();
    for brush in Brush::ALL.into_iter().filter(|b| *b != Brush::Erase) {
        doc.layers[0].strokes = vec![Stroke {
            creation_id: None,
            material: None,
            brush,
            points: vec![p(-4000., 9000.)].into(),
            size: 90.,
            strength: 1.,
            seed: 12,
        }]
        .into();
        doc.validate().unwrap();
        let bounds = doc.content_bounds();
        let scene = build_scene(&doc);
        assert!(
            !scene.is_empty(),
            "{} must paint outside the page",
            brush.label()
        );
        for shape in scene {
            assert!(shape.points().all(|q| q.x > bounds.min.x
                && q.x < bounds.max.x
                && q.y > bounds.min.y
                && q.y < bounds.max.y));
        }
        let svg = to_svg(&doc);
        assert!(svg.contains(&format!(
            "viewBox=\"{} {} {} {}\"",
            bounds.min.x,
            bounds.min.y,
            bounds.width(),
            bounds.height()
        )));
        let reopened: Document =
            serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(svg, to_svg(&reopened));
    }
}
