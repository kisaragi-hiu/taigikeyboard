// Synthesizes the soundtrack from events.json (written by render.mjs) → audio.wav
import fs from 'fs';
const dir = new URL('.', import.meta.url).pathname;
const EV = JSON.parse(fs.readFileSync(dir + 'events.json', 'utf8'));
const SR = 48000, DUR = 44.5, N = Math.ceil(SR * DUR);
const dryL = new Float32Array(N), dryR = new Float32Array(N);
const wetIn = new Float32Array(N); // mono send to reverb
let seed = 12345; const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
const TAU = Math.PI * 2;
const clamp = (x, a, b) => Math.min(b, Math.max(a, x));
const ss = (a, b, x) => { const t = clamp((x - a) / (b - a), 0, 1); return t * t * (3 - 2 * t); };

function add(i, l, r, send) { if (i < 0 || i >= N) return; dryL[i] += l; dryR[i] += r; wetIn[i] += (l + r) * .5 * send; }

// ---- pad: slow chords, pure sines with gentle detune + breathing ----
const hz = m => 440 * Math.pow(2, (m - 69) / 12);
const CH = [
  { t: 0, notes: [50, 57, 64, 66, 73] },        // Dmaj9
  { t: 9.2, notes: [47, 54, 57, 62, 64] },      // Bm11
  { t: 19.8, notes: [43, 50, 54, 57, 59, 66] }, // Gmaj9
  { t: 23.3, notes: [50, 57, 62, 64, 66, 69] }, // D add9 (iPhone)
  { t: 32.5, notes: [45, 52, 57, 59, 61, 64] }, // A add9 (Mac)
  { t: 40.2, notes: [38, 50, 57, 62, 64, 66, 69] }, // D (home)
];
const padEnv = t => ss(0, 3.2, t) * (1 - ss(DUR - 2.6, DUR - .05, t)) * (0.78 + .22 * ss(40.2, 41.8, t));
const XF = 1.8;
{
  const phases = new Map();
  for (let i = 0; i < N; i++) {
    const t = i / SR; const e = padEnv(t); if (e <= 0) continue;
    let l = 0, r = 0;
    CH.forEach((c, ci) => {
      const next = CH[ci + 1];
      const gin = ss(c.t - XF / 2, c.t + XF / 2, t) || (ci === 0 ? 1 : 0);
      const gout = next ? 1 - ss(next.t - XF / 2, next.t + XF / 2, t) : 1;
      const g = (ci === 0 ? 1 : gin) * gout; if (g <= 0) return;
      c.notes.forEach((m, k) => {
        for (const det of [-1, 1]) {
          const f = hz(m) * (1 + det * 0.0016);
          const key = ci * 100 + k * 2 + (det > 0 ? 1 : 0);
          const ph = (phases.get(key) || 0) + TAU * f / SR; phases.set(key, ph);
          const breathe = .75 + .25 * Math.sin(TAU * (0.07 + k * .013) * t + k);
          const lowTilt = m < 48 ? 1.2 : m > 68 ? .45 : .8;
          const v = Math.sin(ph) * .016 * g * breathe * lowTilt;
          const pan = .5 + det * .18 * (k % 2 ? 1 : -1);
          l += v * (1 - pan) * 1.4; r += v * pan * 1.4;
        }
      });
    });
    add(i, l * e, r * e, .35);
  }
}

// ---- voices ----
function bell(t0, f, amp, pan = .5, send = .55, decay = 1.6) {
  const parts = [[1, 1, decay], [2.0, .28, decay * .45], [3.01, .1, decay * .3], [4.16, .05, decay * .18]];
  const len = Math.floor(SR * decay * 3.2), i0 = Math.floor(t0 * SR);
  for (let n = 0; n < len; n++) {
    const t = n / SR; const att = Math.min(1, t / .004);
    let v = 0; for (const [m, a, d] of parts) v += Math.sin(TAU * f * m * t) * a * Math.exp(-t / d * 2.2);
    v *= amp * att;
    add(i0 + n, v * (1 - pan) * 1.4, v * pan * 1.4, send);
  }
}
function click(t0, amp, bright = 1, pan = .5, body = 0) {
  const i0 = Math.floor(t0 * SR), len = Math.floor(SR * .06);
  let prev = 0, lp = 0; const f = 2100 + 500 * bright + rnd() * 250;
  for (let n = 0; n < len; n++) {
    const t = n / SR;
    const w = rnd() * 2 - 1; const hp = w - prev; prev = w; // crude high-pass noise
    lp += (hp - lp) * .55;
    const nEnv = Math.exp(-t / .0045);
    const tEnv = Math.exp(-t / .011);
    let v = lp * nEnv * .55 + Math.sin(TAU * f * t) * tEnv * .28;
    if (body) v += Math.sin(TAU * 170 * t) * Math.exp(-t / .03) * body;
    v *= amp * Math.min(1, t / .0006);
    add(i0 + n, v * (1 - pan) * 1.4, v * pan * 1.4, .12);
  }
}
function whoosh(t0, amp, len = 1.1) {
  const i0 = Math.floor((t0 - len * .55) * SR), L = Math.floor(len * SR);
  let a = 0, b = 0;
  for (let n = 0; n < L; n++) {
    const x = n / L; const env = Math.pow(Math.sin(Math.PI * x), 2) * (x < .55 ? x / .55 : 1);
    const cut = .01 + .08 * Math.sin(Math.PI * x);
    const w = rnd() * 2 - 1; a += (w - a) * cut; b += (a - b) * cut;
    const v = b * env * amp * 6;
    const pan = .5 + .25 * Math.sin(TAU * x * .5 - 1);
    add(i0 + n, v * (1 - pan) * 1.4, v * pan * 1.4, .5);
  }
}
function sub(t0, f, amp, len) {
  const i0 = Math.floor(t0 * SR), L = Math.floor(len * SR);
  for (let n = 0; n < L; n++) { const t = n / SR; const e = ss(0, .8, t) * Math.exp(-Math.max(0, t - .8) / (len * .35)); const v = Math.sin(TAU * f * t) * amp * e; add(i0 + n, v, v, .1); }
}

