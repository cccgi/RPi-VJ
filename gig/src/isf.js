// ISF-lite parser & GLSL 3.00 ES shim for RPi-VJ
// Zero dependencies, ES module.

const BASE_HEADER = `#version 300 es
precision highp float;
out vec4 isf_out;
#define gl_FragColor isf_out
#define texture2D texture
#define isf_FragNormCoord (gl_FragCoord.xy / u_resolution)
#define vv_FragNormCoord isf_FragNormCoord
`;

// Uniforms engine always supplies if not declared in shader body
const WANT_UNIFORMS = [
  'u_time',
  'u_resolution',
  'u_bass',
  'u_mid',
  'u_treble',
  'u_onset',
  'u_volume',
  'u_beat_phase',
  'u_bar_phase',
  'u_energy',
  'u_kick',
  'u_spectrum',
  // Five-band drive: give each visual layer its own frequency range.
  'u_sub',
  'u_low_mid',
  'u_high_mid',
  // Independent transients: kick / snare-clap / hats.
  'u_hit_low',
  'u_hit_mid',
  'u_hit_high',
  // Tempo-lock confidence (0 = clock is free-running, ignore beat phase)
  'u_locked',
  // Audio-INTEGRATED clocks. Use instead of u_time for anything whose SPEED
  // should follow the music; they never jump because rate, not time, is
  // what the audio scales.
  'u_flow',
  'u_flow_low',
  'u_flow_mid',
  'u_flow_high',
  'u_spec_flow',
  // Audio engine extensions if needed / referenced
  'uSubBass',
  'uBass',
  'uLowMid',
  'uHighMid',
  'uTreble',
  'uEnergy',
  'uKick',
  'uBPM',
  'uBeatPhase',
  'uBarPhase',
  // Standard ISF names if referenced
  'RENDERSIZE',
  'TIME',
  'TIMEDELTA',
  'FRAMEINDEX',
  'DATE'
];

const UNIFORM_TYPES = {
  u_resolution: 'vec2',
  RENDERSIZE: 'vec2',
  DATE: 'vec4',
  FRAMEINDEX: 'int'
};

const ISF_TYPE_MAP = {
  float: 'float',
  bool: 'bool',
  long: 'int',
  int: 'int',
  point2D: 'vec2',
  color: 'vec4',
  image: 'sampler2D',
  event: 'bool'
};

/**
 * Extracts JSON metadata header and body from ISF source.
 * @param {string} src
 * @returns {{meta: Object|null, inputs: Array, body: string}}
 */
function parseHeader(src) {
  const m = src.indexOf('/*');
  if (m < 0) return { meta: null, inputs: [], body: src };
  const e = src.indexOf('*/', m);
  if (e < 0) return { meta: null, inputs: [], body: src };

  const raw = src.slice(m + 2, e).trim();
  let meta = null;
  if (raw.startsWith('{') && raw.endsWith('}')) {
    try {
      meta = JSON.parse(raw);
    } catch (err) {
      throw new Error('Malformed ISF JSON header: ' + err.message);
    }
  }

  const body = src.slice(0, m) + src.slice(e + 2);
  const inputs = (meta && Array.isArray(meta.INPUTS)) ? meta.INPUTS : [];
  return { meta, inputs, body };
}

/**
 * Parses an ISF fragment shader source, generating the GLSL 3.00 ES header,
 * input uniform declarations, and default values.
 *
 * @param {string} source
 * @returns {{
 *   meta: Object|null,
 *   inputs: Array<{NAME: string, TYPE: string, DEFAULT?: any, MIN?: any, MAX?: any, LABEL?: string}>,
 *   defaults: Array<{name: string, type: string, val: any}>,
 *   glsl: string
 * }}
 */
export function parseISF(source) {
  if (typeof source !== 'string') {
    throw new Error('parseISF source must be a string');
  }

  const { meta, inputs, body: rawBody } = parseHeader(source);

  // Strip any existing #version directive and clean up
  let body = rawBody.replace(/^\s*#version[^\n]*\n?/m, '');

  // GLSL ES 1.00 -> 3.00 fixups if present in body
  body = body.replace(/\bvarying\s+/g, 'in ');

  const decls = [];
  const defaults = [];
  const declaredNames = new Set();

  // 1. Engine standard uniforms: declare if used or expected, and not already in body
  // Check what uniforms the body already declares
  const uniformDeclRegex = /uniform\s+([a-zA-Z0-9_]+)\s+([a-zA-Z0-9_]+)\s*(?:\[\d+\])?\s*;/g;
  let match;
  while ((match = uniformDeclRegex.exec(body)) !== null) {
    declaredNames.add(match[2]);
  }

  // 2. Uniforms declared in ISF header INPUTS
  for (const inp of inputs) {
    if (!inp || !inp.NAME) continue;
    const name = inp.NAME;
    const mappedType = ISF_TYPE_MAP[inp.TYPE] || 'float';
    if (!declaredNames.has(name)) {
      decls.push(`uniform ${mappedType} ${name};`);
      declaredNames.add(name);
    }
    defaults.push({
      name: inp.NAME,
      type: mappedType,
      val: inp.DEFAULT !== undefined ? inp.DEFAULT : (inp.TYPE === 'color' ? [1, 1, 1, 1] : 0)
    });
  }

  // 3. Core engine uniforms (u_time, u_resolution, etc.) if not declared
  for (const name of WANT_UNIFORMS) {
    if (!declaredNames.has(name)) {
      // Only emit if it's in standard set
      const type = UNIFORM_TYPES[name] || 'float';
      // u_spectrum is a 32-element array, not a scalar.
      decls.push((name === 'u_spectrum' || name === 'u_spec_flow')
        ? `uniform float ${name}[32];`
        : `uniform ${type} ${name};`);
      declaredNames.add(name);
    }
  }

  const declHeader = decls.length > 0 ? decls.join('\n') + '\n' : '';
  const glsl = `${BASE_HEADER}${declHeader}#line 1\n${body}`;

  return {
    meta: meta || {},
    inputs,
    defaults,
    glsl
  };
}
