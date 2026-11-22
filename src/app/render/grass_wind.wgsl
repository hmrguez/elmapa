#define_import_path elmapa::grass_wind

fn blade_ink(height: f32, x: f32, reach: f32, bend: f32, width: f32, pixel: f32) -> f32 {
    let tip = clamp(height / reach, 0.0, 1.0);
    let distance = abs(x - bend * tip * tip);
    let half_width = width * mix(0.48, 0.035, tip);
    let line = clamp((half_width + pixel * 0.5 - distance) / pixel, 0.0, min(1.0, 2.0 * half_width / pixel));
    return line * smoothstep(-0.4, 0.9, height) * (1.0 - smoothstep(reach - pixel, reach + pixel, height));
}

fn grass_random(cell: vec2<f32>) -> vec3<f32> {
    let seed = fract(vec3<f32>(cell.x, cell.y, cell.x + cell.y) * vec3<f32>(0.1031, 0.1030, 0.0973));
    let mixed = seed + dot(seed, seed.yzx + 33.33);
    return fract((mixed.xxy + mixed.yzz) * mixed.zyx);
}

fn grass_breeze(root: vec2<f32>, time: f32) -> vec2<f32> {
    // The time uniform wraps hourly. Use whole cycles across that period.
    let cycles = time / 3600.0;
    let breeze = sin(6.2831853 * fract(cycles * 607.0 - dot(root, vec2<f32>(0.0032, 0.0016))));
    let gust = sin(6.2831853 * fract(cycles * 871.0 - dot(root, vec2<f32>(0.0055, -0.0021))));
    return vec2(breeze, gust);
}

fn heath_shade(world: vec2<f32>, pixel: f32, time: f32) -> f32 {
    // At small scales the paper tile carries the artwork. Fade the hatching
    // before the blades become subpixel marks that shimmer during a pan.
    let detail = 1.0 - smoothstep(0.9, 2.25, pixel);
    if detail <= 0.0 {
        return 1.0;
    }
    let spacing = 23.0;
    let cell = floor(world / spacing);
    let random = grass_random(cell);
    if random.z < 0.45 {
        return 1.0;
    }
    let root = (cell + mix(vec2<f32>(0.31, 0.66), vec2<f32>(0.64, 0.84), random.xy)) * spacing;
    let point = world - root;
    let reach = mix(4.7, 7.4, random.z) * 0.72;
    if abs(point.x) > 6.2 || point.y > 1.5 || point.y < -reach - 1.5 {
        return 1.0;
    }
    let wind = grass_breeze(root, time);
    let bend = (0.8 + 1.5 * wind.x + 0.48 * wind.y) * 0.7;
    // Each blade curves away from the same root, where the deflection is zero.
    let left = blade_ink(-point.y, point.x + 0.7, reach * 0.72, bend - 2.1, 1.0, pixel);
    let middle = blade_ink(-point.y, point.x, reach, bend + 0.4, 1.0, pixel);
    let right = blade_ink(-point.y, point.x - 0.7, reach * 0.81, bend + 2.0, 1.0, pixel);
    let ink = max(left, max(middle, right));
    return 1.0 - detail * ink * 0.14;
}
