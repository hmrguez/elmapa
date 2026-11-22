use super::*;
use crate::app::state::Appearance;
use assets::UiAssets;
use std::collections::HashMap;

fn ui_app(studio: Studio) -> App {
    let mut app = App::new();
    let camera = app.world_mut().spawn_empty().id();
    app.insert_resource(studio)
        .insert_resource(UiAssets {
            camera,
            display_camera: camera,
            icons: HashMap::new(),
            tool_symbols: Handle::default(),
            tool_paper: Handle::default(),
            landmark_ghosts: default(),
            brush_swatches: HashMap::new(),
            avenir: default(),
            baskerville: default(),
            didot: default(),
        })
        .init_resource::<Jobs>()
        .init_resource::<VideoExport>()
        .init_resource::<ChromeMotion>()
        .init_resource::<ScrollOffsets>()
        .add_systems(Update, rebuild);
    app
}

#[test]
fn appearance_switch_rebuilds_the_same_tools_with_the_chosen_faces() {
    use elmapa::document::Brush;

    let mut app = ui_app(Studio {
        home: false,
        menu: true,
        ..default()
    });
    let tools = |world: &mut World| {
        use crate::app::brush_motion::SelectedTool;
        let mut tools: Vec<_> = world
            .query::<(&Hit, Option<&ImageNode>, Option<&SelectedTool>)>()
            .iter(world)
            .filter_map(|(hit, face, selected)| match hit.action? {
                action @ (Action::Brush(_)
                | Action::SelectTool
                | Action::TextTool
                | Action::LandmarkTool
                | Action::Regions(crate::app::regions::RegionAction::Tool)) => {
                    Some((hit.order, action, face.is_some(), selected.is_some()))
                }
                _ => None,
            })
            .collect();
        tools.sort_by_key(|(order, ..)| *order);
        tools
            .into_iter()
            .map(|(_, action, paper, selected)| (action, paper, selected))
            .collect::<Vec<_>>()
    };
    let click = |app: &mut App, action| {
        app.world_mut()
            .resource_scope(|world, mut studio: Mut<Studio>| {
                studio.act(action, &mut world.resource_mut::<Jobs>());
            });
        app.update();
    };

    app.update();
    let holo = tools(app.world_mut());
    assert_eq!(holo.len(), 11);
    assert!(holo.iter().all(|&(_, paper, _)| !paper));
    assert!(
        holo.iter()
            .any(|&(action, _, selected)| action == Action::Brush(Brush::Land) && selected)
    );

    click(&mut app, Action::SetAppearance(Appearance::Paper));
    assert!(app.world().resource::<Jobs>().preferences);
    assert_eq!(
        tools(app.world_mut()),
        holo.iter()
            .map(|&(action, _, selected)| (action, true, selected))
            .collect::<Vec<_>>()
    );
    click(&mut app, Action::SetAppearance(Appearance::Holo));
    assert_eq!(tools(app.world_mut()), holo);
}

#[test]
fn map_menu_and_advanced_settings_show_biome_choices() {
    use elmapa::{biome_packs, document::Brush};

    let desert = biome_packs::find("desert").unwrap();
    let alpine = biome_packs::find("alpine").unwrap();
    let mut studio = Studio {
        home: false,
        menu: true,
        advanced: true,
        ..default()
    };
    studio.set_biome_pack(Some(desert));
    let mut app = ui_app(studio);
    app.update();

    let buttons = |world: &mut World, filter: fn(Action) -> bool| {
        let mut hits: Vec<_> = world
            .query::<(&Hit, &Children)>()
            .iter(world)
            .filter_map(|(hit, children)| {
                let action = hit.action.filter(|&action| filter(action))?;
                let label = children.iter().find_map(|child| world.get::<Text>(child))?;
                Some((hit.order, action, label.0.clone()))
            })
            .collect();
        hits.sort_by_key(|(order, _, _)| *order);
        hits.into_iter()
            .map(|(_, action, label)| (action, label))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        buttons(app.world_mut(), |action| matches!(
            action,
            Action::BiomePack(_)
        )),
        [
            (Action::BiomePack(None), "All Materials".into()),
            (Action::BiomePack(Some(desert)), "Desert".into()),
            (Action::BiomePack(Some(alpine)), "Alpine".into()),
        ]
    );
    let material_buttons = |world: &mut World, brush| {
        assert_eq!(
            buttons(world, |action| matches!(
                action,
                Action::AdvancedBrushMaterial(_)
            )),
            biome_packs::materials(Some(desert), brush)
                .map(|(index, finish)| (Action::AdvancedBrushMaterial(index), finish.name.clone()))
                .collect::<Vec<_>>()
        );
    };
    material_buttons(app.world_mut(), Brush::Land);
    app.world_mut()
        .resource_scope(|world, mut studio: Mut<Studio>| {
            studio.act(
                Action::Brush(Brush::Forest),
                &mut world.resource_mut::<Jobs>(),
            );
        });
    app.update();
    material_buttons(app.world_mut(), Brush::Forest);
}

