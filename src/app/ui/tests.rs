use super::*;
use assets::UiAssets;
use feedback::{Hover, HoverFollow, HoverFrame};
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
fn paper_preview_rebuilds_the_toolbar_in_either_appearance() {
    use elmapa::document::Brush;

    let mut app = ui_app(Studio {
        home: false,
        ..default()
    });
    let land_has_paper = |world: &mut World| {
        let (_, texture) = world
            .query::<(&Hit, Option<&ImageNode>)>()
            .iter(world)
            .find(|(hit, _)| hit.action == Some(Action::Brush(Brush::Land)))
            .expect("Land tool");
        texture.is_some()
    };
    let click = |app: &mut App, action| {
        app.world_mut()
            .resource_scope(|world, mut studio: Mut<Studio>| {
                studio.act(action, &mut world.resource_mut::<Jobs>());
            });
        app.update();
    };

    app.update();
    let initial_dark = app.world().resource::<Studio>().dark;
    assert!(!land_has_paper(app.world_mut()));
    click(&mut app, Action::PaperUi);
    assert!(app.world().resource::<Jobs>().preferences);
    assert_eq!(app.world().resource::<Studio>().dark, initial_dark);
    assert!(land_has_paper(app.world_mut()));
    click(&mut app, Action::Theme);
    assert!(land_has_paper(app.world_mut()));
    click(&mut app, Action::PaperUi);
    assert!(!land_has_paper(app.world_mut()));
    assert_ne!(app.world().resource::<Studio>().dark, initial_dark);
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
fn dark_shelf_chrome_matches_neutral_canvas_panels() {
    let mut library =
        elmapa::library::Library::load(std::env::temp_dir().join("elmapa-palette-test-no-files"))
            .unwrap();
    library.shelf.atlases.push(elmapa::library::AtlasRecord {
        id: 1,
        name: "Test atlas".into(),
        source: None,
        camera: elmapa::library::Camera::for_document(&elmapa::document::Document::blank()),
    });
    let mut app = ui_app(Studio {
        dark: true,
        settings: true,
        library: Some(library),
        ..default()
    });
    app.update();

    let neutral = |c: Color| {
        let c = c.to_linear();
        (c.red - c.green).abs() < 0.001 && (c.green - c.blue).abs() < 0.001
    };
    let world = app.world_mut();
    for (entity, background, text, image, hover, frame, follow) in world
        .query::<(
            Entity,
            Option<&BackgroundColor>,
            Option<&TextColor>,
            Option<&ImageNode>,
            Option<&Hover>,
            Option<&HoverFrame>,
            Option<&HoverFollow>,
        )>()
        .iter(world)
    {
        // Atlas artwork keeps its parchment colors in either appearance.
        if world.get::<PreviewSlot>(entity).is_some() {
            continue;
        }
        for c in background
            .map(|c| c.0)
            .into_iter()
            .chain(text.map(|c| c.0))
            .chain(image.map(|c| c.color))
            .chain(hover.into_iter().flat_map(|h| [h.base, h.target]))
            .chain(frame.into_iter().flat_map(|h| [h.from, h.to]))
            .chain(follow.into_iter().flat_map(|h| [h.from, h.to]))
        {
            assert!(neutral(c), "Dark shelf chrome has a color cast: {c:?}");
        }
    }
    for gradient in world.query::<&BackgroundGradient>().iter(world) {
        for gradient in &gradient.0 {
            if let Gradient::Linear(gradient) = gradient {
                for stop in &gradient.stops {
                    assert!(
                        neutral(stop.color),
                        "Dark shelf background has a color cast"
                    );
                    assert!(
                        stop.color.to_linear().red <= 0.22,
                        "Dark shelf background must stay as dark as the canvas chrome"
                    );
                }
            }
        }
    }
    for border in world.query::<&BorderColor>().iter(world) {
        assert!(
            [border.top, border.right, border.bottom, border.left]
                .into_iter()
                .all(neutral)
        );
    }
    for shadow in world.query::<&BoxShadow>().iter(world) {
        assert!(shadow.0.iter().all(|s| neutral(s.color)));
    }
    assert!(
        world
            .query::<&BackgroundColor>()
            .iter(world)
            .any(|c| { c.0 == color(Palette::new(true).surface, 0.98) }),
        "Settings should use the canvas panel surface"
    );
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
