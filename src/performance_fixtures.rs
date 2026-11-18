//! Size-sweep fixtures shared by CPU invariants and displayed native replays.
use crate::document::{Brush, Document, Layer, Point, Stroke};
pub const WORKLOADS: [&str; 8] = [
    "blank",
    "demo",
    "dense-forest",
    "many-strokes",
    "many-islands",
    "many-layers",
    "paint-erase",
    "long-gesture",
];
pub fn fixture(name: &str, size: usize) -> Result<Document, String> {
    if !(1..=16).contains(&size) {
        return Err("Fixture size must be 1–16.".into());
    }
    let mut document = Document::blank();
    document.name = format!("{name} ×{size}");
    let stroke = |brush, i: usize, points: Vec<Point>, size| Stroke {
        creation_id: None,
        material: None,
        brush,
        points: points.into(),
        size,
        strength: 0.85,
        seed: 42_000 + i as u64,
    };
    match name {
        "blank" => {}
        "demo" => return Ok(Document::demo()),
        "dense-forest" => {
            for row in 0..size * 4 {
                document.layers[0].strokes.push(stroke(
                    Brush::Forest,
                    row,
                    vec![
                        Point::new(-600., row as f32 * 65. - 300.),
                        Point::new(600., row as f32 * 65. - 300.),
                    ],
                    180.,
                ));
            }
        }
        "many-strokes" => {
            for i in 0..size * 100 {
                document.layers[0].strokes.push(stroke(
                    Brush::Path,
                    i,
                    vec![
                        Point::new(i as f32 % 30. * 25., (i / 30) as f32 * 25.),
                        Point::new(i as f32 % 30. * 25. + 20., (i / 30) as f32 * 25. + 8.),
                    ],
                    24.,
                ));
            }
        }
        "many-islands" => {
            for i in 0..size * 16 {
                document.layers[0].strokes.push(stroke(
                    Brush::Land,
                    i,
                    vec![Point::new(
                        (i % 16) as f32 * 4000. - 32000.,
                        (i / 16) as f32 * 4000. - 32000.,
                    )],
                    220.,
                ));
            }
        }
        "many-layers" => {
            document.layers.clear();
            for i in 0..size * 8 {
                document.layers.push(Layer {
                    id: i as u64 + 1,
                    name: format!("Layer {i}"),
                    visible: true,
                    opacity: 0.6,
                    labels: Vec::new(),
                    landmarks: Vec::new(),
                    terrain_replay: None,
                    territories: None,
                    strokes: vec![stroke(
                        Brush::Land,
                        i,
                        vec![Point::new((i % 16) as f32 * 100., (i / 16) as f32 * 100.)],
                        70.,
                    )]
                    .into(),
                });
            }
            document.next_id = document.layers.len() as u64 + 1;
        }
        "paint-erase" => {
            for i in 0..size * 30 {
                document.layers[0].strokes.push(stroke(
                    [
                        Brush::Land,
                        Brush::Forest,
                        Brush::Erase,
                        Brush::River,
                        Brush::Erase,
                    ][i % 5],
                    i,
                    vec![
                        Point::new((i % 10) as f32 * 60., (i % 7) as f32 * 60.),
                        Point::new((i % 10) as f32 * 60. + 100., (i % 7) as f32 * 60. + 40.),
                    ],
                    110.,
                ));
            }
        }
        "long-gesture" => {
            document.layers[0].strokes.push(stroke(
                Brush::Mountain,
                0,
                (0..size * 120)
                    .map(|i| Point::new(i as f32 * 5. - 600., (i as f32 * 0.05).sin() * 150.))
                    .collect(),
                120.,
            ));
        }
        _ => return Err(format!("Unknown performance fixture: {name}")),
    }
    document.validate()?;
    Ok(document)
}
