use super::{RegionAction, Selection, StyleAction};
use crate::app::state::Studio;
use elmapa::{
    document::{Brush, Point, Stroke},
    scene::{
        SceneCache,
        territories::{self, RegionView, Shoreline},
    },
};
use std::sync::Arc;

fn fixture() -> (Studio, Arc<Shoreline>, Point) {
    let mut studio = Studio::default();
    let island = Point::new(90., 280.);
    for path in [
        vec![Point::new(-300., 0.), Point::new(300., 0.)],
        vec![island],
    ] {
        assert!(studio.editor.commit_stroke(Stroke {
            creation_id: None,
            material: None,
            brush: Brush::Land,
            points: path.into(),
            size: 180.,
            strength: 1.,
            seed: 86,
        }));
    }
    let scene = SceneCache::default().render(studio.editor.document(), None);
    let shore = scene.shoreline().clone();
    studio.activate_regions();
    studio.regions.selection = Some(Selection::new(
        shore
            .island_at(Point::new(0., 0.))
            .expect("mainland")
            .clone(),
        None,
        shore.clone(),
    ));
    (studio, shore, island)
}

#[test]
fn split_preview_edits_only_the_draft_and_apply_is_one_undo_step() {
    let (mut studio, shore, island) = fixture();
    let original = studio.editor.document().clone();
    let revision = studio.editor.revision();
    studio.region_action(RegionAction::Split);
    assert_eq!(studio.regions.split_count(), Some(2));
    studio.region_action(RegionAction::SplitCount(1));
    let (view, washes) = studio.regions.split_preview().unwrap();
    assert_eq!(view.faces.len(), 3);
    assert_eq!(washes.len(), 3);
    assert_eq!(view.borders.len(), 2);
    assert!(
        view.faces
            .iter()
            .all(|face| !territories::contains(&face.polygon, island))
    );

    let span = &view.borders[0].visible[0];
    let before = span.clone();
    let a = span[0];
    let b = *span.last().unwrap();
    let center = Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    let position = Point::new(center.x + 24., center.y);
    let border =
        |studio: &Studio| studio.regions.split_preview().unwrap().0.borders[0].visible[0].clone();
    studio.region_down(center);
    studio.region_move(position);
    assert_ne!(border(&studio), before);
    assert_eq!(studio.editor.revision(), revision);
    studio.regions.cancel_gesture();
    assert_eq!(border(&studio), before);

    studio.region_down(center);
    studio.region_up(position, false);
    let adjusted = border(&studio);
    assert_ne!(adjusted, before);
    assert_eq!(studio.editor.document(), &original);
    studio.region_action(RegionAction::ApplySplit);
    assert!(studio.regions.split_preview().is_none());
    assert_eq!(studio.editor.revision(), revision + 1);
    let data = studio
        .regions_layer()
        .unwrap()
        .territories
        .as_deref()
        .unwrap();
    let applied = RegionView::resolve(data, &shore).unwrap();
    assert_eq!(applied.faces.len(), 3);
    assert_eq!(applied.borders[0].visible[0], adjusted);
    assert!(studio.editor.undo());
    assert_eq!(studio.editor.document(), &original);

    studio.regions.selection = Some(Selection::new(
        shore.island_at(Point::new(0., 0.)).unwrap().clone(),
        None,
        shore,
    ));
    studio.region_action(RegionAction::Split);
    assert!(studio.cancel_region_step());
    assert!(studio.regions.selection.is_some() && studio.regions.split_preview().is_none());
    assert_eq!(studio.editor.document(), &original);
}

#[test]
fn style_edits_belong_to_the_owner_and_survive_round_trip_and_split() {
    use elmapa::{
        scene::Shape,
        storage::{DocumentStore, LocalDocumentStore},
    };

    let (mut studio, shore, island) = fixture();
    let style = |action| RegionAction::Style(action);
    studio.region_action(RegionAction::Make);
    let owner = studio.regions.selection.as_ref().unwrap().owner.unwrap();
    let separate = Selection::new(
        shore.island_at(island).unwrap().clone(),
        None,
        shore.clone(),
    );
    studio.claim_region(separate, Some(owner));
    studio.region_action(style(StyleAction::Rename));
    studio.text_insert("The Jade Coast");
    studio.region_action(style(StyleAction::Apply));
    studio.region_action(style(StyleAction::CustomColor));
    studio.text_insert("#bad");
    studio.region_action(style(StyleAction::Apply));
    assert!(studio.regions.style.error.is_some());
    studio.region_action(style(StyleAction::CustomColor));
    studio.text_insert("#3D8476");
    studio.region_action(style(StyleAction::Apply));

    let (_, data, _) = studio.region_style_target().unwrap();
    assert_eq!(data.names[owner], "The Jade Coast");
    assert_eq!(data.styles[&owner].label, Some(true));
    assert_eq!(data.styles[&owner].color, Some(0x3d8476));
    let view = RegionView::resolve(data, &shore).unwrap();
    let washes = view.washes(&shore, data, elmapa::document::AtlasStyle::Classic);
    let jade_wash = |shape: &Shape| {
        matches!(
            shape,
            Shape::RegionSurface {
                fill: 0x3d8476,
                granulated: true,
                ..
            }
        )
    };
    assert_eq!(washes.len(), 2);
    assert!(washes.iter().all(jade_wash));
    let ink = view.shapes(&shore, data, elmapa::document::AtlasStyle::Classic);
    assert!(ink.len() > washes.len());
    assert!(
        ink[washes.len()..]
            .iter()
            .all(|shape| shape.bounds().unwrap().max.y < island.y)
    );

    let path =
        std::env::temp_dir().join(format!("elmapa-region-style-{}.elmapa", std::process::id()));
    let document = studio.editor.document();
    LocalDocumentStore.save(document, &path).unwrap();
    assert_eq!(LocalDocumentStore.load(&path).unwrap(), *document);
    std::fs::remove_file(path).unwrap();

    studio.region_action(style(StyleAction::ToggleLabel));
    let (_, data, _) = studio.region_style_target().unwrap();
    assert_eq!(
        view.shapes(&shore, data, elmapa::document::AtlasStyle::Classic)
            .len(),
        washes.len()
    );
    assert!(studio.editor.undo());
    studio.region_action(RegionAction::Split);
    assert!(
        studio
            .regions
            .split_preview()
            .unwrap()
            .1
            .iter()
            .any(jade_wash)
    );
    studio.region_action(RegionAction::ApplySplit);
    let data = studio
        .regions_layer()
        .unwrap()
        .territories
        .as_deref()
        .unwrap();
    assert_eq!(data.names[owner], "The Jade Coast");
    assert_eq!(data.styles[&owner].label, Some(true));
    assert_eq!(data.styles[&owner].color, Some(0x3d8476));
    assert!(!data.styles.contains_key(&(owner + 1)));
}

