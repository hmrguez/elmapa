//! Tessellate shared scene batches into world-space Bevy meshes.
use crate::app::{river_material::spray_envelope, ui::color};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use elmapa::{
    brush_materials,
    document::Point,
    scene::{PathPart, RiverReach, Shape},
};
use lyon::{
    math::point,
    path::Path,
    tessellation::{
        BuffersBuilder, FillOptions, FillTessellator, FillVertex, LineCap, LineJoin, StrokeOptions,
        StrokeTessellator, StrokeVertex, VertexBuffers,
    },
};
use std::sync::Arc;

pub struct CpuMesh {
    pub source: Arc<Vec<Shape>>,
    pub mesh: Mesh,
}

pub fn tessellate(source: Arc<Vec<Shape>>) -> Result<CpuMesh, String> {
    tessellate_river(source, None)
}
fn tessellate_river(
    source: Arc<Vec<Shape>>,
    river: Option<&RiverReach>,
) -> Result<CpuMesh, String> {
    let _span = elmapa::performance::Span::new("bevy_tessellation");
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    let mut fill = FillTessellator::new();
    let mut stroke = StrokeTessellator::new();
    let spray = river
        .filter(|reach| reach.spray && !source.is_empty())
        .map(spray_envelope);
    for (shape_index, shape) in source.iter().chain(spray.iter()).enumerate() {
        if let Shape::Sprite {
            points,
            material,
            frame,
            opacity,
        } = shape
        {
            let frames = brush_materials::artwork_catalog()[*material]
                .sprites
                .expect("sprite sheet material")
                .frames as f32;
            let left = *frame as f32 / frames;
            let right = (*frame + 1) as f32 / frames;
            let offset = positions.len() as u32;
            positions.extend(points.map(|p| [p.x, -p.y, 0.]));
            uvs.extend([[left, 0.], [right, 0.], [right, 1.], [left, 1.]]);
            colors.extend([[1., 1., 1., *opacity]; 4]);
            indices.extend([0, 2, 1, 0, 3, 2].map(|i| i + offset));
            continue;
        }
        let (contours, hex, alpha, tile) = match shape {
            Shape::Sprite { .. } => unreachable!("sprite quads need no tessellation"),
            Shape::ShoreWash(wash) => (wash.contours.as_slice(), 0, 0., None),
            Shape::Polygon {
                points,
                fill,
                opacity,
            } => (std::slice::from_ref(points), *fill, *opacity, None),
            Shape::Line {
                points,
                color,
                opacity,
                ..
            } => (std::slice::from_ref(points), *color, *opacity, None),
            Shape::RegionSurface {
                contours,
                material,
                fill,
                opacity,
            } => {
                let tile = material.map(|id| brush_materials::artwork_catalog()[id].tile_size);
                let color = if tile.is_some() { 0xffffff } else { *fill };
                (contours.as_slice(), color, *opacity, tile)
            }
        };
        let mut builder = Path::builder();
        for points in contours.iter().filter(|points| points.len() >= 2) {
            builder.begin(point(points[0].x, -points[0].y));
            for p in &points[1..] {
                builder.line_to(point(p.x, -p.y));
            }
            builder.end(!matches!(shape, Shape::Line { .. }));
        }
        let path = builder.build();
        let mut buffers: VertexBuffers<[f32; 3], u32> = VertexBuffers::new();
        match shape {
            Shape::Sprite { .. } => unreachable!("sprite quads need no tessellation"),
            Shape::Polygon { .. } | Shape::RegionSurface { .. } | Shape::ShoreWash(_) => fill
                .tessellate_path(
                    &path,
                    &FillOptions::default(),
                    &mut BuffersBuilder::new(&mut buffers, |v: FillVertex<'_>| {
                        [v.position().x, v.position().y, 0.]
                    }),
                ),
            Shape::Line { width, .. } => stroke.tessellate_path(
                &path,
                &StrokeOptions::default()
                    .with_line_width(*width)
                    .with_line_cap(LineCap::Butt)
                    .with_line_join(LineJoin::Miter),
                &mut BuffersBuilder::new(&mut buffers, |v: StrokeVertex<'_, '_>| {
                    [v.position().x, v.position().y, 0.]
                }),
            ),
        }
        .map_err(|e| e.to_string())?;
        let offset = positions.len() as u32;
        let rgba = color(hex, alpha).to_linear().to_f32_array();
        colors.extend(buffers.vertices.iter().map(|v| match shape {
            Shape::ShoreWash(wash) => wash.vertex_color(Point::new(v[0], -v[1])),
            _ => rgba,
        }));
        uvs.extend(buffers.vertices.iter().map(|v| {
            river.map_or_else(
                || tile.map_or([0., 0.], |size| [v[0] / size, -v[1] / size]),
                |reach| {
                    let (along, across) = reach.coordinates(Point::new(v[0], -v[1]));
                    let surface = if shape_index >= source.len() {
                        3.
                    } else if reach.is_current_ink(hex) {
                        1.
                    } else if reach.is_water_fill(hex) {
                        0.
                    } else {
                        2.
                    };
                    [along, surface + across * 0.125]
                },
            )
        }));
        positions.extend(buffers.vertices);
        indices.extend(buffers.indices.into_iter().map(|i| i + offset));
    }
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices));
    Ok(CpuMesh { source, mesh })
}

