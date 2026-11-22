//! Embedded icons, brush textures and system fonts for the native UI.
use crate::app::{branding, presentation::DisplayCamera, render};
use bevy::{prelude::*, text::TextLayoutInfo};
use elmapa::{
    brush_materials::{self, ForestStyle},
    document::{AtlasStyle, Brush, LabelFont},
    scene::{forest_specimen_svg, forest_swatch},
};
use resvg::tiny_skia::{Pixmap, Transform};
use std::collections::HashMap;

include!(concat!(env!("OUT_DIR"), "/icons.rs"));
pub fn snap_text_glyphs(
    mut labels: Query<&mut TextLayoutInfo, (With<Text>, Changed<TextLayoutInfo>)>,
) {
    for mut layout in &mut labels {
        for glyph in &mut layout.glyphs {
            // Align the bitmap's edges to physical pixels so linear sampling
            // preserves the original antialiased coverage in the glyph atlas.
            let half_size = glyph.atlas_info.rect.size() * 0.5;
            glyph.position = (glyph.position - half_size).round() + half_size;
        }
    }
}

#[derive(Resource)]
pub struct UiAssets {
    pub(super) camera: Entity,
    pub(super) display_camera: Entity,
    pub(super) icons: HashMap<String, Handle<Image>>,
    pub(super) tool_symbols: Handle<Image>,
    pub(super) tool_paper: Handle<Image>,
    pub(super) landmark_ghosts: Vec<(Handle<Image>, Vec2)>,
    pub(super) brush_swatches: HashMap<usize, Handle<Image>>,
    pub(super) avenir: Handle<Font>,
    pub(super) baskerville: Handle<Font>,
    pub(super) didot: Handle<Font>,
}

