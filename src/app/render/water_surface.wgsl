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

fn surface_light(profile: u32, point: vec2<f32>, time: f32, footprint: f32) -> vec3<f32> {
    let hour = time / 3600.0;
    switch profile {
        case 1u: {
            // Lake. Broad, faint undulations with a quiet, slightly crosswise echo.
            let slow = wave(point, vec2(0.0013, 0.0108), hour, 308.0);
            let echo = wave(point, vec2(0.0059, 0.0037), hour, -199.0);
            return vec3((0.007 * slow + 0.0035 * echo) * resolve_detail(footprint, 92.0));
        }
        case 2u: {
            // Oasis Water. A sheltered surface with occasional soft glints.
            let drift = wave(point, vec2(0.0011, 0.0128), hour, 167.0);
            let shelter = smoothstep(-0.15, 0.80, wave(point, vec2(0.008, 0.003), hour, 83.0));
            let glint = drift * drift * drift * shelter;
            return vec3(0.0045 * glint * resolve_detail(footprint, 78.0));
        }
        case 3u: {
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
