// The app's colour (Settings → General → Colour): one picked colour becomes the accent tokens of
// prose.css, light and dark. Worked out here, not with color-mix() or contrast-color(), because the
// Mac app's oldest WebKit (Safari 15) has neither. Kept free of imports so vitest runs it as is.

type RGB = [number, number, number];
type Lab = [number, number, number];

const LIGHT_BG = '#faf9fd'; // --bg
const DARK_BG = '#141319'; // --bg, dark
const INK = '#1b1a22'; // --text: the dark text on a pale accent

const hex = (h: string): RGB => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) / 255) as RGB;
const toHex = (c: RGB) => '#' + c.map((v) => Math.round(Math.min(1, Math.max(0, v)) * 255).toString(16).padStart(2, '0')).join('');
const lin = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const gam = (v: number) => (v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055);

function oklab(c: RGB): Lab {
  const [r, g, b] = c.map(lin);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
}
function rgb([L, a, b]: Lab): RGB {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s].map(gam) as RGB;
}
/** `a` mixed with `t` of `b`, in OKLab. */
export function mix(a: string, b: string, t: number): string {
  const [x, y] = [oklab(hex(a)), oklab(hex(b))];
  return toHex(rgb(x.map((v, i) => v + (y[i] - v) * t) as Lab));
}
/** WCAG contrast ratio of two colours. */
export function contrast(a: string, b: string): number {
  const Y = (h: string) => { const [r, g, bl] = hex(h).map(lin); return 0.2126 * r + 0.7152 * g + 0.0722 * bl; };
  const [p, q] = [Y(a), Y(b)].sort((m, n) => n - m);
  return (p + 0.05) / (q + 0.05);
}
/** `c` moved towards `to` until it reads as text on `bg` (4.5:1). */
function readable(c: string, bg: string, to: string): string {
  for (let t = 0; t <= 1; t += 0.05) { const m = mix(c, to, t); if (contrast(m, bg) >= 4.5) return m; }
  return to;
}
/** Text on the accent: white, or the dark ink when the accent is pale. */
const onAccent = (c: string) => (contrast(c, '#ffffff') >= contrast(c, INK) ? '#ffffff' : INK);

/** The accent tokens for one picked colour. */
export function accentTokens(picked: string) {
  const light = readable(picked, LIGHT_BG, '#000000');
  const dark = readable(mix(picked, '#ffffff', 0.4), DARK_BG, '#ffffff');
  return {
    light: { '--accent': light, '--accent-ink': mix(light, '#000000', 0.2), '--accent-soft': mix(light, '#ffffff', 0.85), '--on-accent': onAccent(light) },
    dark: { '--accent': dark, '--accent-ink': mix(dark, '#ffffff', 0.3), '--accent-soft': mix(dark, DARK_BG, 0.8), '--on-accent': onAccent(dark) },
  };
}

/** CSS that sets them; `html:root` outranks prose.css whatever the order of the stylesheets. `light`: the saved recipe book. */
export function accentCss(picked: string | null, light = false): string {
  if (!picked || !/^#[0-9a-f]{6}$/i.test(picked)) return '';
  const t = accentTokens(picked.toLowerCase());
  const decl = (o: Record<string, string>) => Object.entries(o).map(([k, v]) => `${k}:${v}`).join(';');
  return `html:root{${decl(t.light)}}` + (light ? '' : `@media screen{html:root[data-theme="dark"]{${decl(t.dark)}}}`);
}

/** Show `picked` (null: the built-in purple) and remember it for the next start (theme.js applies it before first paint). */
export function applyAccent(picked: string | null) {
  const css = accentCss(picked);
  let el = document.getElementById('accent');
  if (!el) { el = document.createElement('style'); el.id = 'accent'; document.head.appendChild(el); }
  el.textContent = css;
  try { if (css) localStorage.setItem('accent-css', css); else localStorage.removeItem('accent-css'); } catch {}
}

/** The colours offered as swatches; the first is the built-in purple. */
export const SWATCHES = [
  { name: 'Purple', value: '#7a52cc' },
  { name: 'Blue', value: '#2f6fd0' },
  { name: 'Teal', value: '#14837b' },
  { name: 'Green', value: '#3d8a3a' },
  { name: 'Orange', value: '#c25a1c' },
  { name: 'Red', value: '#c23a4b' },
  { name: 'Pink', value: '#c2408f' },
];
