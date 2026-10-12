// WebGL2 renderer for result fields.
import { lut, TERMINAL_COLORS, type ColormapName } from "./colormaps";

const VS = `#version 300 es
in vec2 aPos;
uniform vec2 uOff;      // grid top-left minus camera center (µm)
uniform vec2 uSize;     // grid size (µm)
uniform float uScale;   // css px per µm
uniform vec2 uViewport; // css px
out vec2 vUv;
void main() {
  vec2 w = vec2(uOff.x + aPos.x * uSize.x, uOff.y - aPos.y * uSize.y);
  vec2 clip = w * uScale * 2.0 / uViewport;
  gl_Position = vec4(clip, 0.0, 1.0);
  vUv = aPos;
}`;

const FS = `#version 300 es
precision highp float;
precision highp sampler2D;
uniform sampler2D uVal;
uniform sampler2D uPot;
uniform sampler2D uMat;
uniform sampler2D uCov;
uniform sampler2D uLut;
uniform float uMin;
uniform float uMax;
uniform int uLog;
uniform int uIso;
uniform float uIsoStep;
uniform int uMode;       // 0 = field, 1 = geometry only
uniform vec3 uTerm[8];
uniform vec3 uCond;
uniform vec2 uTexSize;
in vec2 vUv;
out vec4 outColor;

// NaN handling is avoided on the GPU (fast-math drivers); validity comes from the material map.
float potAt(ivec2 p, float fallback) {
  p = clamp(p, ivec2(0), ivec2(uTexSize) - 1);
  float m = floor(texelFetch(uMat, p, 0).r * 255.0 + 0.5);
  bool ok = (m > 0.5 && m < 1.5) || m >= 9.5;
  return ok ? texelFetch(uPot, p, 0).r : fallback;
}

void main() {
  float m = floor(texture(uMat, vUv).r * 255.0 + 0.5);
  if (m < 0.5) discard;
  float cov = max(texture(uCov, vUv).r, 0.0);
  float a = clamp(cov, 0.0, 1.0);
  vec3 c;
  if (m >= 9.5) {
    int t = int(m - 10.0);
    c = uTerm[t < 8 ? t : 7];
  } else if (m > 1.5) {
    float s = mod(gl_FragCoord.x + gl_FragCoord.y, 10.0) < 5.0 ? 0.62 : 0.5;
    c = vec3(s);
  } else if (uMode == 1) {
    c = uCond;
  } else {
    float v = texture(uVal, vUv).r;
    float t;
    if (uLog == 1) {
      t = (log(max(v, 1e-30)) / log(10.0) - log(uMin) / log(10.0)) / (log(uMax) / log(10.0) - log(uMin) / log(10.0));
    } else {
      t = (v - uMin) / max(uMax - uMin, 1e-30);
    }
    t = clamp(t, 0.0, 1.0);
    c = texture(uLut, vec2(t * (255.0 / 256.0) + 0.5 / 256.0, 0.5)).rgb;
  }
  if (uIso == 1 && m < 1.5) {
    vec2 p = vUv * uTexSize - 0.5;
    ivec2 i0 = ivec2(floor(p));
    vec2 f = fract(p);
    float center = texture(uPot, vUv).r;
    {
      float v00 = potAt(i0, center), v10 = potAt(i0 + ivec2(1, 0), center);
      float v01 = potAt(i0 + ivec2(0, 1), center), v11 = potAt(i0 + ivec2(1, 1), center);
      float v = mix(mix(v00, v10, f.x), mix(v01, v11, f.x), f.y);
      float q = v / uIsoStep;
      float d = abs(fract(q - 0.5) - 0.5) / max(fwidth(q), 1e-6);
      float line = 1.0 - smoothstep(0.0, 1.2, d);
      c = mix(c, vec3(1.0), line * 0.85);
    }
  }
  outColor = vec4(c * a, a);
}`;

function compile(gl: WebGL2RenderingContext, type: number, src: string) {
  const s = gl.createShader(type)!;
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) ?? "shader");
  return s;
}

function hexToRgb(h: string): [number, number, number] {
  return [parseInt(h.slice(1, 3), 16) / 255, parseInt(h.slice(3, 5), 16) / 255, parseInt(h.slice(5, 7), 16) / 255];
}

export interface FieldUniforms {
  offX: number;
  offY: number;
  sizeX: number;
  sizeY: number;
  scale: number;
  viewW: number;
  viewH: number;
  min: number;
  max: number;
  log: boolean;
  iso: boolean;
  isoStep: number;
  geometryOnly: boolean;
  condColor: string;
}

export class FieldRenderer {
  private gl: WebGL2RenderingContext;
  private prog: WebGLProgram;
  private vao: WebGLVertexArrayObject;
  private tex: Record<"val" | "pot" | "mat" | "cov" | "lut", WebGLTexture>;
  private loc: Record<string, WebGLUniformLocation | null> = {};
  private nx = 0;
  private ny = 0;
  readonly maxTexture: number;
  hasData = false;