#[test]
fn settings_and_map_menu_offer_both_appearances() {
    for (home, selected) in [(true, Appearance::Holo), (false, Appearance::Paper)] {
        let mut app = ui_app(Studio {
            home,
            settings: home,
            menu: !home,
            appearance: selected,
            ..default()
        });
        app.update();

        let world = app.world_mut();
        let choices: Vec<_> = world
            .query::<(&Hit, &Children, &BackgroundColor)>()
            .iter(world)
            .filter_map(|(hit, children, background)| {
                let Action::SetAppearance(appearance) = hit.action? else {
                    return None;
                };
                let text = children.iter().find_map(|id| world.get::<Text>(id))?;
                Some((appearance, text.0.as_str(), background.0.alpha()))
            })
            .collect();
        assert_eq!(choices.len(), 2);
        for (appearance, label) in [(Appearance::Holo, "Holo"), (Appearance::Paper, "Paper")] {
            let alpha = if appearance == selected { 1. } else { 0.1 };
            assert!(choices.contains(&(appearance, label, alpha)));
        }
    }
}

#[test]
fn the_layers_tree_discloses_countries_and_selects_their_visible_land() {
    use crate::app::{regions::RegionAction, state::LayerBranch};
    use elmapa::{
        document::{Brush, Point, RegionLayer, RegionParent, Stroke},
        scene::SceneCache,
    };

    let mut studio = Studio {
        home: false,
        ..default()
    };
    assert!(studio.editor.commit_stroke(Stroke {
        creation_id: None,
        material: None,
        brush: Brush::Land,
        points: vec![Point::new(-220., 0.), Point::new(220., 0.)].into(),
        size: 250.,
        strength: 1.,
        seed: 19,
    }));
    let mut cache = SceneCache::default();
    let shore = cache
        .render(studio.editor.document(), None)
        .shoreline()
        .clone();
    let mainland = shore.island_at(Point::new(0., 0.)).unwrap().clone();
    let mut continents = RegionLayer::default();
    let owner = continents.new_region();
    continents.claim(mainland.clone(), owner);
    studio.editor.set_regions(None, continents, &shore).unwrap();
    let continent = RegionParent {
        layer: studio.editor.active_layer(),
        owner,
    };
    let mut countries = RegionLayer {
        parent: Some(continent),
        ..default()
    };
    let owner = countries.new_region();
    countries.claim(mainland, owner);
    studio.editor.set_regions(None, countries, &shore).unwrap();
    let country = RegionParent {
        layer: studio.editor.active_layer(),
        owner,
    };
    studio
        .scene
        .publish(cache.render(studio.editor.document(), None), Vec::new());
    studio.viewport.resolve(studio.bounds);
    studio.viewport.pan.x += 9_000.;
    let zoom = studio.viewport.zoom;
    let revision = studio.editor.revision();
    let mut app = ui_app(studio);
    app.update();

    let select = Action::Regions(RegionAction::SelectRegion(country));
    let expand = Action::SetLayerBranch(LayerBranch::Region(continent), true);
    let collapse = Action::SetLayerBranch(LayerBranch::Region(continent), false);
    let has_action = |app: &mut App, action| {
        let world = app.world_mut();
        world
            .query::<&Hit>()
            .iter(world)
            .any(|hit| hit.action == Some(action))
    };
    let click = |app: &mut App, action| {
        app.world_mut()
            .resource_mut::<Studio>()
            .act(action, &mut Jobs::default());
        app.update();
    };
    assert!(has_action(&mut app, expand));
    assert!(!has_action(&mut app, select));
    click(&mut app, expand);
    assert!(has_action(&mut app, select));
    assert!(has_action(&mut app, collapse));
    click(&mut app, select);
    {
        let studio = app.world().resource::<Studio>();
        let (_, _, owner) = studio.region_style_target().unwrap();
        assert_eq!(
            (studio.editor.active_layer(), owner),
            (country.layer, country.owner)
        );
        let center = studio.map_point(studio.bounds.center());
        let anchor = studio.regions.style.anchor.unwrap();
        assert!((center.x - anchor.x).abs() < 0.01 && (center.y - anchor.y).abs() < 0.01);
        assert_eq!(studio.viewport.zoom, zoom);
    }
    click(&mut app, collapse);
    assert!(!has_action(&mut app, select));
    let studio = app.world().resource::<Studio>();
    assert_eq!(studio.editor.revision(), revision);
    assert!(studio.region_style_target().is_some());
    click(&mut app, expand);
    let layer = LayerBranch::Layer(continent.layer);
    click(&mut app, Action::SetLayerBranch(layer, false));
    assert!(!has_action(&mut app, select));
    click(&mut app, Action::SetLayerBranch(layer, true));
    assert!(has_action(&mut app, select));
}