// pentatonic in D (chimes follow the tone colour, gently rising)
const PENTA = [74, 76, 78, 81, 83, 86, 88];
for (const e of EV) {
  const lv = e.level ?? 1;
  switch (e.type) {
    case 'key': e.mac ? click(e.t, .2, e.v * .4 - .6, .42 + e.v * .16, .1) : click(e.t, e.soft ? .16 : .19, e.v, .42 + e.v * .16); break;
    case 'tonekey': click(e.t, .2, e.mac ? -.5 : .3, .5, e.mac ? .16 : .12); break;
    case 'tapkey': click(e.t, .16, .2, .5, .18); break;
    case 'chime': bell(e.t, hz(PENTA[e.note % PENTA.length]), .085 * lv / .55, .35 + (e.note % 3) * .15, .6, 1.5); break;
    case 'reveal':
      whoosh(e.t + .05, .05 * lv, 1.2);
      [74, 78, 81, 86].forEach((m, k) => bell(e.t + .12 + k * .07, hz(m), .05 * lv * (1 - k * .12), .3 + k * .13, .75, 2.4));
      break;
    case 'whoosh': whoosh(e.t, .045 * lv / .35, 1.0); break;
    case 'theme': whoosh(e.t + .3, .025, .8); bell(e.t + .2, hz(81 + (e.t > 34 ? 5 : e.t > 33 ? 2 : 0)), .035, .6, .7, 1.2); break;
    case 'tick': bell(e.t, hz(PENTA[e.note] + 12), .026, .3 + e.note * .1, .7, 1.0); break;
    case 'bloom':
      sub(e.t, hz(38), .09 * lv, 3.2);
      [62, 66, 69, 74, 76].forEach((m, k) => bell(e.t + .25 + k * .11, hz(m), .045 * lv, .25 + k * .12, .8, 3.0));
      break;
  }
}

// ---- reverb (Schroeder/Freeverb-ish), stereo ----
function reverb(input, offs) {
  const combs = [1557, 1617, 1491, 1422, 1277, 1356].map(d => Math.floor((d + offs) * SR / 44100 * 1.35));
  const aps = [556, 441, 341].map(d => Math.floor((d + offs * .3) * SR / 44100));
  const out = new Float32Array(N);
  for (const d of combs) {
    const buf = new Float32Array(d); let idx = 0, store = 0;
    for (let i = 0; i < N; i++) {
      const y = buf[idx]; store = y * .72 + store * .28; buf[idx] = input[i] + store * .86; idx = (idx + 1) % d; out[i] += y / combs.length;
    }
  }
  for (const d of aps) {
    const buf = new Float32Array(d); let idx = 0;
    for (let i = 0; i < N; i++) { const b = buf[idx]; const y = -out[i] + b; buf[idx] = out[i] + b * .5; idx = (idx + 1) % d; out[i] = y; }
  }
  return out;
}
// pre-delay
const pd = Math.floor(.022 * SR); const wet = new Float32Array(N); for (let i = pd; i < N; i++) wet[i] = wetIn[i - pd];
const rl = reverb(wet, 0), rr = reverb(wet, 23);
const L = new Float32Array(N), R = new Float32Array(N);
let peak = 0;
for (let i = 0; i < N; i++) {
  const t = i / SR; const master = 1 - ss(DUR - .5, DUR, t);
  L[i] = (dryL[i] + rl[i] * .9) * master; R[i] = (dryR[i] + rr[i] * .9) * master;
  peak = Math.max(peak, Math.abs(L[i]), Math.abs(R[i]));
}
const g = .89 / peak;
const buf = Buffer.alloc(44 + N * 4);
buf.write('RIFF', 0); buf.writeUInt32LE(36 + N * 4, 4); buf.write('WAVE', 8); buf.write('fmt ', 12);
buf.writeUInt32LE(16, 16); buf.writeUInt16LE(1, 20); buf.writeUInt16LE(2, 22); buf.writeUInt32LE(SR, 24);
buf.writeUInt32LE(SR * 4, 28); buf.writeUInt16LE(4, 32); buf.writeUInt16LE(16, 34); buf.write('data', 36); buf.writeUInt32LE(N * 4, 40);
for (let i = 0; i < N; i++) {
  buf.writeInt16LE(Math.round(clamp(L[i] * g, -1, 1) * 32767), 44 + i * 4);
  buf.writeInt16LE(Math.round(clamp(R[i] * g, -1, 1) * 32767), 46 + i * 4);
}
fs.writeFileSync(dir + 'audio.wav', buf);
console.log('peak', peak.toFixed(3), 'events', EV.length);