#[test]
fn split_replaces_only_the_selected_piece_of_an_existing_region() {
    let (mut studio, shore, island) = fixture();
    studio.region_action(RegionAction::Make);
    let owner = studio.regions.selection.as_ref().unwrap().owner.unwrap();
    let separate = Selection::new(
        shore.island_at(island).unwrap().clone(),
        None,
        shore.clone(),
    );
    studio.claim_region(separate, Some(owner));
    let original = studio.regions_layer().unwrap().territories.clone().unwrap();
    studio.region_action(RegionAction::Split);
    studio.region_action(RegionAction::SplitCount(1));
    studio.region_action(RegionAction::ApplySplit);
    let revised = studio.regions_layer().unwrap().territories.clone().unwrap();
    assert_eq!(original.areas[1], revised.areas[1]);
    assert_eq!(revised.names.len(), 3);
    let view = RegionView::resolve(&revised, &shore).unwrap();
    assert_eq!(view.faces.len(), 4);
    assert_eq!(
        view.faces.iter().filter(|face| face.owner == owner).count(),
        2
    );
    assert!(studio.editor.undo());
    assert_eq!(
        studio
            .regions_layer()
            .unwrap()
            .territories
            .as_ref()
            .unwrap(),
        &original
    );
}

#[test]
fn the_region_orbit_creates_and_reopens_a_child_layer() {
    use elmapa::document::RegionParent;

    let (mut studio, shore, island) = fixture();
    let anchor = Point::new(0., 0.);
    studio.region_action(RegionAction::Make);
    let parent = RegionParent {
        layer: studio.editor.active_layer(),
        owner: studio.regions.selection.as_ref().unwrap().owner.unwrap(),
    };
    let separate = Selection::new(
        shore.island_at(island).unwrap().clone(),
        None,
        shore.clone(),
    );
    studio.claim_region(separate, Some(parent.owner));
    studio.regions.style.anchor = Some(anchor);
    let before = studio.editor.document().clone();
    let action = studio
        .region_style_actions()
        .into_iter()
        .find(|item| item.title == "Split into child regions…")
        .and_then(|item| item.action);
    assert!(matches!(
        action,
        Some(crate::app::state::Action::Regions(
            RegionAction::ChildRegions
        ))
    ));

    studio.region_action(RegionAction::ChildRegions);
    assert_eq!(studio.split_parent_name(), Some("Region 1"));
    studio.region_action(RegionAction::SplitCount(1));
    let (preview, _) = studio.regions.split_preview().unwrap();
    assert_eq!(preview.faces.len(), 3);
    assert!(
        preview
            .faces
            .iter()
            .all(|face| !territories::contains(&face.polygon, island))
    );
    assert_eq!(studio.editor.document(), &before);
    assert!(studio.cancel_region_step());
    assert_eq!(studio.editor.document(), &before);
    assert!(studio.regions.split_preview().is_none());

    studio.region_action(RegionAction::ChildRegions);
    studio.region_action(RegionAction::SplitCount(1));
    studio.region_action(RegionAction::ApplySplit);
    let child = studio.editor.active_layer();
    let doc = studio.editor.document();
    assert_eq!(doc.layer(parent.layer), before.layer(parent.layer));
    assert_eq!(doc.child_regions(parent).unwrap().id, child);
    let countries = doc.region_data(child).unwrap();
    assert_eq!(countries.names.len(), 3);
    let ancestors = countries.ancestors(|id| doc.region_data(id)).unwrap();
    assert!(
        Shoreline::scoped(&shore, &ancestors)
            .unwrap()
            .island_at(island)
            .is_some()
    );
    assert!(doc.validate().is_ok());
    let created = doc.clone();
    assert!(studio.editor.undo());
    assert_eq!(studio.editor.document(), &before);
    assert!(studio.editor.redo());
    assert_eq!(studio.editor.document(), &created);

    studio.region_action(RegionAction::ParentLayer);
    assert_eq!(studio.editor.active_layer(), parent.layer);
    studio.regions.selection = Some(Selection::new(
        shore.island_at(anchor).unwrap().clone(),
        Some((
            elmapa::document::RegionAddress { area: 0, node: 0 },
            parent.owner,
        )),
        shore,
    ));
    studio.regions.style.anchor = Some(anchor);
    assert!(
        studio
            .region_style_actions()
            .iter()
            .any(|item| item.title == "Open child regions")
    );
    studio.region_action(RegionAction::ChildRegions);
    assert_eq!(studio.editor.active_layer(), child);
    assert_eq!(studio.editor.document(), &created);
}
