use super::{Action, Studio, Tool};
use crate::app::files;
use bevy::prelude::Vec2;
use elmapa::{
    biome_packs, brush_materials,
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
fn advanced_materials_share_selection_with_the_orbit_and_next_stroke() {
    let mut studio = editor();
    let mut jobs = files::Jobs::default();
    studio.act(Action::Advanced, &mut jobs);
    let point = Vec2::new(300., 300.);

    for brush in [Brush::Land, Brush::Water, Brush::Forest, Brush::Mountain] {
        studio.act(Action::Brush(brush), &mut jobs);
        let mut materials = brush_materials::for_brush(brush);
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
                .items(brush, studio.biome_pack)
                .iter()
                .any(|item| { item.active && item.action == Some(Action::BrushMaterial(index)) })
        );

        studio.open_brush_orbit(point);
        studio.act(Action::BrushMaterial(default_index), &mut jobs);
        assert_eq!(
            studio.brush_orbit.current_index(brush, studio.biome_pack),
            Some(default_index)
        );
        assert!(studio.brush_orbit.anchor.is_none());
    }
}

#[test]
fn switching_biomes_changes_only_authoring_defaults_and_choices() {
    let mut studio = editor();
    let mut jobs = files::Jobs::default();
    let desert = biome_packs::find("desert");
    let alpine = biome_packs::find("alpine");
    let (plain_land, _) = brush_materials::for_brush(Brush::Land).next().unwrap();
    let point = Vec2::new(500., 500.);

    studio.down(point);
    studio.up(point, false);
    studio.act(Action::Advanced, &mut jobs);
    studio.open_brush_orbit(point);
    studio.brush_orbit.page = 1;
    studio.dirty = false;
    let document = studio.editor.document().clone();
    let revision = studio.editor.revision();
    let scene_epoch = studio.scene.epoch();
    studio.act(Action::BiomePack(desert), &mut jobs);
    assert!(desert.is_some() && alpine.is_some());
    assert!(jobs.preferences);
    assert!(studio.advanced);
    assert!(studio.brush_orbit.anchor.is_none());
    assert_eq!(studio.brush_orbit.page, 0);
    assert!(!studio.dirty && studio.editor.can_undo());
    assert_eq!(*studio.editor.document(), document);
    assert_eq!(studio.editor.revision(), revision);
    assert_eq!(studio.scene.epoch(), scene_epoch);

    let desert_land: Vec<_> = biome_packs::materials(desert, Brush::Land).collect();
    let orbit: Vec<_> = studio
        .brush_orbit
        .items(Brush::Land, desert)
        .into_iter()
        .filter_map(|item| match item.action {
            Some(Action::BrushMaterial(index)) => Some(index),
            _ => None,
        })
        .collect();
    assert_eq!(
        orbit,
        desert_land
            .iter()
            .map(|(index, _)| *index)
            .collect::<Vec<_>>()
    );
    studio.act(Action::AdvancedBrushMaterial(plain_land), &mut jobs);
    assert_eq!(
        studio.brush_orbit.current_index(Brush::Land, desert),
        Some(desert_land[0].0)
    );
    studio.act(Action::AdvancedBrushMaterial(desert_land[1].0), &mut jobs);
    assert_eq!(
        studio.brush_orbit.current_index(Brush::Land, desert),
        Some(desert_land[1].0)
    );
    studio.act(Action::Brush(Brush::River), &mut jobs);
    studio.down(point);
    assert_eq!(
        studio.stroke.as_ref().unwrap().material.as_deref(),
        Some("oasis-water")
    );
    studio.up(point, false);

    studio.act(Action::BiomePack(alpine), &mut jobs);
    let material = |brush| studio.brush_orbit.stroke_material(brush, studio.biome_pack);
    assert_eq!(material(Brush::Land).as_deref(), Some("alpine-meadow"));
    assert_eq!(material(Brush::River).as_deref(), Some("glacial-water"));
    assert_eq!(material(Brush::Path), None);
    studio.act(Action::BiomePack(None), &mut jobs);
    assert_eq!(
        studio.brush_orbit.current_index(Brush::Land, None),
        Some(plain_land)
    );
    assert_eq!(studio.brush_orbit.stroke_material(Brush::River, None), None);
    assert_eq!(studio.editor.document().layers[0].strokes[0].material, None);
    assert_eq!(
        studio.editor.document().layers[0].strokes[1]
            .material
            .as_deref(),
        Some("oasis-water")
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
