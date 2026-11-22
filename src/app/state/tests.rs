use super::{Action, Studio, Tool};
use crate::app::files;
use bevy::prelude::Vec2;
use elmapa::{
    biome_packs,
    document::{Brush, Point},
};

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
fn wet_paint_follows_the_gesture_and_clears_on_edit_or_reduced_motion() {
    let mut studio = editor();
    let mut jobs = files::Jobs::default();
    let point = Vec2::new(300., 300.);

    studio.down(point);
    let live = studio.wet_paint();
    assert!(live.is_some());
    studio.up(point, false);
    assert_eq!(studio.wet_paint(), live);
    studio.act(Action::Undo, &mut jobs);
    assert!(studio.wet_paint().is_none());
    studio.act(Action::Redo, &mut jobs);
    assert!(studio.wet_paint().is_none());

    studio.act(Action::Brush(Brush::Water), &mut jobs);
    studio.down(point);
    assert!(studio.wet_paint().is_some());
    studio.cancel_canvas_interaction();
    assert!(studio.wet_paint().is_none());
    studio.down(point);
    studio.up(point, false);
    assert!(studio.wet_paint().is_some());
    studio.act(Action::Motion, &mut jobs);
    assert!(studio.wet_paint().is_none());
    studio.down(point);
    assert!(studio.stroke.is_some());
    assert!(studio.wet_paint().is_none());
}
#[test]
fn shader_toggle_suppresses_wet_paint_without_changing_reduced_motion() {
    let mut studio = editor();
    let mut jobs = files::Jobs::default();
    let point = Vec2::new(300., 300.);
    studio.down(point);
    studio.up(point, false);
    assert!(studio.wet_paint().is_some());

    studio.act(Action::ShaderEffects, &mut jobs);
    assert!(jobs.preferences);
    assert!(!studio.reduced);
    assert!(studio.wet_paint().is_none());

    studio.down(point);
    assert!(studio.wet_paint().is_none());
    studio.up(point, false);
    assert_eq!(studio.editor.stroke_count(), 2);

    studio.act(Action::ShaderEffects, &mut jobs);
    studio.down(point);
    assert!(studio.wet_paint().is_some());
}

#[test]
fn the_brush_orbit_changes_only_the_next_strokes_of_the_active_brush() {
    let mut studio = editor();
    let mut jobs = files::Jobs::default();
    let desert = biome_packs::find("desert").unwrap();
    let alpine = biome_packs::find("alpine").unwrap();
    let (dunes, _) = biome_packs::materials(Some(desert), Brush::Land)
        .next()
        .unwrap();
    let (glacial, _) = biome_packs::materials(Some(alpine), Brush::Water)
        .next()
        .unwrap();
    let point = Vec2::new(500., 500.);
    let landmark = studio.placement_landmark();

    studio.down(point);
    studio.up(point, false);
    let document = studio.editor.document().clone();
    let revision = studio.editor.revision();
    studio.act(Action::Advanced, &mut jobs);
    studio.act(Action::OpenBrushOrbit, &mut jobs);
    studio.act(Action::BrushOrbitEnter(0, desert), &mut jobs);
    assert_eq!(studio.brush_orbit.navigation.path, [desert]);
    studio.act(Action::BrushMaterial(Some(dunes)), &mut jobs);
    assert!(studio.brush_orbit.anchor.is_none());
    assert_eq!(studio.brush_orbit.current_index(Brush::Land), Some(dunes));
    assert_eq!(studio.placement_landmark(), landmark);
    assert_eq!(studio.editor.revision(), revision);
    assert_eq!(*studio.editor.document(), document);

    studio.act(Action::Brush(Brush::River), &mut jobs);
    assert_eq!(studio.brush_orbit.stroke_material(Brush::River), None);
    studio.open_brush_orbit(point);
    studio.act(Action::BrushOrbitEnter(0, desert), &mut jobs);
    studio.act(Action::BrushOrbitEnter(0, alpine), &mut jobs);
    assert_eq!(studio.brush_orbit.navigation.path, [alpine]);
    studio.act(Action::BrushMaterial(Some(glacial)), &mut jobs);
    studio.down(point);
    assert_eq!(
        studio.stroke.as_ref().unwrap().material.as_deref(),
        Some("glacial-water")
    );
    studio.up(point, false);
    assert_ne!(
        studio.brush_orbit.current_index(Brush::Water),
        Some(glacial)
    );
    assert_eq!(studio.brush_orbit.current_index(Brush::Land), Some(dunes));

    studio.open_brush_orbit(point);
    studio.act(Action::BrushOrbitEnter(0, 0), &mut jobs);
    studio.act(Action::BrushMaterial(None), &mut jobs);
    assert_eq!(studio.brush_orbit.stroke_material(Brush::River), None);
    assert_eq!(studio.editor.document().layers[0].strokes[0].material, None);
    assert_eq!(
        studio.editor.document().layers[0].strokes[1]
            .material
            .as_deref(),
        Some("glacial-water")
    );
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
