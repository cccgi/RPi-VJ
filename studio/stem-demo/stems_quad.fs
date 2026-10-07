/*{
    "DESCRIPTION": "Stem compositor: four pre-rendered 960x540 stems in one video frame, mixed by live audio",
    "CATEGORIES": ["Stems", "Audio-Reactive"]
}*/
// PROPOSED: needs the engine to bind the active deck texture as `u_video` in the
// shader pass. Not runnable on the Pi until that lands; not yet measured.
// Layout of the video frame (uv origin bottom-left): TL base, TR bass, BL mid, BR high.

uniform sampler2D u_video;

vec3 stem(vec2 uv, vec2 cell) {
    // Half-texel inset keeps bilinear taps and codec bleed out of the neighbour stem.
    vec2 q = clamp(uv, vec2(0.004), vec2(0.996));
    return texture(u_video, (q + cell) * 0.5).rgb;
}

vec3 hue(float h) {
    return 0.5 + 0.5 * cos(6.2831853 * (h + vec3(0.0, 0.33, 0.67)));
}

void main() {
    vec2 uv = gl_FragCoord.xy / u_resolution;
    vec2 c = uv - 0.5;

    // BASE breathes with overall level.
    vec3 base = stem(0.5 + c / (1.0 + 0.06 * u_volume), vec2(0.0, 1.0));
    base *= 0.35 + 0.65 * u_volume;

    // BASS stem punches in scale and brightness on low-band hits.
    float punch = 1.0 + 0.35 * u_bass + 0.25 * u_hit_low;
    float bass = stem(0.5 + c / punch, vec2(1.0, 1.0)).r;
    vec3 bassCol = hue(0.02 + 0.08 * u_flow_low) * bass * (0.15 + 1.6 * u_bass);

    // MID stem gets a horizontal chroma split that opens with the mid band.
    float split = 0.012 * u_mid;
    vec3 mid = vec3(stem(uv + vec2(split, 0.0), vec2(0.0, 0.0)).r,
                    stem(uv, vec2(0.0, 0.0)).g,
                    stem(uv - vec2(split, 0.0), vec2(0.0, 0.0)).b) * (0.1 + 1.2 * u_mid);

    // HIGH stem sparkles with treble and flashes on onsets.
    float high = stem(uv, vec2(1.0, 0.0)).r;
    vec3 highCol = vec3(0.8, 0.95, 1.0) * high * (0.05 + 1.5 * u_treble + 1.2 * u_onset);

    gl_FragColor = vec4(base + bassCol + mid + highCol, 1.0);
}
