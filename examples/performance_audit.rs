//! CPU-only audit probes. These do not measure GPU submission or displayed FPS.
use elmapa::{
    document::{Brush, Document, Point, Stroke},
    editor::Editor,
    scene::{ForestAnimation, SceneCache, to_svg},
};
use std::{hint::black_box, time::Instant};

fn measure<T>(name: &str, n: usize, mut f: impl FnMut() -> T) {
    let mut times = Vec::with_capacity(n);
    for _ in 0..n {
        let start = Instant::now();
        black_box(f());
        times.push(start.elapsed().as_secs_f64() * 1000.);
    }
    times.sort_by(f64::total_cmp);
    println!(
        "{name}: n={n}, median={:.3} ms, p95={:.3} ms, max={:.3} ms",
        times[n / 2],
        times[((n as f64 * 0.95).ceil() as usize - 1).min(n - 1)],
        times[n - 1]
    );
}

fn main() {
    println!(
        "profile={}",
        if cfg!(debug_assertions) {
            "dev"
        } else {
            "release"
        }
    );
    let doc = Document::demo();
    let mut cache = SceneCache::default();
    let scene = cache.render(&doc, None);
    println!(
        "demo: {} layers, {} strokes, {} trees, {} retained shapes, {} retained vertices (forest expansion excluded)",
        doc.layers.len(),
        doc.layers.iter().map(|l| l.strokes.len()).sum::<usize>(),
        scene.forest_trees().count(),
        scene.batches().map(|batch| batch.len()).sum::<usize>(),
        scene
            .batches()
            .flat_map(|batch| batch.iter())
            .map(|shape| shape.points().count())
            .sum::<usize>()
    );
    measure("demo scene cold (specimens warm)", 5, || {
        SceneCache::default().render(&doc, None)
    });
    measure("unchanged scene cache", 100, || cache.render(&doc, None));
    measure("scene clone", 100, || scene.clone());
    for final_point_rendered in [true, false] {
        let mut doc = Document::demo();
        let id = doc.layers[1].id;
        let mut cache = SceneCache::default();
        cache.render(&doc, None);
        let mut stroke = Stroke {
            creation_id: None,
            material: None,
            brush: Brush::Forest,
            points: vec![Point::new(-500., -500.)].into(),
            size: 110.,
            strength: 0.85,
            seed: 98765,
        };
        let live = cache.render(&doc, Some((id, &stroke)));
        if !final_point_rendered {
            stroke.points.push(Point::new(-490., -500.));
        }
        doc.layers[1].strokes.push(stroke);
        let start = Instant::now();
        let committed = cache.render(&doc, None);
        let reused = committed
            .batches()
            .filter(|batch| {
                committed.forest_tree(batch).is_some()
                    && live.batches().any(|old| std::sync::Arc::ptr_eq(old, batch))
            })
            .count();
        println!(
            "release final_point_rendered={final_point_rendered}: {:.3} ms geometry, {reused}/{} forest batches reused",
            start.elapsed().as_secs_f64() * 1000.,
            committed.forest_trees().count()
        );
        if !final_point_rendered
            && std::env::args().any(|arg| arg == "--check-release-reuse")
            && reused < live.forest_trees().count()
        {
            std::process::exit(1);
        }
    }
    let mut motion = ForestAnimation::default();
    motion.sync(&scene, false, 0.);
    measure("animation clone", 100, || motion.clone());
    measure("demo SVG", 3, || to_svg(&doc));
    let mut large = Document::blank();
    for i in 0..1000 {
        large.layers[0].strokes.push(Stroke {
            creation_id: None,
            material: None,
            brush: Brush::Path,
            points: (0..100)
                .map(|j| Point::new(j as f32 * 4., i as f32))
                .collect(),
            size: 12.,
            strength: 0.85,
            seed: i,
        });
    }
    println!(
        "history fixture: 1000 strokes / 100000 points / at least 800000 point bytes per snapshot"
    );
    measure("100k-point document clone", 20, || large.clone());
    measure("100k-point document validation", 20, || large.validate());
    measure("100k-point document JSON encode", 10, || {
        serde_json::to_vec(&large)
    });
    let mut editor = Editor::new(large).unwrap();
    let id = editor.active_layer();
    let mut opacity = 0.5;
    measure("100k-point opacity edit + undo checkpoint", 20, || {
        opacity = if opacity == 0.5 { 0.6 } else { 0.5 };
        editor.set_layer_opacity(id, opacity)
    });
}
