use super::*;
use assets::UiAssets;
use feedback::{Hover, HoverFollow, HoverFrame};
use std::collections::HashMap;

#[test]
fn dark_shelf_chrome_matches_neutral_canvas_panels() {
    let mut app = App::new();
    let camera = app.world_mut().spawn_empty().id();
    let mut library =
        elmapa::library::Library::load(std::env::temp_dir().join("elmapa-palette-test-no-files"))
            .unwrap();
    library.shelf.atlases.push(elmapa::library::AtlasRecord {
        id: 1,
        name: "Test atlas".into(),
        source: None,
        camera: elmapa::library::Camera::for_document(&elmapa::document::Document::blank()),
    });
    app.insert_resource(Studio {
        dark: true,
        settings: true,
        library: Some(library),
        ..default()
    })
    .insert_resource(UiAssets {
        camera,
        display_camera: camera,
        icons: HashMap::new(),
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
