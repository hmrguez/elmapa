#define_import_path elmapa::dune_wind

fn dune_hash(cell: vec2<f32>) -> f32 {
    let seed = fract(cell.xyx * vec3(0.1031, 0.1030, 0.0973));
    let mixed = seed + dot(seed, seed.yzx + 33.33);
    return fract((mixed.x + mixed.y) * mixed.z);
}

fn dune_noise(point: vec2<f32>) -> f32 {
    let cell = floor(point);
    let fraction = fract(point);
    let blend = fraction * fraction * (3.0 - 2.0 * fraction);
    return mix(
        mix(dune_hash(cell), dune_hash(cell + vec2(1.0, 0.0)), blend.x),
        mix(dune_hash(cell + vec2(0.0, 1.0)), dune_hash(cell + vec2(1.0, 1.0)), blend.x),
        blend.y,
    );
}

fn dune_light(point: vec2<f32>, time: f32, footprint: f32) -> vec3<f32> {
    let dune_detail = 1.0 - smoothstep(10.0, 28.0, footprint);
    if dune_detail <= 0.0 {
        return vec3(0.0);
    }
    let along = dot(point, vec2(0.97, -0.24));
    let across = dot(point, vec2(0.24, 0.97));
    let wind_field = vec2(along / 148.0, across / 215.0);
    let bend = dune_noise(wind_field) - 0.5;
    let exposure = smoothstep(0.16, 0.68, dune_noise(wind_field + vec2(12.8, 3.6)));

    // Move the crests downwind while local bends hold their shape. Whole cycles
    // keep the ridges continuous when the shader clock wraps after an hour.
    let hour = time / 3600.0;
    let phase = fract(across / 96.0 + bend * 1.65 - hour * 79.0);
    let windward = smoothstep(0.06, 0.43, phase) * (1.0 - smoothstep(0.47, 0.57, phase));
    let crest = smoothstep(0.33, 0.48, phase) * (1.0 - smoothstep(0.48, 0.57, phase));
    let lee = smoothstep(0.50, 0.63, phase) * (1.0 - smoothstep(0.65, 0.94, phase));
    let relief = (windward * 0.012 + crest * 0.021 - lee * 0.034)
        * mix(0.22, 1.0, exposure) * dune_detail;

    // Small ripples travel faster over the open slopes. Fade them as the
    // sampling footprint approaches their wavelength so they do not shimmer.
    let ripple_detail = 1.0 - smoothstep(1.3, 3.5, footprint);
    let ripple_phase = fract(across / 13.0 + bend * 3.2 - hour * 1783.0);
    let ripple = sin(6.2831853 * ripple_phase) * (0.002 + 0.005 * exposure)
        * (1.0 - lee) * ripple_detail;
    return (relief + ripple) * vec3(0.92, 1.0, 1.12);
}
