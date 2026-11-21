//! Bevy application: retained GPU rendering, transactional editor and native UI.
mod benchmark;
mod branding;
mod brush_motion;
mod brush_orbit;
#[cfg(not(target_os = "macos"))]
mod clipboard;
mod files;
mod input;
mod landmark_render;
mod landmarks;
mod lettering;
#[cfg(target_os = "macos")]
mod native;
mod presentation;
mod radial;
mod regions;
mod render;
mod replay;
mod river_material;
mod scene_work;
mod selection;
mod state;
mod ui;
mod viewport;

use bevy::{
    prelude::*,
    window::{ExitCondition, PresentMode},
    winit::WinitSettings,
};

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--export-demo") {
        let path = args.get(2).map(String::as_str).unwrap_or("elmapa-demo.svg");
        let file = std::fs::File::create(path).expect("create SVG");
        elmapa::scene::write_svg(
            &elmapa::document::Document::demo(),
            std::io::BufWriter::new(file),
        )
        .expect("write SVG");
        println!("Exported {path}");
        return;
    }
    if elmapa::performance::enabled() {
        elmapa::allocation::enable();
        elmapa::performance::mark("launch");
        assert!(
            std::env::var_os("ELMAPA_REQUIRE_PRODUCTION").is_none() || !cfg!(debug_assertions),
            "Native acceptance requires a release or profiling build"
        );
    }
    #[cfg(target_os = "macos")]
    native::observe_presentations();
    let mut app = App::new();
    #[cfg(target_os = "macos")]
    app.add_systems(Startup, (native::setup_menu, branding::set_app_icon))
        .add_systems(
            Update,
            native::menu_actions
                .after(input::dispatch)
                .before(files::poll),
        );
    #[cfg(not(target_os = "macos"))]
    app.add_systems(Update, branding::set_window_icon);
    let app = app
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Elmapa".into(),
                        resolution: (1440, 940).into(),
                        resize_constraints: bevy::window::WindowResizeConstraints {
                            min_width: 780.,
                            min_height: 620.,
                            ..default()
                        },
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    close_when_requested: false,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    custom_layer: benchmark::log_layer,
                    ..default()
                }),
        )
        .add_plugins(presentation::PresentationPlugin)
        .add_plugins(river_material::RiverPlugin)
        .add_plugins(landmark_render::LandmarkPlugin)
        .add_plugins(render::ForestWindPlugin)
        .add_plugins(render::view_replay::InkRevealPlugin)
        .add_plugins(render::wet_edge::WetEdgePlugin)
        .add_plugins(render::tree_leaves::TreeLeafPlugin)
        .insert_resource(WinitSettings::game())
        .insert_resource(ClearColor(ui::color(0xe6e5d3, 1.)))
        .init_resource::<state::Studio>()
        .init_resource::<render::Canvas>()
        .init_resource::<files::Jobs>()
        .init_resource::<replay::Replay>()
        .init_resource::<render::view_replay::video::VideoExport>()
        .init_resource::<benchmark::Benchmark>()
        .init_resource::<ui::ChromeMotion>()
        .init_resource::<brush_motion::BrushMotion>()
        .init_resource::<ui::ScrollOffsets>()
        .add_systems(
            Startup,
            (presentation::setup, render::setup, ui::setup).chain(),
        )
        .add_systems(First, benchmark::frame_start)
        .add_systems(
            Update,
            (
                replay::advance,
                input::dispatch,
                files::poll,
                (
                    scene_work::resolve_scene,
                    render::sync,
                    render::wet_edge::sync,
                    scene_work::schedule_scene,
                )
                    .chain(),
                (
                    render::grow,
                    render::rise,
                    render::flow,
                    render::open_path,
                    render::drop_text,
                    landmark_render::assemble,
                    landmark_render::follow_drag,
                )
                    .chain(),
                presentation::resize,
                render::camera,
                (render::region_overlay, render::path_guide).chain(),
                (
                    render::view_replay::video::prepare,
                    render::view_replay::play,
                    landmark_render::accents,
                    render::tree_leaves::fall,
                )
                    .chain(),
                render::opening,
                ui::advance_motion,
                ui::sync_scale,
                ui::rebuild,
                ui::alpha,
                ui::hover,
                ui::hover_follow,
                ui::menu_animation,
                (
                    ui::dynamic_text,
                    ui::landmark_cursor,
                    ui::landmark_selection_follow,
                    ui::text_selection_follow,
                    ui::region_selection_follow,
                    ui::path_selection_follow,
                    ui::selection_follow,
                    ui::radial_follow,
                    ui::text_ime,
                )
                    .chain(),
                ui::tooltips,
                input::close,
            )
                .chain(),
        )
        .add_systems(Last, benchmark::frame_end)
        .add_systems(
            PostUpdate,
            ui::snap_text_glyphs.after(bevy::ui::UiSystems::PostLayout),
        )
        .add_systems(
            PostUpdate,
            brush_motion::animate
                .after(bevy::ui::UiSystems::Layout)
                .before(bevy::ui::UiSystems::PostLayout),
        );
    render::tile_upload::install(app);
    let result = app.run();
    if let Err(error) = elmapa::performance::flush() {
        eprintln!("Could not write trace: {error}");
    }
    if !result.is_success() {
        std::process::exit(1);
    }
}
