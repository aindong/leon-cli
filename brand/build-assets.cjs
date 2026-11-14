/* Rebuild PNG exports, the complete iconset, ICNS, and optical-size proof.
 * Needs Node, sharp, playwright, Google Chrome, and macOS iconutil.
 * Example: npm install --prefix /tmp/leon-brand-build sharp playwright
 * NODE_PATH=/tmp/leon-brand-build/node_modules node brand/build-assets.cjs
 * The generated 1024px raster master is an input, never regenerated here.
 */
const fs = require('node:fs/promises');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { execFileSync } = require('node:child_process');
const sharp = require('sharp');
const { chromium } = require('playwright');
const root = __dirname;
const png = { palette: true, colours: 256, effort: 10 };
async function main() {
  const iconset = path.join(root, 'icon/Leon.iconset');
  await fs.mkdir(iconset, { recursive: true });
  for (const size of [16, 32, 128, 256, 512]) {
    for (const factor of [1, 2]) {
      const source = size <= 32 ? 'leon-small.svg' : 'leon-1024.png';
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
    await page.goto('about:blank');
    await page.setViewportSize({ width: 960, height: 600 });
    await page.setContent(`<!doctype html><style>*{box-sizing:border-box}body{margin:0;font:14px Menlo,monospace;background:#F5F0E7;color:#292521}h1{font:24px system-ui;margin:0 0 20px}main{padding:28px}.row{display:flex;gap:20px;padding:20px;border-radius:8px;align-items:center;margin-bottom:14px}.dark{background:#292521;color:#F5F0E7}.cell{width:150px;display:flex;align-items:center;justify-content:space-between;gap:20px}.cell img{flex-shrink:0}.caption{font-size:12px;max-width:250px;margin-left:20px;line-height:1.6}.zoom{image-rendering:pixelated}.zoomrow{display:flex;align-items:center;gap:26px;margin-top:20px}.label{font-size:12px}</style><main><h1>leon · optical size check</h1>${['light', 'dark'].map(theme => `<div class="row ${theme}">${[16, 32, 128].map(s => `<div class="cell"><span>${s}px</span><img width="${s}" height="${s}" src="${icons[s]}" alt="leon at ${s}px"></div>`).join('')}<div class="caption">${theme} background<br>Shown at native pixel size.</div></div>`).join('')}<div class="zoomrow"><img class="zoom" width="128" height="128" src="data:image/png;base64,${before.toString('base64')}" alt="Before: downsampled 16px icon"><div class="label">Before<br>16px enlarged 8×</div><img class="zoom" width="128" height="128" src="${icons[16]}" alt="After: optically adjusted 16px icon"><div class="label">After<br>16px enlarged 8×</div><div class="caption">Less rim and shadow.<br>Larger, flatter symbol.<br>Clear channel between panes.</div></div></main>`);
    await page.evaluate(() => Promise.all([...document.images].map(i => i.decode())));
    await sharp(await page.screenshot()).png(png).toFile(path.join(root, 'icon/size-check.png'));
  } finally {
    await browser.close();
  }
  console.log('Built hero.png, og.png, 10 iconset PNGs, Leon.icns, and size-check.png.');
}
main().catch(error => { console.error(error); process.exitCode = 1; });
