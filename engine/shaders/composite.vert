#version 300 es
precision highp float;
// Compositor vertex stage: a 64x36 grid generated from gl_VertexID, no buffers. The
// shader-layer warp is evaluated here, at the grid points, and interpolated, so bending
// the layer costs nothing per pixel.
out vec2 v_guv;
uniform vec2 u_resolution;
uniform vec4 u_geo;     // bulge, angle, twist
uniform vec4 u_geo2;    // ripple amplitude, ripple phase, mirror segments
uniform vec4 u_geo3;    // wave amplitude, wave phase, canvas scroll x and y
const int COLS = 64;
const int ROWS = 36;

// Every term varies across the picture, so the layer bends and flows instead of scaling
// as one piece: a bulge that zooms the centre more than the edges, a twist that turns the
// centre against the edges, crossing waves and radial ripples.
vec2 warp(vec2 uv) {
  float aspect = u_resolution.x / u_resolution.y;
  vec2 p = (uv - 0.5) * vec2(aspect, 1.0);
  float r = length(p);
  p *= 1.0 - u_geo.x * exp(-r * r * 2.5);
  float ang = u_geo.y + u_geo.z * (0.6 - r);
  float c = cos(ang), s = sin(ang);
  p = mat2(c, -s, s, c) * p;
  // The scroll is an offset in canvas coordinates, added after the turn; the renderer
  // keeps it so that the canvas still spins about its own centre. Under the mirror fold
  // it is added after the fold instead, per pixel.
  if (u_geo2.z < 1.5) p += u_geo3.zw;
  p += vec2(sin(p.y * 7.0 + u_geo3.y), cos(p.x * 5.0 + u_geo3.y * 0.8) * 0.6) * u_geo3.x;
  if (r > 0.0001) p += (p / r) * sin(r * 30.0 - u_geo2.y) * u_geo2.x;
  return p / vec2(aspect, 1.0) + 0.5;
}

void main() {
  int quad = gl_VertexID / 6;
  int corner = gl_VertexID - quad * 6;
  vec2 cell = vec2(float(quad - (quad / COLS) * COLS), float(quad / COLS));
  vec2 offset = (corner == 0) ? vec2(0.0, 0.0) : (corner == 1 || corner == 3) ? vec2(1.0, 0.0)
              : (corner == 2 || corner == 5) ? vec2(0.0, 1.0) : vec2(1.0, 1.0);
  vec2 p = (cell + offset) / vec2(float(COLS), float(ROWS));
  v_guv = warp(p);
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
