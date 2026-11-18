//! Portable work-count evidence. Deliberately contains no wall-clock thresholds.
use elmapa::{
    document::{Brush, Point, Stroke},
    performance_fixtures::{WORKLOADS, fixture},
    scene::{
        SceneCache,
        generation::{self, Limits},
    },
};
fn main() {
    for size in [1, 4, 16] {
        for workload in WORKLOADS {
            let document = fixture(workload, size).unwrap();
            let mut cache = SceneCache::default();
            let result =
                generation::bounded(Limits::INTERACTIVE, None, || cache.render(&document, None));
            match result {
                Ok(scene) => {
                    println!(
                        "{workload} size={size}: trees={}, retained_shapes={}, work={:?}",
                        scene.forest_trees().count(),
                        scene.batches().map(|b| b.len()).sum::<usize>(),
                        cache.work()
                    );
                    cache.render(&document, None);
                    assert_eq!(cache.work(), Default::default());
                    let stroke = Stroke {
                        creation_id: None,
                        material: None,
                        brush: Brush::Forest,
                        points: vec![Point::new(-600., -600.)].into(),
                        size: 110.,
                        strength: 0.85,
                        seed: 98765,
                    };
                    let _ = generation::bounded(Limits::INTERACTIVE, None, || {
                        cache.render(&document, Some((document.layers[0].id, &stroke)))
                    });
                    assert_eq!(
                        cache.work().field_samples,
                        0,
                        "forest gesture sampled region fields"
                    );
                }
                Err(error) => {
                    println!("{workload} size={size}: explicit generation limit: {error}")
                }
            }
        }
    }
}