// Road sections are tessellated once in their own walking frame. Animation can
// reveal them with a retained transform, without per-frame mesh regeneration.
fn tessellate_batch(source: Arc<Vec<Shape>>, road: Option<&PathPart>) -> Result<CpuMesh, String> {
    let mut cpu = tessellate(source)?;
    if let Some(part) = road {
        let angle = -(part.end.y - part.start.y).atan2(part.end.x - part.start.x);
        let inverse = Quat::from_rotation_z(-angle);
        if let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            cpu.mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            for position in positions {
                let local = inverse
                    * (Vec3::from_array(*position) - Vec3::new(part.start.x, -part.start.y, 0.));
                *position = local.to_array();
            }
        }
    }
    Ok(cpu)
}

pub fn tessellate_material(
    source: Arc<Vec<Shape>>,
    road: Option<&PathPart>,
    river: Option<&RiverReach>,
) -> Result<CpuMesh, String> {
    if river.is_some() {
        tessellate_river(source, river)
    } else {
        tessellate_batch(source, road)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::render::motion::{mountain_transform, path_transform};
    use elmapa::scene::MountainPart;
    #[test]
    fn tessellation_keeps_world_coordinates_alpha_and_shape_order() {
        let source = Arc::new(vec![
            Shape::Polygon {
                points: vec![
                    Point::new(-20., 10.),
                    Point::new(30., 10.),
                    Point::new(30., 40.),
                    Point::new(-20., 40.),
                ],
                fill: 0x4f6b43,
                opacity: 0.5,
            },
            Shape::Line {
                points: vec![Point::new(-10., -10.), Point::new(10., -10.)],
                color: 0xffffff,
                width: 2.,
                opacity: 0.2,
            },
        ]);
        let cpu = tessellate(source.clone()).unwrap();
        assert!(Arc::ptr_eq(&cpu.source, &source));
        let positions = cpu
            .mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        assert!(positions.iter().any(|v| v[0] == -20. && v[1] == -10.));
        assert!(positions.iter().any(|v| v[1] > 0.));
        let bevy::mesh::VertexAttributeValues::Float32x4(colors) =
            cpu.mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap()
        else {
            panic!("RGBA colors")
        };
        assert_eq!(colors[0][3], 0.5);
        assert_eq!(colors.last().unwrap()[3], 0.2);
    }
    #[test]
    fn river_mesh_encodes_banks_currents_and_native_spray_in_channel_coordinates() {
        let reach = RiverReach {
            id: (1, 0, 7, 0),
            course: vec![
                Point::new(100., 100.),
                Point::new(100., 0.),
                Point::new(0., 0.),
            ],
            distance: 32.,
            length: 200.,
            half_width: 10.,
            channel_fill: 0xccc9ba,
            spray: true,
        };
        let source = Arc::new(vec![
            Shape::Polygon {
                points: vec![
                    Point::new(95., 90.),
                    Point::new(105., 90.),
                    Point::new(105., 60.),
                    Point::new(95., 60.),
                ],
                fill: reach.channel_fill,
                opacity: 1.,
            },
            Shape::Line {
                points: reach.course.clone(),
                color: elmapa::scene::CURRENT_INK,
                width: 0.6,
                opacity: 0.5,
            },
            Shape::Line {
                points: reach.course.clone(),
                color: 0x75654f,
                width: 0.72,
                opacity: 0.8,
            },
        ]);
        let mesh = tessellate_river(source, Some(&reach)).unwrap().mesh;
        let bevy::mesh::VertexAttributeValues::Float32x2(uvs) =
            mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap()
        else {
            panic!("river coordinates");
        };
        assert!(uvs.iter().any(|uv| uv[0] <= 32.001 && uv[1].round() == 1.));
        assert!(uvs.iter().any(|uv| uv[0] >= 231.99 && uv[1].round() == 1.));
        assert!(uvs.iter().any(|uv| uv[1].round() == 0.));
        assert!(uvs.iter().any(|uv| uv[1].round() == 2.));
        assert!(uvs.iter().any(|uv| uv[1].round() == 3. && uv[0] > 232.));
        assert!(
            uvs.iter()
                .any(|uv| uv[1].round() == 3. && (uv[1] - 3.).abs() > 0.125)
        );
        assert!(
            uvs.iter()
                .filter(|uv| uv[1].round() == 1.)
                .all(|uv| (32. - 0.001..=232. + 0.001).contains(&uv[0]))
        );
    }
    #[test]
    fn mountain_uplift_keeps_the_earth_fixed_and_finishes_at_identity() {
        let part = MountainPart {
            id: (1, 0, 7, 0),
            base: Point::new(100., 200.),
            side: -1.,
            width: 60.,
            bounds: elmapa::document::ContentBounds {
                min: Point::new(70., 140.),
                max: Point::new(130., 200.),
            },
        };
        let transform = mountain_transform(&part, 0.4, (0.2, -2.));
        let ground = transform.transform_point(Vec3::new(100., -200., 0.));
        assert!((ground.y + 200.).abs() < 0.00001);
        assert_eq!(ground.x, 98.);
        assert_eq!(
            mountain_transform(&part, 0.4, (1., 0.)),
            Transform::from_xyz(0., 0., 0.4)
        );
    }
    #[test]
    fn road_reveal_uses_the_walking_frame_and_finishes_at_export_coordinates() {
        for end in [
            Point::new(150., 75.),
            Point::new(55., 160.),
            Point::new(0., 20.),
        ] {
            let part = PathPart {
                id: (1, 0, 7, 0),
                start: Point::new(75., 80.),
                end,
            };
            let source = Arc::new(vec![Shape::Line {
                points: vec![part.start, part.end],
                color: 0x796044,
                width: 1.,
                opacity: 1.,
            }]);
            let world = tessellate(source.clone()).unwrap();
            let local = tessellate_batch(source, Some(&part)).unwrap();
            let positions = |mesh: &Mesh| match mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() {
                bevy::mesh::VertexAttributeValues::Float32x3(p) => p.clone(),
                _ => panic!("position format"),
            };
            for (expected, local) in positions(&world.mesh).iter().zip(positions(&local.mesh)) {
                let displayed =
                    path_transform(&part, 0., (1., 1.)).transform_point(Vec3::from_array(local));
                assert!((displayed - Vec3::from_array(*expected)).length() < 0.0001);
            }
            for pose in [(0., 0.), (0.6, 0.4), (1., 1.)] {
                assert_eq!(
                    path_transform(&part, 0., pose).transform_point(Vec3::ZERO),
                    Vec3::new(75., -80., 0.)
                );
            }
        }
    }
}
