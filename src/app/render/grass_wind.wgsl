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

fn patch_cover(point: vec2<f32>) -> f32 {
    let corner = floor(point);
    let weight = smoothstep(vec2(0.0), vec2(1.0), fract(point));
    let top = mix(grass_random(corner).x, grass_random(corner + vec2(1.0, 0.0)).x, weight.x);
    let bottom = mix(grass_random(corner + vec2(0.0, 1.0)).x, grass_random(corner + vec2(1.0)).x, weight.x);
    return smoothstep(0.25, 0.72, mix(top, bottom, weight.y));
}

fn meadow_ink(tuft: vec2<f32>, reach: f32, spread: f32, bend: f32, habit: vec3<f32>, family: u32, pixel: f32) -> f32 {
    let x = tuft.x;
    let h = tuft.y;
    let side = select(-1.0, 1.0, habit.y > 0.5);
    if family == 0u {
        // Low, compact fans with uneven tips and a wide base.
        let outer = blade_ink(h, x + 0.94 * spread, reach * mix(0.55, 0.82, habit.y), bend * 0.55 - 1.6 * spread, 0.96, pixel);
        let inner = blade_ink(h, x + 0.31 * spread, reach, bend * 0.72 - 0.62 * spread, 1.12, pixel);
        let blade = blade_ink(h, x - 0.29 * spread, reach * mix(0.74, 0.96, habit.z), bend * 0.72 + 0.58 * spread, 1.02, pixel);
        let skirt = blade_ink(h, x - 0.91 * spread, reach * mix(0.62, 0.86, habit.x), bend * 0.55 + 1.8 * spread, 0.9, pixel);
        return max(max(outer, inner), max(blade, skirt));
    }
    if family == 1u {
        // Loose bunches lean to either side, with one long arch and shorter leaves.
        let arch = blade_ink(h, x - 0.2 * side, reach, bend + 1.35 * side * spread, 1.23, pixel);
        let leaf = blade_ink(h, x + 0.81 * side * spread, reach * mix(0.48, 0.78, habit.x), bend * 0.7 - 1.82 * side * spread, 1.16, pixel);
        if habit.y < 0.65 {
            let low = blade_ink(h, x - 0.83 * side * spread, reach * 0.54, bend * 0.61 + 2.0 * side * spread, 0.83, pixel);
            return max(arch, max(leaf, low));
        }
        return max(arch, leaf);
    }
    // Taller stems with tapered ink seed heads above sparse foliage.
    let stem = blade_ink(h, x, reach, bend, 0.88, pixel);
    let leaf = blade_ink(h, x + 0.62 * side * spread, reach * mix(0.39, 0.62, habit.z), bend * 0.55 - 1.7 * side * spread, 1.04, pixel);
    let grain = vec2(x - bend - side * (0.22 + 0.32 * (h - reach)), h - reach);
    let radius = vec2(mix(0.52, 0.79, habit.z), mix(0.95, 1.34, habit.x));
    let unit = grain / radius;
    let length_to_center = length(unit);
    let gradient = length(unit / radius) / max(0.001, length_to_center);
    let distance = (length_to_center - 1.0) / max(0.001, gradient);
    let seeds = clamp(0.5 - distance / pixel, 0.0, min(1.0, 2.0 * radius.x / pixel));
    return max(max(stem, leaf), seeds * 0.82);
}

fn meadow_shade(world: vec2<f32>, pixel: f32, time: f32) -> f32 {
    let detail = 1.0 - smoothstep(1.05, 2.65, pixel);
    if detail <= 0.0 {
        return 1.0;
    }
    // Curve the sampling lattice gently so neighboring tufts do not line up.
    // Map coordinates keep the field continuous across strokes and terrain tiles.
    let point = world + vec2(9.0 * sin(world.y * 0.028), 8.0 * sin(world.x * 0.026));
    let spacing = 18.0;
    let cell = floor(point / spacing);
    let random = grass_random(cell + vec2(41.0, -31.0));
    let cover = patch_cover(cell * vec2(0.17, 0.23) + vec2(6.3, -4.8));
    if random.z > mix(0.025, 0.91, cover) {
        return 1.0;
    }
    let habit = grass_random(cell + vec2(-19.0, 61.0));
    // Vary the local mix independently of coverage, so each patch can still
    // contain several kinds of grass.
    let habitat = patch_cover(cell * vec2(0.12, 0.14) + vec2(-8.4, 12.7));
    let kind = habit.z + mix(-0.24, 0.24, habitat);
    let family = select(select(1u, 2u, kind >= 0.77), 0u, kind < 0.41);
    let growth = select(select(1.08, 1.34, family == 2u), 0.72, family == 0u);
    let reach = mix(4.4, 7.8, random.x) * mix(0.9, 1.16, cover) * growth;
    let spread = mix(0.72, 1.18, random.y);
    let lean = mix(-1.42, 1.42, habit.x);
    // Keep blades and their antialiasing footprint within the sampling cell.
    let margin = 4.1 + abs(lean) + 2.2 * spread;
    let root = cell * spacing + mix(vec2(margin, reach + 2.9), vec2(spacing - margin, spacing - 1.0), random.yx);
    let tuft = vec2(point.x - root.x, root.y - point.y);
    if abs(tuft.x) > margin || tuft.y < -0.4 || tuft.y > reach + 2.9 {
        return 1.0;
    }
    let wind = grass_breeze(root, time);
    let bend = lean + 0.35 + wind.x * mix(0.79, 1.21, habit.y) + 0.36 * wind.y;
    let strength = mix(0.27, 0.45, random.x) * select(1.0, 0.85, family == 2u);
    return 1.0 - detail * meadow_ink(tuft, reach, spread, bend, habit, family, pixel) * strength;
}

fn grass_breeze(root: vec2<f32>, time: f32) -> vec2<f32> {
    // The time uniform wraps hourly. Use whole cycles across that period.
    let cycles = time / 3600.0;
    let breeze = sin(6.2831853 * fract(cycles * 607.0 - dot(root, vec2<f32>(0.0032, 0.0016))));
    let gust = sin(6.2831853 * fract(cycles * 871.0 - dot(root, vec2<f32>(0.0055, -0.0021))));
    return vec2(breeze, gust);
}

fn grass_shade(world: vec2<f32>, pixel: f32, time: f32, heath: bool) -> f32 {
    if !heath {
        return meadow_shade(world, pixel, time);
    }
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
