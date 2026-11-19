use super::*;
use crate::app::{
    files::Jobs,
    lettering::Field,
    ui::{
        ChromeMotion, Hit, ScrollOffsets, assets::UiAssets, landmark_selection_follow, rebuild,
        text_selection_follow,
    },
};

fn app() -> App {
    let mut app = App::new();
    let camera = app.world_mut().spawn_empty().id();
    let mut studio = Studio {
        home: false,
        disabled_landmark_packs: elmapa::scene::landmark_catalog()
            .packs
            .iter()
            .filter(|p| p.id != "elmapa.basic")
            .map(|p| p.id.clone())
            .collect(),
        reduced: true,
        ..default()
    };
    studio.viewport.resolve(studio.bounds);
    let id = studio
        .editor
        .add_label(elmapa::document::Label {
            position: studio.map_point(studio.bounds.center()),
            ..default()
        })
        .unwrap();
    studio.tool = crate::app::state::Tool::Select;
    studio.text.selected = Some(id);
    app.insert_resource(studio)
        .insert_resource(UiAssets {
            camera,
            icons: HashMap::new(),
            landmark_ghosts: default(),
            brush_swatches: HashMap::new(),
            avenir: default(),
            baskerville: default(),
            didot: default(),
        })
        .init_resource::<Time<Real>>()
        .init_resource::<Jobs>()
        .init_resource::<ChromeMotion>()
        .init_resource::<ScrollOffsets>()
        .add_systems(
            Update,
            (
                rebuild,
                text_selection_follow,
                landmark_selection_follow,
                radial_follow,
            )
                .chain(),
        );
    app
}
fn bubbles(app: &mut App) -> HashMap<(usize, usize), Vec2> {
    let world = app.world_mut();
    world
        .query::<(&RadialElement, &Node)>()
        .iter(world)
        .filter_map(|(element, node)| {
            let Element::Bubble { index, depth } = element.kind else {
                return None;
            };
            let (Val::Px(x), Val::Px(y)) = (node.left, node.top) else {
                panic!("Bubble must have a screen position");
            };
            Some(((depth, index), Vec2::new(x, y)))
        })
        .collect()
}
fn act(app: &mut App, action: Action) {
    app.world_mut()
        .resource_mut::<Studio>()
        .act(action, &mut Jobs::default());
    app.update();
}
#[test]
fn landmark_orbit_flattens_one_pack_and_nests_multiple_packs() {
    let mut app = app();
    act(&mut app, Action::LandmarkTool);
    assert!(bubbles(&mut app).is_empty());
    {
        let mut studio = app.world_mut().resource_mut::<Studio>();
        let point = Vec2::new(700., 510.);
        studio.down(point);
        studio.up(point, false);
    }
    app.update();

    let landmark_bubbles = |app: &mut App| {
        let world = app.world_mut();
        world
            .query::<(&RadialElement, &Hit)>()
            .iter(world)
            .filter_map(|(element, hit)| match element.kind {
                Element::Bubble { depth, .. } => Some((depth, hit.action.unwrap())),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let catalog = elmapa::scene::landmark_catalog();
    let basic_pack = catalog
        .packs
        .iter()
        .position(|p| p.id == "elmapa.basic")
        .unwrap();
    let basic_count = catalog
        .items
        .iter()
        .filter(|item| item.pack == basic_pack)
        .count();
    let visible = landmark_bubbles(&mut app);
    assert_eq!(visible.len(), basic_count + 4);
    assert_eq!(
        visible
            .iter()
            .filter(|(depth, action)| { *depth == 0 && matches!(action, Action::LandmarkKind(_)) })
            .count(),
        basic_count,
    );

    let other_pack = (0..catalog.packs.len())
        .find(|&pack| pack != basic_pack)
        .unwrap();
    act(&mut app, Action::LandmarkPack(other_pack));
    let visible = landmark_bubbles(&mut app);
    assert_eq!(visible.len(), 6);
    assert!(
        !visible
            .iter()
            .any(|(_, action)| matches!(action, Action::LandmarkKind(_)))
    );
    let branch = usize::from(basic_pack < other_pack);
    act(&mut app, Action::LandmarkEnter(0, branch));
    let visible = landmark_bubbles(&mut app);
    let options: Vec<_> = visible
        .iter()
        .filter_map(|(depth, action)| match action {
            Action::LandmarkKind(index) => {
                assert_eq!(*depth, 1);
                Some(*index)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        options.len(),
        catalog
            .items
            .iter()
            .filter(|item| item.pack == other_pack)
            .count()
    );
    assert!(
        options
            .iter()
            .all(|&index| catalog.items[index].pack == other_pack)
    );
    assert!(visible.contains(&(1, Action::LandmarkBack)));

    act(&mut app, Action::LandmarkKind(options[0]));
    assert_eq!(
        app.world()
            .resource::<Studio>()
            .current_landmark()
            .unwrap()
            .kind,
        catalog.items[options[0]].kind,
    );
    act(&mut app, Action::LandmarkBack);
    assert_eq!(landmark_bubbles(&mut app).len(), 6);
}

#[test]
fn draft_orbit_styles_labels_live_and_select_owns_object_actions() {
    let mut app = app();
    act(&mut app, Action::TextTool);
    assert!(bubbles(&mut app).is_empty());
    let point = Vec2::new(400., 400.);
    let click = |app: &mut App| {
        let mut studio = app.world_mut().resource_mut::<Studio>();
        studio.down(point);
        studio.up(point, false);
    };
    click(&mut app);
    app.update();
    assert_eq!(bubbles(&mut app).len(), 4);
    {
        let world = app.world_mut();
        assert!(world.resource::<Studio>().text.selected.is_none());
        assert!(world.resource::<Studio>().text.edit.is_some());
        assert_eq!(
            world
                .query::<&crate::app::ui::TextCard>()
                .iter(world)
                .count(),
            0
        );
        assert_eq!(
            world
                .query::<&crate::app::ui::overlays::text::InlineNameEdit>()
                .iter(world)
                .count(),
            1
        );
        assert!(world.query::<&Hit>().iter(world).all(|hit| {
            !matches!(hit.action, Some(Action::TextDuplicate | Action::TextDelete))
        }));
        world.resource_mut::<Studio>().text_insert("Stillwater");
    }
    let revision = app.world().resource::<Studio>().editor.revision();
    act(&mut app, Action::RadialEnter(0, 0));
    assert_eq!(bubbles(&mut app).len(), 8);
    act(
        &mut app,
        Action::TextFont(elmapa::document::LabelFont::Didot),
    );
    act(&mut app, Action::RadialEnter(0, 2));
    act(&mut app, Action::TextField(Field::Size));
    act(&mut app, Action::TextStep(Field::Size, 4.));
    let studio = app.world().resource::<Studio>();
    let draft = studio.draft_label().unwrap();
    assert_eq!(draft.text, "Stillwater");
    assert_eq!(draft.font, elmapa::document::LabelFont::Didot);
    assert_eq!(draft.size, 36.);
    assert_eq!(studio.editor.revision(), revision);
    act(&mut app, Action::PlaceLabel);
    assert!(bubbles(&mut app).is_empty());
    assert!(app.world().resource::<Studio>().text.edit.is_none());
    act(&mut app, Action::SelectTool);
    click(&mut app);
    app.update();
    assert!(app.world().resource::<Studio>().text.selected.is_some());
    assert_eq!(bubbles(&mut app).len(), 5);
}
#[test]
fn typing_a_selected_name_shows_an_inline_caret_without_a_name_card() {
    use crate::app::ui::overlays::text::InlineNameEdit;
    let mut app = app();
    let id = app.world().resource::<Studio>().text.selected.unwrap();
    app.world_mut()
        .resource_mut::<Studio>()
        .text_insert("North Watch");
    app.update();

    let world = app.world_mut();
    assert_eq!(
        world
            .query::<&crate::app::ui::TextCard>()
            .iter(world)
            .count(),
        0
    );
    assert_eq!(world.query::<&InlineNameEdit>().iter(world).count(), 1);
    let studio = world.resource::<Studio>();
    assert_eq!(studio.current_label().text, "North Watch");
    assert_eq!(studio.editor.label(id).unwrap().text, "Place name");

    world.resource_mut::<Studio>().commit_text_field();
    app.update();
    assert_eq!(
        app.world()
            .resource::<Studio>()
            .editor
            .label(id)
            .unwrap()
            .text,
        "North Watch"
    );
}

#[test]
fn text_orbit_places_delete_and_advanced_typography_on_the_first_ring() {
    let mut app = app();
    app.update();
    let root = bubbles(&mut app);
    assert_eq!(root.len(), 5);
    let world = app.world_mut();
    let root_actions: HashMap<_, _> = world
        .query::<(&RadialElement, &Hit)>()
        .iter(world)
        .filter_map(|(element, hit)| match element.kind {
            Element::Bubble { index, depth: 0 } => Some((index, hit.action.unwrap())),
            _ => None,
        })
        .collect();
    assert_eq!(root_actions[&3], Action::TextDelete);
    assert_eq!(root_actions[&4], Action::RadialEnter(0, 4));

    act(&mut app, root_actions[&4]);
    let expanded = bubbles(&mut app);
    assert_eq!(expanded.len(), 11);
    assert!(expanded.keys().all(|(depth, _)| *depth <= 1));
    for (key, position) in &root {
        assert_eq!(expanded.get(key), Some(position));
    }
    assert!((expanded[&(0, 0)].x + 18.).abs() < 0.001);
    assert!((expanded[&(1, 0)].y - expanded[&(0, 0)].y + 46.).abs() < 0.001);

    act(&mut app, root_actions[&4]);
    assert_eq!(bubbles(&mut app), root);
}
#[test]
fn advanced_stepper_stays_open_across_repeated_steps() {
    let mut app = app();
    act(&mut app, Action::RadialEnter(0, 4));
    let selected = app.world().resource::<Studio>().text.selected;
    act(&mut app, Action::TextField(Field::Spacing));
    act(&mut app, Action::TextStep(Field::Spacing, 0.5));
    act(&mut app, Action::TextStep(Field::Spacing, 0.5));
    let studio = app.world().resource::<Studio>();
    assert_eq!(studio.current_label().spacing, 1.);
    assert_eq!(studio.text.control, Some(Field::Spacing));
    assert_eq!(studio.text.selected, selected);
    assert_eq!(studio.text.radial.path, vec![4]);
    act(&mut app, Action::RadialBack);
    assert!(app.world().resource::<Studio>().text.control.is_none());
}
