struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    var pos = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );

    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
    );

    var out: VertexOutput;
    out.clip_position = vec4<f32>(pos[idx], 0.0, 1.0);
    out.uv = uvs[idx];
    return out;
}

struct Uniforms {
    time: f32,
    _pad0: f32,
    resolution: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

const U_BASS: f32 = 0.3;
const U_MID: f32 = 0.2;
const U_TREBLE: f32 = 0.1;
const U_ONSET: f32 = 0.0;
const GRID_SPEED: f32 = 1.0;
const MOUNTAIN_HEIGHT: f32 = 1.5;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let aspect = u.resolution.x / u.resolution.y;
    let uv = vec2<f32>((in.uv.x - 0.5) * aspect, 0.5 - in.uv.y);

    if (uv.y > 0.0) {
        // Horizon / Sky with audio sun
        let sun_dist = length(uv - vec2<f32>(0.0, 0.15));
        let sun = smoothstep(0.18 + U_BASS * 0.22, 0.16 - U_BASS * 0.05, sun_dist);
        var sun_col = mix(vec3<f32>(1.0, 0.1, 0.4), vec3<f32>(1.0, 0.8, 0.0), (uv.y - 0.15) / 0.18);
        sun_col += vec3<f32>(U_ONSET * 1.1);
        return vec4<f32>(sun_col * sun, 1.0);
    }

    // 3D Perspective Plane
    let depth = -0.4 / uv.y;
    let x = uv.x * depth;
    let z = depth + u.time * (GRID_SPEED + U_BASS * 3.2);

    // Audio-reactive terrain elevation
    let terrain = sin(x * 1.5) * cos(z * 0.8) * (U_BASS * MOUNTAIN_HEIGHT * 2.6);

    // Grid lines
    let grid_uv = fract(vec2<f32>(x, z + terrain)) - vec2<f32>(0.5, 0.5);
    let line_x = smoothstep(0.06, 0.0, abs(grid_uv.x));
    let line_z = smoothstep(0.06, 0.0, abs(grid_uv.y));
    let grid = max(line_x, line_z) * (1.0 / (depth * 0.4 + 1.0));

    var col = mix(vec3<f32>(0.0, 0.8, 1.0), vec3<f32>(1.0, 0.1, 0.6), sin(z * 0.2 + u.time) * 0.5 + 0.5);
    col *= grid * (1.5 + U_MID);
    col += vec3<f32>(U_ONSET * 0.9 + U_BASS * 0.25);

    return vec4<f32>(col, 1.0);
}
