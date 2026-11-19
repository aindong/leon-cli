/* Rebuild PNG exports, the complete iconset, ICNS, and optical-size proof.
 * Needs Node, sharp, playwright, Google Chrome, and macOS iconutil.
 * Example: npm install --prefix /tmp/leon-brand-build sharp playwright
 * NODE_PATH=/tmp/leon-brand-build/node_modules node brand/build-assets.cjs
 * The art is icon/leon-original-280.png, the original thumbnail 11. This script makes the master,
 * icon/leon-1024.png, from it: enlarged 3× with a Lanczos-3 filter, its own rounded tile kept and
 * everything outside made clear (the numbers are in check-icon.cjs, which then compares the pixels).
 * No image generator and no AI upscaler touches it.
 * Flat marks and the optical 16 px SVG are hand-authored, independent vector assets.
 */
const fs = require('node:fs/promises');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { execFileSync } = require('node:child_process');
const sharp = require('sharp');
const { chromium } = require('playwright');
const { geometry, coverage, enlarge, check } = require('./check-icon.cjs');
const root = __dirname;
const png = { palette: true, colors: 256, effort: 10 };
// The master: the original enlarged, drawn where the tile covers the canvas, RGBA so its grain is not dithered.
async function buildMaster() {
  const { SCALE, LEFT, TOP } = geometry, side = 280 * SCALE, out = Buffer.alloc(1024 * 1024 * 4);
  const big = enlarge(await sharp(path.join(root, 'icon/leon-original-280.png')).removeAlpha().raw().toBuffer(), 280, 280, SCALE);
  for (let y = 0; y < 1024; y++) for (let x = 0; x < 1024; x++) {
    const alpha = Math.round(coverage(x, y) * 255), from = ((y - TOP) * side + x - LEFT) * 3;
    if (alpha) out.set([big[from], big[from + 1], big[from + 2], alpha], (y * 1024 + x) * 4);
  }
  await sharp(out, { raw: { width: 1024, height: 1024, channels: 4 } }).png({ compressionLevel: 9 }).toFile(path.join(root, 'icon/leon-1024.png'));
}
async function main() {
  await buildMaster();
  // A standalone SVG wrapper is provided for consumers requiring SVG format.
  // It embeds the canonical raster; it is not editable vector artwork.
  const master = await fs.readFile(path.join(root, 'icon/leon-1024.png'));
  // The tile fills 80.5% of the master (Apple's grid); the page and OG layouts were drawn for a
  // tile at 87.5%, so they take this crop and the icon keeps its size in them.
  const framed = () => sharp(master).extract({ left: 40, top: 40, width: 944, height: 944 });
  await fs.writeFile(path.join(root, 'icon/leon-1024.svg'), `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024" role="img" aria-label="leon Pocket guardian app icon"><title>leon Pocket guardian — raster master wrapper</title><image width="1024" height="1024" href="data:image/png;base64,${master.toString('base64')}"/></svg>\n`);
  const ogPath = path.join(root, 'marketing/og.svg');
  const ogIcon = await framed().resize(672, 672).png(png).toBuffer();
  const ogSource = await fs.readFile(ogPath, 'utf8');
  await fs.writeFile(ogPath, ogSource.replace(/<image id="app-icon"[^>]*\/>/, `<image id="app-icon" x="808" y="125" width="336" height="336" href="data:image/png;base64,${ogIcon.toString('base64')}"/>`));
  // the landing page shows the icon at 112 px; this is twice that, for a sharp screen
  await framed().resize(256, 256).webp({ quality: 90 }).toFile(path.join(root, 'marketing/app-icon.webp'));
  // shortcut: the README banner's layout exists only as its PNG, so the icon is redrawn in place; give it a source file before changing the layout
  const { BANNER, TILE } = geometry, bannerPath = path.join(root, '../docs/images/banner.png');
  const tile = await sharp(master).extract({ left: TILE.x, top: TILE.y, width: TILE.width, height: TILE.height }).resize(BANNER.width, BANNER.height).raw().toBuffer();
  const banner = await sharp(bannerPath).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  // written pixel by pixel: sharp's composite would round the banner's own soft corners again
  const pad = 5; // the old icon's tile was that much larger
  for (let y = BANNER.top - pad; y < BANNER.top + BANNER.height + pad; y++) for (let x = BANNER.left - pad; x < BANNER.left + BANNER.width + pad; x++) {
    const [tx, ty] = [x - BANNER.left, y - BANNER.top], inTile = tx >= 0 && ty >= 0 && tx < BANNER.width && ty < BANNER.height;
    const at = (ty * BANNER.width + tx) * 4, alpha = inTile ? tile[at + 3] / 255 : 0;
    const pixel = [0xF5, 0xF0, 0xE7].map((ground, c) => Math.round(inTile ? tile[at + c] * alpha + ground * (1 - alpha) : ground));
    banner.data.set([...pixel, 255], (y * banner.info.width + x) * 4);
  }
  await sharp(banner.data, { raw: banner.info }).png().toFile(bannerPath);
  const iconset = path.join(root, 'icon/Leon.iconset');
  await fs.mkdir(iconset, { recursive: true });
  for (const size of [16, 32, 128, 256, 512]) {
    for (const factor of [1, 2]) {
      // the face does not survive a reduction to 16 px, so that one file is the optical icon
      const source = size * factor === 16 ? 'leon-small.svg' : 'leon-1024.png';
      const output = `icon_${size}x${size}${factor === 2 ? '@2x' : ''}.png`;
      const input = path.join(root, 'icon', source);
      await sharp(input, source.endsWith('.svg') ? { density: 288 } : {})
        .resize(size * factor, size * factor).png(png).toFile(path.join(iconset, output));
    }
  }
  execFileSync('iconutil', ['-c', 'icns', '-o', path.join(root, 'icon/Leon.icns'), iconset]);
  const browser = await chromium.launch({ channel: 'chrome', headless: true });
  try {
    const page = await browser.newPage({ deviceScaleFactor: 1 });
    for (const [name, width, height] of [['hero', 1600, 600], ['og', 1200, 630]]) {
      await page.setViewportSize({ width, height });
      await page.goto(pathToFileURL(path.join(root, `marketing/${name}.svg`)).href);
      await page.evaluate(() => document.fonts.ready);
      const image = await page.screenshot({ type: 'png' });
      await sharp(image).png(png).toFile(path.join(root, `marketing/${name}.png`));
    }
    const icons = {};
    for (const size of [16, 32, 128]) {
      const bytes = await fs.readFile(path.join(iconset, `icon_${size}x${size}.png`));
      icons[size] = `data:image/png;base64,${bytes.toString('base64')}`;
    }
    const before = await sharp(path.join(root, 'icon/leon-1024.png')).resize(16).png().toBuffer();
    // Render the proof deterministically with Sharp, avoiding browser capture artefacts.
    const zoomBefore = await sharp(before).resize(128, 128, { kernel: 'nearest' }).png().toBuffer();
    const zoomAfter = await sharp(path.join(iconset, 'icon_16x16.png')).resize(128, 128, { kernel: 'nearest' }).png().toBuffer();
    const row = (y, dark) => `<rect x="28" y="${y}" width="904" height="170" rx="8" fill="${dark ? '#292521' : '#F5F0E7'}"/><g fill="${dark ? '#F5F0E7' : '#292521'}">${[16,32,128].map((size, i) => `<text x="${48+i*170}" y="${y+90}">${size}px</text><image x="${183+i*150}" y="${y+(170-size)/2}" width="${size}" height="${size}" href="${icons[size]}"/>`).join('')}<text x="660" y="${y+78}" font-size="13">${dark ? 'Dark' : 'Light'} background</text><text x="660" y="${y+100}" font-size="13">Native pixel sizes</text></g>`;
    const proof = `<svg xmlns="http://www.w3.org/2000/svg" width="960" height="600"><rect width="960" height="600" fill="#F5F0E7"/><g font-family="Arial,sans-serif" font-size="14" fill="#292521"><text x="28" y="42" font-size="24">leon · optical size check</text>${row(65,false)}${row(250,true)}<text x="174" y="492">Direct reduction</text><text x="174" y="514">16px enlarged 8×</text><text x="478" y="492">Optical source</text><text x="478" y="514">16px enlarged 8×</text><text x="660" y="480">One maroon field.</text><text x="660" y="503">One warm mane silhouette.</text><text x="660" y="526">Stronger lids, broad nose</text><text x="660" y="549">and half-smile.</text></g></svg>`;
    await sharp(Buffer.from(proof)).composite([{input:zoomBefore,left:28,top:448},{input:zoomAfter,left:332,top:448}]).png(png).toFile(path.join(root, 'icon/size-check.png'));

  } finally {
    await browser.close();
  }
  console.log('Built the master, hero.png, og.png, the README banner, 10 iconset PNGs, Leon.icns, size-check.png, the landing page icon, and raster SVG wrapper.');
  if ((await check()).length) throw new Error('the built master does not pass check-icon.cjs');
}
main().catch(error => { console.error(error); process.exitCode = 1; });
