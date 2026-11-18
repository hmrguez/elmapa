//! Retained river meshes: a shared stream front only updates small uniforms.
use bevy::{
    asset::embedded_asset,
    prelude::*,
    reflect::TypePath,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
};
use elmapa::{
    document::Point,
    scene::{RiverFlow, RiverReach, Shape},
};

pub struct RiverPlugin;
impl Plugin for RiverPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "river.wgsl");
        app.add_plugins(Material2dPlugin::<RiverMaterial>::default());
    }
}
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct RiverMaterial {
    /// Advancing front (-1000 for settled), energy, layer opacity, clock.
    #[uniform(0)]
    pub flow: Vec4,
    /// Reach start/end and current stroke tip; gives spray one owner at seams.
    #[uniform(1)]
    pub span: Vec4,
}
impl RiverMaterial {
    pub fn new(reach: &RiverReach, pose: Option<RiverFlow>, now: f32, opacity: f32) -> Self {
        let mut material = Self {
            flow: Vec4::new(-1000., 0., opacity, now),
            span: Vec4::new(reach.distance, reach.distance + reach.length, 0., 0.),
        };
        material.set_flow(pose, now);
        material
    }
    pub fn set_flow(&mut self, pose: Option<RiverFlow>, now: f32) {
        self.flow.x = pose.map_or(-1000., |p| p.front);
        self.flow.y = pose.map_or(0., |p| p.energy);
        self.flow.w = now;
        self.span.z = pose.map_or(self.span.y, |p| p.tip);
    }
}
impl Material2d for RiverMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://elmapa/app/river.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Native-only room for the leading crest and a few tiny droplets. This never
/// enters Scene geometry, saved paths, thumbnails or SVG.
pub fn spray_envelope(reach: &RiverReach) -> Shape {
    let side = |sign: f32| {
        reach
            .course
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let a = reach.course[i.saturating_sub(1)];
                let b = reach.course[(i + 1).min(reach.course.len() - 1)];
                let len = (b.x - a.x).hypot(b.y - a.y).max(0.001);
                let tangent = Point::new((b.x - a.x) / len, (b.y - a.y) / len);
                let cap = if i == 0 {
                    -8.
                } else if i + 1 == reach.course.len() {
                    8.
                } else {
                    0.
                };
                let across = (reach.half_width * 1.5 + 1.) * sign;
                Point::new(
                    q.x - tangent.y * across + tangent.x * cap,
                    q.y + tangent.x * across + tangent.y * cap,
                )
            })
            .collect::<Vec<_>>()
    };
    let mut points = side(-1.);
    points.extend(side(1.).into_iter().rev());
    Shape::Polygon {
        points,
        fill: 0xf3eddb,
        opacity: 1.,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clearing_a_stream_preserves_layer_opacity_and_hides_native_spray() {
        let reach = RiverReach {
            id: (1, 0, 7, 0),
            course: vec![Point::new(10., 20.), Point::new(40., 20.)],
            distance: 32.,
            length: 30.,
            half_width: 10.,
            channel_fill: 0xd2d4bf,
            spray: true,
        };
        let mut material = RiverMaterial::new(
            &reach,
            Some(RiverFlow {
                front: 45.,
                energy: 0.8,
                tip: 62.,
            }),
            1.,
            0.4,
        );
        assert_eq!(material.flow, Vec4::new(45., 0.8, 0.4, 1.));
        assert_eq!(material.span, Vec4::new(32., 62., 62., 0.));
        material.set_flow(None, 2.);
        assert_eq!(material.flow, Vec4::new(-1000., 0., 0.4, 2.));
    }
}
