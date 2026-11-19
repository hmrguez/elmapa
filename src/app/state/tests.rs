use super::{Action, Studio, Tool};
use crate::app::files;
use bevy::prelude::Vec2;
use elmapa::document::{Brush, Point};

fn editor() -> Studio {
    Studio {
        home: false,
        opening: None,
        ..Studio::default()
    }
}
#[test]
fn annotations_leave_regions_and_resume_on_the_artwork_layer() {
    use crate::app::regions::RegionAction;
    let mut studio = editor();
    let mut jobs = files::Jobs::default();
    let artwork = studio.editor.active_layer();
    studio.act(Action::Regions(RegionAction::NewLayer), &mut jobs);
    assert!(studio.regions.active);
    assert!(studio.regions_layer().is_some());
    studio.act(Action::TextTool, &mut jobs);
    assert_eq!(studio.tool, Tool::Text);
    assert_eq!(studio.editor.active_layer(), artwork);
    assert!(!studio.regions.active);
    studio.annotation_press(Point::new(400., 400.), false);
    studio.text_insert("Returning to artwork");
    studio.act(Action::PlaceLabel, &mut jobs);
    assert_eq!(
        studio
            .editor
            .document()
            .layer(artwork)
            .unwrap()
            .labels
            .len(),
        1
    );
}

#[test]
fn toolbar_action_commits_once_and_hidden_layers_reject_paint() {
    let mut s = editor();
    let mut jobs = files::Jobs::default();
    s.down(Vec2::new(300., 300.));
    s.moved(Vec2::new(360., 330.), false);
    s.act(Action::Brush(Brush::Forest), &mut jobs);
    assert_eq!(s.editor.stroke_count(), 1);
    assert!(s.stroke.is_none());
    s.act(Action::Undo, &mut jobs);
    assert_eq!(s.editor.stroke_count(), 0);
    s.act(Action::Visibility(s.editor.active_layer()), &mut jobs);
    s.down(Vec2::new(300., 300.));
    assert!(s.stroke.is_none());
}
#[test]
fn advanced_materials_share_selection_with_the_orbit_and_next_stroke() {
    let mut studio = editor();
    let mut jobs = files::Jobs::default();
    studio.act(Action::Advanced, &mut jobs);
    let point = Vec2::new(300., 300.);

    for brush in [Brush::Land, Brush::Water, Brush::Forest, Brush::Mountain] {
        studio.act(Action::Brush(brush), &mut jobs);
        let mut materials = elmapa::brush_materials::for_brush(brush);
        let (default_index, _) = materials.next().unwrap();
        let (index, material) = materials.next().unwrap();

        let strokes = studio.editor.stroke_count();
        studio.act(Action::AdvancedBrushMaterial(index), &mut jobs);
        assert!(studio.advanced);
        assert_eq!(studio.editor.stroke_count(), strokes);
        studio.down(point);
        assert_eq!(
            studio.stroke.as_ref().unwrap().material.as_deref(),
            Some(material.id.as_str())
        );
        studio.up(point, false);
        assert!(
            studio
                .brush_orbit
                .items(brush)
                .iter()
                .any(|item| { item.active && item.action == Some(Action::BrushMaterial(index)) })
        );

        studio.open_brush_orbit(point);
        studio.act(Action::BrushMaterial(default_index), &mut jobs);
        assert_eq!(studio.brush_orbit.current_index(brush), Some(default_index));
        assert!(studio.brush_orbit.anchor.is_none());
    }
}
#[test]
fn river_pointer_down_requests_material_animation_and_undo_clears_it() {
    let mut studio = editor();
    studio.brush = Brush::River;
    studio.down(Vec2::new(500., 500.));
    assert!(studio.animate_new);
    assert!(studio.stroke.is_some());
    studio.act(Action::Undo, &mut files::Jobs::default());
    assert!(!studio.animate_new);
}
#[test]
fn releasing_on_ui_finishes_without_extending_under_the_panel() {
    let mut s = editor();
    s.down(Vec2::new(300., 300.));
    let p = s.stroke.as_ref().unwrap().points[0];
    s.moved(Vec2::new(1300., 60.), true);
    s.up(Vec2::new(1300., 60.), true);
    assert_eq!(s.editor.stroke_count(), 1);
    assert_eq!(s.editor.document().layers[0].strokes[0].points.len(), 1);
    assert_eq!(s.editor.document().layers[0].strokes[0].points[0], p);
}
