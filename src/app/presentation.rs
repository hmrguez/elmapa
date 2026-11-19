//! Preserve the original gamma-space compositing while presenting through Bevy.
//! Cartographic inks and UI masks blend in their authored sRGB coordinates. A
//! single final pass converts that result to the window's linear/sRGB pipeline.
use super::render::WorldCamera;
use bevy::{
    asset::embedded_asset,
    camera::{RenderTarget, visibility::RenderLayers},
    image::ImageSampler,
    prelude::*,
    reflect::TypePath,
    render::render_resource::{AsBindGroup, TextureFormat, TextureUsages},
    shader::ShaderRef,
    sprite_render::{Material2d, Material2dPlugin},
};

pub struct PresentationPlugin;
impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "presentation.wgsl");
        app.add_plugins(Material2dPlugin::<DisplayMaterial>::default());
    }
}
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct DisplayMaterial {
    #[texture(0)]
    #[sampler(1)]
    image: Handle<Image>,
}
impl Material2d for DisplayMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://elmapa/app/presentation.wgsl".into()
    }
}
#[derive(Component)]
pub struct DisplayQuad;
#[derive(Component)]
pub struct DisplayCamera;
#[derive(Resource)]
pub struct Presentation {
    physical: UVec2,
}
fn target(size: UVec2) -> Image {
    let mut image = Image::new_target_texture(
        size.x.max(1),
        size.y.max(1),
        TextureFormat::Rgba8Unorm,
        None,
    );
    image.sampler = ImageSampler::nearest();
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    image
}
pub fn setup(
    mut commands: Commands,
    window: Single<&Window>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DisplayMaterial>>,
) {
    let physical = UVec2::new(window.physical_width(), window.physical_height());
    elmapa::performance::counter("window_logical_width", window.width() as u64);
    elmapa::performance::counter("window_logical_height", window.height() as u64);
    elmapa::performance::counter("window_physical_width", u64::from(physical.x));
    elmapa::performance::counter("window_physical_height", u64::from(physical.y));
    elmapa::performance::counter("window_scale_milli", (window.scale_factor() * 1000.) as u64);
    let image = images.add(target(physical));
    commands.spawn((
        Camera2d,
        WorldCamera,
        Camera {
            order: -1,
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        Msaa::Sample4,
        IsDefaultUiCamera,
    ));
    commands.spawn((
        Camera2d,
        DisplayCamera,
        Camera {
            order: 0,
            ..default()
        },
        RenderLayers::layer(1),
        Msaa::Off,
    ));
    commands.spawn((
        DisplayQuad,
        Mesh2d(meshes.add(Rectangle::default())),
        MeshMaterial2d(materials.add(DisplayMaterial { image })),
        Transform::from_scale(Vec3::new(window.width(), window.height(), 1.)),
        RenderLayers::layer(1),
    ));
    commands.insert_resource(Presentation { physical });
}
pub fn resize(
    mut presentation: ResMut<Presentation>,
    window: Single<&Window>,
    mut images: ResMut<Assets<Image>>,
    mut camera_target: Single<&mut RenderTarget, With<WorldCamera>>,
    mut quad: Single<(&mut Transform, &MeshMaterial2d<DisplayMaterial>), With<DisplayQuad>>,
    mut materials: ResMut<Assets<DisplayMaterial>>,
) {
    let physical = UVec2::new(window.physical_width(), window.physical_height());
    if presentation.physical != physical {
        // Rebind both passes together. An existing material bind group can
        // still sample the previous GPU view when an Image changes in place.
        let image = images.add(target(physical));
        **camera_target = RenderTarget::Image(image.clone().into());
        materials
            .get_mut(&quad.1.0)
            .expect("display material")
            .image = image;
        presentation.physical = physical;
    }
    let desired = Vec3::new(window.width(), window.height(), 1.);
    if quad.0.scale != desired {
        quad.0.scale = desired;
    }
}
