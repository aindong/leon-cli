/* Check that icon/leon-1024.png is icon/leon-original-280.png scaled up, on the macOS icon grid:
 * the master's tile is the Lanczos-3 enlargement of the original, pixel for pixel, with every
 * original pixel unchanged at the centre of its 3 × 3 block; the canvas is clear outside the tile;
 * no parchment from the thumbnail's corners is left at the tile's edge; and the icon slots and the
 * README banner show that master. Prints the numbers and exits 1 on a failure. build-assets.cjs
 * uses the same geometry and filter, and runs the check.
 * NODE_PATH=/tmp/leon-brand-build/node_modules node brand/check-icon.cjs
 */
const path = require('node:path');
const sharp = require('sharp');
// The thumbnail is enlarged SCALE times and its top-left pixel block lands at (LEFT, TOP). TILE is
// the thumbnail's own rounded tile, pulled in about 1.3 thumbnail pixels so its anti-aliased edge
// (a blend with the parchment around it) is cut off. It is centred on the 1024 canvas, where Apple's
// grid has an 824 × 824 tile; the thumbnail's tile is slightly taller than wide and keeps that shape.
// BANNER is where the README banner (docs/images/banner.png) draws the tile, in its own pixels.
const geometry = {
  SCALE: 3, LEFT: 92, TOP: 94, TILE: { x: 106, y: 99, width: 812, height: 826, radius: 126 },
  BANNER: { left: 65, top: 89, width: 218, height: 222 },
};
// How much of pixel (x, y) the tile, pulled in by `inset` pixels, covers: 1 inside, 0 outside, and
// in between along the rounded corners.
function coverage(x, y, inset = 0) {
  const { TILE: t } = geometry, r = t.radius - inset;
  const [x0, y0, x1, y1] = [t.x + inset, t.y + inset, t.x + t.width - inset, t.y + t.height - inset];
  if (x < x0 || x >= x1 || y < y0 || y >= y1) return 0;
  const cx = Math.min(Math.max(x + 0.5, x0 + r), x1 - r), cy = Math.min(Math.max(y + 0.5, y0 + r), y1 - r);
  return Math.min(1, Math.max(0, r + 0.5 - Math.hypot(x + 0.5 - cx, y + 0.5 - cy)));
}
const inTile = (x, y, inset) => coverage(x, y, inset) === 1;
const sinc = x => x === 0 ? 1 : Math.sin(Math.PI * x) / (Math.PI * x);
const lanczos3 = x => Math.abs(x) < 3 ? sinc(x) * sinc(x / 3) : 0;
// Enlarge a w × h RGB buffer `scale` times with a separable Lanczos-3 filter, edge pixels repeated.
// At a whole odd scale the centre of each block is the source pixel itself (the kernel is 1 at 0
// and 0 at every other whole number), which sharp's own enlargement does not keep.
function enlarge(src, w, h, scale) {
  const pass = (buf, w, h, across) => {
    const n = across ? w : h, out = new Float64Array(w * h * scale * 3);
    for (let j = 0; j < n * scale; j++) {
      const u = (j + 0.5) / scale - 0.5, taps = [];
      for (let i = Math.floor(u) - 2; i <= Math.floor(u) + 3; i++) taps.push([Math.min(n - 1, Math.max(0, i)), lanczos3(u - i)]);
      const total = taps.reduce((sum, [, weight]) => sum + weight, 0);
      for (let o = 0; o < (across ? h : w); o++) for (let c = 0; c < 3; c++) {
        let v = 0;
        for (const [i, weight] of taps) v += weight * buf[((across ? o * w + i : i * w + o)) * 3 + c];
        out[(across ? o * w * scale + j : j * w + o) * 3 + c] = v / total;
      }
    }
    return out;
  };
  return Uint8ClampedArray.from(pass(pass(src, w, h, true), w * scale, h, false), Math.round);
}
const luminance = (buf, i) => 0.299 * buf[i] + 0.587 * buf[i + 1] + 0.114 * buf[i + 2];
// Mean and largest channel difference between two RGBA buffers, and the PSNR in dB.
function compare(a, b) {
  let [sum, squares, max] = [0, 0, 0];
  for (let i = 0; i < a.length; i++) { const d = Math.abs(a[i] - b[i]); sum += d; squares += d * d; max = Math.max(max, d); }
  return { mean: sum / a.length, max, psnr: squares ? 10 * Math.log10(255 * 255 * a.length / squares) : Infinity };
}
// An image's pixels over mid grey, so what is under clear pixels does not count.
const onGrey = image => image.flatten({ background: '#808080' }).raw().toBuffer();
// `root` is the brand folder; the banner is beside it in docs/images.
async function check(root = __dirname) {
  const { SCALE, LEFT, TOP, TILE, BANNER } = geometry, failures = [];
  const expect = (ok, text) => { console.log(`${ok ? 'ok  ' : 'FAIL'} ${text}`); if (!ok) failures.push(text); };
  const icon = name => path.join(root, 'icon', name);
  // a ramp of 30 per pixel, 9 wide: a filter that copies pixels or reads them mirrored is 10 or 20 off
  const ramp = enlarge(Uint8Array.from({ length: 9 * 3 }, (_, i) => Math.floor(i / 3) * 30), 9, 1, SCALE);
  const off = Math.max(...Array.from({ length: 9 }, (_, j) => Math.abs(ramp[(9 + j) * 3] - ((9 + j + 0.5) / SCALE - 0.5) * 30)));
  expect(off <= 2, `the filter follows a ramp of 30 per pixel: ${off.toFixed(2)} off at most`);
  // one pixel of 200 in a row of 100: these are Lanczos-3's own values, worked out apart from this file.
  // Its second lobe dips to 85 and its third rises to 103; bilinear gives 100 there, Catmull-Rom 93 and 100.
  const known = [85, 100, 138, 181, 200, 181, 138, 100, 85, 91, 100, 103];
  const pulse = enlarge(Uint8Array.from({ length: 9 * 3 }, (_, i) => Math.floor(i / 3) === 4 ? 200 : 100), 9, 1, SCALE);
  const got = known.map((_, j) => pulse[(9 + j) * 3]);
  expect(got.every((v, j) => v === known[j]), `the filter is Lanczos-3: one pixel of 200 among 100s becomes ${got.join(' ')}`);
  const src = await sharp(icon('leon-original-280.png')).removeAlpha().raw().toBuffer({ resolveWithObject: true });
  const meta = await sharp(icon('leon-1024.png')).metadata();
  expect(src.info.width === 280 && src.info.height === 280, `the original is 280 × 280: ${src.info.width} × ${src.info.height}`);
  expect(meta.width === 1024 && meta.height === 1024 && meta.hasAlpha, `the master is 1024 × 1024 with alpha: ${meta.width} × ${meta.height}`);
  const master = sharp(icon('leon-1024.png')), m = await master.clone().ensureAlpha().raw().toBuffer();
  const big = enlarge(src.data, 280, 280, SCALE), side = 280 * SCALE;
  let [kept, moved, same, differ, outside, holes, brightest] = [0, 0, 0, 0, 0, 0, 0];
  for (let y = 0; y < 1024; y++) for (let x = 0; x < 1024; x++) {
    const i = (y * 1024 + x) * 4, alpha = m[i + 3];
    if (alpha && !coverage(x, y)) outside++;
    if (alpha < 255 && inTile(x, y)) holes++;
    if (alpha && !inTile(x, y, 8)) brightest = Math.max(brightest, Math.round(luminance(m, i)));
    if (!coverage(x, y)) continue;
    const [bx, by] = [x - LEFT, y - TOP], pixel = [0, 1, 2].map(c => m[i + c]);
    if (pixel.every((v, c) => v === big[(by * side + bx) * 3 + c])) same++; else differ++;
    if (bx % SCALE !== 1 || by % SCALE !== 1) continue;
    const at = (Math.floor(by / SCALE) * 280 + Math.floor(bx / SCALE)) * 3;
    if (pixel.every((v, c) => v === src.data[at + c])) kept++; else moved++;
  }
  expect(kept > 70000 && moved === 0, `original pixels unchanged at their block centres: ${kept} of ${kept + moved}`);
  expect(differ === 0, `the tile is the Lanczos-3 enlargement, pixel for pixel: ${same} of ${same + differ}`);
  const back = await sharp(icon('leon-1024.png')).removeAlpha().extract({ left: LEFT, top: TOP, width: side, height: side }).resize(280, 280, { kernel: 'lanczos3' }).raw().toBuffer();
  const pick = buf => Array.from({ length: 280 * 280 }, (_, p) => p).filter(p => inTile(LEFT + SCALE * (p % 280) + 1, TOP + SCALE * Math.floor(p / 280) + 1, 6)).flatMap(p => [buf[p * 3], buf[p * 3 + 1], buf[p * 3 + 2]]);
  const round = compare(pick(back), pick(src.data));
  expect(round.mean <= 2, `reduced back to 280 px against the original: mean difference ${round.mean.toFixed(3)} per channel, largest ${round.max}`);
  expect(outside === 0 && holes === 0, `clear outside the ${TILE.width} × ${TILE.height} tile at (${TILE.x}, ${TILE.y}), solid inside: ${outside} stray, ${holes} holes`);
  expect(Math.abs(TILE.x * 2 + TILE.width - 1024) <= 1 && Math.abs(TILE.y * 2 + TILE.height - 1024) <= 1 && Math.abs(Math.max(TILE.width, TILE.height) - 824) <= 4,
    'the tile is centred and about 824 px on its longer side');
  expect(brightest <= 110, `no light fringe: the brightest pixel in the tile's outer 8 px has luminance ${brightest} (the tile is about 55, the parchment 243)`);
  // a seam is an edge strip lighter or darker than the tile just inside it, along the straight part of a side
  const strip = (from, to, edge) => {
    let [sum, n] = [0, 0];
    for (let d = from; d < to; d++) for (let s = 2 * TILE.radius; s < (edge < 2 ? TILE.height : TILE.width) - 2 * TILE.radius; s++) {
      const [x, y] = [[TILE.x + d, TILE.y + s], [TILE.x + TILE.width - 1 - d, TILE.y + s], [TILE.x + s, TILE.y + d], [TILE.x + s, TILE.y + TILE.height - 1 - d]][edge];
      sum += luminance(m, (y * 1024 + x) * 4); n++;
    }
    return sum / n;
  };
  const seams = ['left', 'right', 'top', 'bottom'].map((name, edge) => [name, strip(0, 3, edge) - strip(6, 9, edge)]);
  expect(seams.every(([, gap]) => Math.abs(gap) <= 10), `no seam: the outer 3 px against 6 to 9 px in, mean luminance: ${seams.map(([name, gap]) => `${name} ${gap.toFixed(1)}`).join(', ')}`);
  // all ten slots are there, each at its own size and showing its source: the optical icon at 16 px, the master elsewhere
  for (const size of [16, 32, 128, 256, 512]) for (const factor of [1, 2]) {
    const file = `icon_${size}x${size}${factor === 2 ? '@2x' : ''}.png`, px = size * factor;
    const slot = sharp(icon(`Leon.iconset/${file}`)), { width, height } = await slot.metadata().catch(() => ({}));
    if (width !== px || height !== px) { expect(false, `${file} is ${px} × ${px}: ${width ? `${width} × ${height}` : 'missing or unreadable'}`); continue; }
    const source = px === 16 ? sharp(icon('leon-small.svg'), { density: 288 }) : master.clone();
    const { psnr, mean } = compare(await onGrey(slot), await onGrey(source.resize(px, px)));
    expect(psnr >= 40, `${file} is the ${px === 16 ? 'optical icon' : 'master'} at ${px} px: ${psnr.toFixed(1)} dB, mean difference ${mean.toFixed(2)}`);
  }
  const tile = master.clone().extract({ left: TILE.x, top: TILE.y, width: TILE.width, height: TILE.height }).resize(BANNER.width, BANNER.height);
  const banner = sharp(path.join(root, '../docs/images/banner.png')).extract(BANNER);
  // away from the tile's rounded corners, where the banner's ground shows
  const inner = async image => onGrey(sharp(await image.png().toBuffer()).extract({ left: 12, top: 12, width: BANNER.width - 24, height: BANNER.height - 24 }));
  const shown = compare(await inner(banner), await inner(tile));
  expect(shown.mean <= 3, `the README banner draws the master's tile: mean difference ${shown.mean.toFixed(2)} per channel`);
  return failures;
}
module.exports = { geometry, coverage, enlarge, check };
if (require.main === module) check().then(f => { process.exitCode = f.length ? 1 : 0; }, e => { console.error(`FAIL ${e.message}`); process.exitCode = 1; });