pub fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut fonts: ResMut<Assets<Font>>,
    camera: Single<Entity, With<render::WorldCamera>>,
    display_camera: Single<Entity, With<DisplayCamera>>,
    canvas: Res<render::Canvas>,
) {
    let mut icons = HashMap::new();
    icons.insert(
        branding::SYMBOL.into(),
        images.add(render::rgba_image(
            128,
            128,
            branding::icon_pixels(branding::MARK, 128),
        )),
    );
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../assets/icons/manifest.json")).expect("icons");
    for icon in manifest["icons"].as_array().expect("icon manifest") {
        let file = icon["file"].as_str().expect("icon filename");
        let bytes = embedded_icon(file).expect("embedded icon");
        let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).expect("icon SVG");
        let mut pixmap = resvg::tiny_skia::Pixmap::new(68, 68).expect("icon");
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(68. / 24., 68. / 24.),
            &mut pixmap.as_mut(),
        );
        // GPUI treats these monochrome SVGs as masks. Store white, straight-alpha pixels.
        let mut pixels = pixmap.take();
        for p in pixels.chunks_exact_mut(4) {
            p[0] = 255;
            p[1] = 255;
            p[2] = 255;
        }
        icons.insert(
            file.trim_end_matches(".svg").to_string(),
            images.add(render::rgba_image(68, 68, pixels)),
        );
    }
    let tool = usvg::Tree::from_str(
        include_str!("../../../assets/landmarks/tool.svg"),
        &usvg::Options::default(),
    )
    .expect("landmark tool");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(96, 96).expect("tool");
    resvg::render(
        &tool,
        resvg::tiny_skia::Transform::from_scale(4., 4.),
        &mut pixmap.as_mut(),
    );
    let mut pixels = pixmap.take();
    for p in pixels.chunks_exact_mut(4) {
        p[0] = 255;
        p[1] = 255;
        p[2] = 255;
    }
    icons.insert(
        "landmark".into(),
        images.add(render::rgba_image(96, 96, pixels)),
    );
    let tool_symbols = usvg::Tree::from_str(
        include_str!("../../../assets/tool-icons/symbols.svg"),
        &usvg::Options::default(),
    )
    .expect("toolbar symbols");
    let size = tool_symbols.size().to_int_size().scale_by(4.).unwrap();
    let mut pixmap = Pixmap::new(size.width(), size.height()).expect("toolbar symbols");
    resvg::render(
        &tool_symbols,
        Transform::from_scale(4., 4.),
        &mut pixmap.as_mut(),
    );
    let tool_symbols = images.add(render::rgba_image(
        size.width(),
        size.height(),
        unpremultiply(pixmap.take()),
    ));
    let paper = image::load_from_memory_with_format(
        include_bytes!("../../../assets/tool-icons/paper.png"),
        image::ImageFormat::Png,
    )
    .expect("toolbar paper")
    .into_rgba8();
    let tool_paper = images.add(render::rgba_image(
        paper.width(),
        paper.height(),
        paper.into_raw(),
    ));
    let catalog = elmapa::scene::landmark_catalog();
    let previews = catalog
        .items
        .iter()
        .map(|d| (d.kind.key(), d.icon.as_str()));
    let pack_icons = catalog
        .packs
        .iter()
        .filter_map(|p| p.icon.as_deref().map(|svg| (p.icon_key.as_str(), svg)));
    for (key, svg) in previews.chain(pack_icons) {
        let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).expect("landmark preview");
        let mut pixmap = resvg::tiny_skia::Pixmap::new(160, 128).expect("preview");
        resvg::render(
            &tree,
            {
                let bounds = tree.root().abs_bounding_box();
                let scale = (148. / bounds.width()).min(116. / bounds.height());
                resvg::tiny_skia::Transform::from_row(
                    scale,
                    0.,
                    0.,
                    scale,
                    80. - (bounds.left() + bounds.width() / 2.) * scale,
                    64. - (bounds.top() + bounds.height() / 2.) * scale,
                )
            },
            &mut pixmap.as_mut(),
        );
        let mut pixels = unpremultiply(pixmap.take());
        // Pack icons are theme-tinted masks; item previews keep their authored pigments.
        if key.starts_with("landmark-pack/") {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel[..3].fill(255);
            }
        }
        icons.insert(key.into(), images.add(render::rgba_image(160, 128, pixels)));
    }
    let landmark_ghosts = catalog
        .items
        .iter()
        .map(|definition| {
            let tree = usvg::Tree::from_str(&definition.svg, &usvg::Options::default())
                .expect("landmark artwork");
            let size = Vec2::new(tree.size().width(), tree.size().height());
            let scale = 256. / size.max_element();
            let pixels = (size * scale).ceil();
            let mut pixmap = Pixmap::new((pixels.x as u32).max(1), (pixels.y as u32).max(1))
                .expect("landmark ghost");
            resvg::render(
                &tree,
                Transform::from_scale(scale, scale),
                &mut pixmap.as_mut(),
            );
            let image = render::rgba_image(
                pixmap.width(),
                pixmap.height(),
                unpremultiply(pixmap.take()),
            );
            (images.add(image), size)
        })
        .collect();
    let mut load = |face| {
        elmapa::scene::label_font_bytes(face)
            .map(|bytes| fonts.add(Font::from_bytes(bytes)))
            .unwrap_or_default()
    };
    let avenir = load(LabelFont::Avenir);
    let baskerville = load(LabelFont::Baskerville);
    let mut brush_swatches = canvas.brush_textures.clone();
    for style in [AtlasStyle::Classic, AtlasStyle::Woodland] {
        for (index, material) in brush_materials::for_brush(Brush::Forest) {
            brush_swatches.insert(
                brush_materials::artwork_index(index, style),
                images.add(forest_preview(material.forest, style)),
            );
        }
    }
    commands.insert_resource(UiAssets {
        camera: *camera,
        display_camera: *display_camera,
        icons,
        tool_symbols,
        tool_paper,
        landmark_ghosts,
        brush_swatches,
        avenir,
        baskerville,
        didot: load(LabelFont::Didot),
    });
}

// resvg returns premultiplied pixels, but Bevy samples straight alpha.
fn unpremultiply(mut pixels: Vec<u8>) -> Vec<u8> {
    for pixel in pixels.chunks_exact_mut(4) {
        if pixel[3] > 0 {
            for channel in 0..3 {
                pixel[channel] =
                    ((u32::from(pixel[channel]) * 255) / u32::from(pixel[3])).min(255) as u8;
            }
        }
    }
    pixels
}

fn forest_preview(family: Option<ForestStyle>, style: AtlasStyle) -> Image {
    const SIZE: u32 = 96;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{SIZE}\" height=\"{SIZE}\" viewBox=\"-45 -45 90 90\">"
    );
    use std::fmt::Write;
    let variants = forest_swatch(family);
    for (index, &variant) in variants.iter().enumerate() {
        let transform = match (variants.len(), index) {
            (1, _) => "translate(-2 22) scale(1.12)",
            (_, 0) => "translate(-12 10) scale(.73)",
            _ => "translate(10 23) scale(.82)",
        };
        write!(
            svg,
            "<g transform=\"{transform}\">{}</g>",
            forest_specimen_svg(variant, style)
        )
        .unwrap();
    }
    svg.push_str("</svg>");
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("forest swatch");
    let mut pixmap = Pixmap::new(SIZE, SIZE).expect("forest swatch");
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
    let mut pixels = pixmap.take();
    render::straight_alpha(&mut pixels);
    render::rgba_image(SIZE, SIZE, pixels)
}
