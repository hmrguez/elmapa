#define_import_path elmapa::water_surface

const TAU: f32 = 6.2831853;

fn wave(point: vec2<f32>, frequency: vec2<f32>, hour: f32, cycles: f32) -> f32 {
    // The global clock wraps after one hour. Whole cycles keep the wave continuous.
    return sin(TAU * (dot(point, frequency) - hour * cycles));
}

fn resolve_detail(footprint: f32, wavelength: f32) -> f32 {
    // Fade detail once the wavelength spans fewer than eight physical pixels.
    return 1.0 - smoothstep(wavelength / 8.0, wavelength / 3.0, footprint);
}

fn ocean_hash(cell: vec2<f32>) -> f32 {
    let seed = fract(vec3<f32>(cell.x, cell.y, cell.x) * 0.1031);
    let mixed = seed + dot(seed, seed.yzx + 33.33);
    return fract((mixed.x + mixed.y) * mixed.z);
}

fn ocean_noise(point: vec2<f32>) -> f32 {
    let cell = floor(point);
    let fraction = fract(point);
    let blend = fraction * fraction * (3.0 - 2.0 * fraction);
    let top = mix(ocean_hash(cell), ocean_hash(cell + vec2(1.0, 0.0)), blend.x);
    let bottom = mix(ocean_hash(cell + vec2(0.0, 1.0)), ocean_hash(cell + vec2(1.0)), blend.x);
    return mix(top, bottom, blend.y);
}

fn ocean_light(point: vec2<f32>, hour: f32, footprint: f32) -> vec3<f32> {
    // Broad swells carry narrow, broken lines of ink in their wake.
    let bend = ocean_noise(point * vec2(0.023, 0.007));
    let phase = dot(point, vec2(0.0008, 0.014)) - hour * 487.0 + bend * 0.21;
    let swell = cos(TAU * phase) * 0.013 * resolve_detail(footprint, 71.0);
    let cross = wave(point, vec2(-0.0028, 0.0097), hour, 373.0);
    let exposure = ocean_noise(point * vec2(0.017, 0.021) + vec2(cross * 0.25, 7.0));
    let broken = smoothstep(0.46, 0.73, exposure);
    let width = mix(0.55, 1.45, exposure);
    let distance = (fract(phase + 0.5) - 0.5) * 71.0;
    let pixel = max(footprint, 0.35);
    let crest = clamp((width + pixel * 0.5 - abs(distance)) / pixel, 0.0, min(1.0, 2.0 * width / pixel));
    let highlight = 1.0 - smoothstep(0.0, 3.7 + pixel, abs(distance - 3.4));
    let ink = broken * (0.025 * highlight - 0.16 * crest) * resolve_detail(footprint, 17.0);
    return vec3(swell + ink);
}

fn surface_light(profile: u32, point: vec2<f32>, time: f32, footprint: f32) -> vec3<f32> {
    let hour = time / 3600.0;
    switch profile {
        case 1u: {
            return ocean_light(point, hour, footprint);
        }
        case 2u: {
            // Shallow Ocean. Short ripples taper into broken runs.
            let choppy = wave(point, vec2(0.0063, 0.0236), hour, 832.0);
            let broken = smoothstep(-0.55, 0.65, wave(point, vec2(0.016, -0.003), hour, -213.0));
            let undercurrent = wave(point, vec2(-0.0062, 0.0164), hour, 598.0);
            let light = 0.017 * choppy * broken * resolve_detail(footprint, 41.0)
                + 0.003 * undercurrent * resolve_detail(footprint, 57.0);
            return vec3(light);
        }
        case 3u: {
            // Lake. Broad, faint undulations with a quiet, slightly crosswise echo.
            let slow = wave(point, vec2(0.0013, 0.0108), hour, 308.0);
            let echo = wave(point, vec2(0.0059, 0.0037), hour, -199.0);
            return vec3((0.007 * slow + 0.0035 * echo) * resolve_detail(footprint, 92.0));
        }
        case 4u: {
            // Oasis Water. A sheltered surface with occasional soft glints.
            let drift = wave(point, vec2(0.0011, 0.0128), hour, 167.0);
            let shelter = smoothstep(-0.15, 0.80, wave(point, vec2(0.008, 0.003), hour, 83.0));
            let glint = drift * drift * drift * shelter;
            return vec3(0.0045 * glint * resolve_detail(footprint, 78.0));
        }
        case 5u: {
            // Glacial Water. Fine mineral-blue crests with a pale, restrained highlight.
            let ripple = wave(point, vec2(-0.0067, 0.024), hour, 600.0);
            let patches = 0.55 + 0.45 * wave(point, vec2(0.0127, 0.0011), hour, 228.0);
            let light = ripple * ripple * ripple * 0.013 * patches;
            return light * vec3(0.62, 0.88, 1.16) * resolve_detail(footprint, 40.0);
        }
        default: {
            return vec3(0.0);
        }
    }
}