  constructor(canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl2", { premultipliedAlpha: true, preserveDrawingBuffer: true, antialias: false });
    if (!gl) throw new Error("WebGL2 nicht verfügbar");
    this.gl = gl;
    this.maxTexture = gl.getParameter(gl.MAX_TEXTURE_SIZE);
    const p = gl.createProgram()!;
    gl.attachShader(p, compile(gl, gl.VERTEX_SHADER, VS));
    gl.attachShader(p, compile(gl, gl.FRAGMENT_SHADER, FS));
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p) ?? "link");
    this.prog = p;
    for (const n of [
      "uOff", "uSize", "uScale", "uViewport", "uVal", "uPot", "uMat", "uCov", "uLut", "uMin", "uMax",
      "uLog", "uIso", "uIsoStep", "uMode", "uTerm", "uCond", "uTexSize",
    ])
      this.loc[n] = gl.getUniformLocation(p, n);
    this.vao = gl.createVertexArray()!;
    gl.bindVertexArray(this.vao);
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([0, 0, 1, 0, 0, 1, 1, 1]), gl.STATIC_DRAW);
    const a = gl.getAttribLocation(p, "aPos");
    gl.enableVertexAttribArray(a);
    gl.vertexAttribPointer(a, 2, gl.FLOAT, false, 0, 0);
    const mk = () => {
      const t = gl.createTexture()!;
      gl.bindTexture(gl.TEXTURE_2D, t);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      return t;
    };
    this.tex = { val: mk(), pot: mk(), mat: mk(), cov: mk(), lut: mk() };
    gl.bindTexture(gl.TEXTURE_2D, this.tex.lut);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  }

  setGeometry(nx: number, ny: number, material: Uint8Array, coverage: Float32Array) {
    const gl = this.gl;
    this.nx = nx;
    this.ny = ny;
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    gl.bindTexture(gl.TEXTURE_2D, this.tex.mat);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R8, nx, ny, 0, gl.RED, gl.UNSIGNED_BYTE, material);
    const cov8 = new Uint8Array(coverage.length);
    for (let i = 0; i < coverage.length; i++) cov8[i] = Math.round(Math.min(1, Math.max(0, coverage[i])) * 255);
    gl.bindTexture(gl.TEXTURE_2D, this.tex.cov);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R8, nx, ny, 0, gl.RED, gl.UNSIGNED_BYTE, cov8);
    this.hasData = true;
  }

  setValues(values: Float32Array, potential: Float32Array) {
    const gl = this.gl;
    const clean = (a: Float32Array) => {
      const o = new Float32Array(a.length);
      for (let i = 0; i < a.length; i++) o[i] = Number.isNaN(a[i]) ? 0 : a[i];
      return o;
    };
    values = clean(values);
    potential = clean(potential);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
    gl.bindTexture(gl.TEXTURE_2D, this.tex.val);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R32F, this.nx, this.ny, 0, gl.RED, gl.FLOAT, values);
    gl.bindTexture(gl.TEXTURE_2D, this.tex.pot);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R32F, this.nx, this.ny, 0, gl.RED, gl.FLOAT, potential);
  }

  setColormap(name: ColormapName) {
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, this.tex.lut);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, 256, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, lut(name));
  }

  clear(w: number, h: number) {
    const gl = this.gl;
    gl.viewport(0, 0, w, h);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
  }

  draw(devW: number, devH: number, u: FieldUniforms) {
    const gl = this.gl;
    this.clear(devW, devH);
    if (!this.hasData) return;
    gl.useProgram(this.prog);
    gl.bindVertexArray(this.vao);
    const L = this.loc;
    gl.uniform2f(L.uOff, u.offX, u.offY);
    gl.uniform2f(L.uSize, u.sizeX, u.sizeY);
    gl.uniform1f(L.uScale, u.scale);
    gl.uniform2f(L.uViewport, u.viewW, u.viewH);
    gl.uniform1f(L.uMin, u.min);
    gl.uniform1f(L.uMax, u.max);
    gl.uniform1i(L.uLog, u.log ? 1 : 0);
    gl.uniform1i(L.uIso, u.iso ? 1 : 0);
    gl.uniform1f(L.uIsoStep, u.isoStep);
    gl.uniform1i(L.uMode, u.geometryOnly ? 1 : 0);
    gl.uniform2f(L.uTexSize, this.nx, this.ny);
    gl.uniform3fv(L.uTerm, TERMINAL_COLORS.flatMap(hexToRgb));
    gl.uniform3fv(L.uCond, hexToRgb(u.condColor));
    const units: [string, WebGLTexture][] = [
      ["uVal", this.tex.val],
      ["uPot", this.tex.pot],
      ["uMat", this.tex.mat],
      ["uCov", this.tex.cov],
      ["uLut", this.tex.lut],
    ];
    units.forEach(([name, tex], i) => {
      gl.activeTexture(gl.TEXTURE0 + i);
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.uniform1i(L[name], i);
    });
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }
}
