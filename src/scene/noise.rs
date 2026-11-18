use crate::document::Point;

pub(super) fn hash(mut x: u64) -> f32 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58476d1ce4e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d049bb133111eb);
    ((x ^ (x >> 31)) >> 40) as f32 / 16_777_215.0
}
pub(super) fn noise(q: Point, seed: u64) -> f32 {
    let phase = hash(seed) * 30.0;
    (q.x * 0.021 + phase).sin() * (q.y * 0.028 - phase).cos() * 0.56
        + (q.x * 0.068 + q.y * 0.045 + phase).sin() * 0.27
        + (q.x * 0.139 - q.y * 0.111 + phase).cos() * 0.17
}
