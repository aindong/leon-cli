# leon identity

leon's mark is a lion with a chunky square silhouette, rounded ear and mane bumps, relaxed lids, a broad nose and a knowing half-smile, shaded gold to coral to Oxide on a deep maroon tile.

**Tagline:** Keep your work in view.

![leon hero](marketing/hero.png)

## Assets and rules

| Asset | Light surfaces | Dark surfaces |
| --- | --- | --- |
| Flat colour lockup | [logo.svg](logo.svg) | [logo-dark.svg](logo-dark.svg) |
| Flat colour lion | [logo-mark.svg](logo-mark.svg) | [logo-mark-dark.svg](logo-mark-dark.svg) |
| Single-colour lion | [logo-mark-mono.svg](logo-mark-mono.svg) | [logo-mark-mono-dark.svg](logo-mark-mono-dark.svg) |
| Existing wordmark | [wordmark.svg](wordmark.svg) | [wordmark-dark.svg](wordmark-dark.svg) |

The flat lion is a manual SVG contour transcription of the cleaned original: one square mane silhouette with rounded top bumps, relaxed lids, a broad nose and a one-sided smile. Light marks use Oxide with Parchment details; dark marks use Amber with Maroon details. Monochrome marks cut the features out of one solid colour. The flat logo is an intentional simplification of the shaded character, not a miniature raster embedded in a logo.

Keep at least 10 design units of space around the lion. Use flat marks at 24 px or above and lockups at 108 px or above; use the optical app icon for 16 px. Preserve the sideways smile, low lids, original rounded ear bumps, cheek tufts, broad base and single colour field. Do not add a swept top curl, pointed chin, fangs, angry brows, shields, panels or separated colour chips. Do not stretch, rotate or give individual mane parts their own gloss. Keep depth on the app icon; keep logos flat.

The custom lowercase wordmark, 360 × 90 lockup frame, tagline and product copy are retained. Use **leon** in copy, **Leon.app** for the bundle and `leon` for commands.

## Palette

Oxide stays the brand anchor. Maroon replaces Iron as the app tile colour: its warm dark field separates the golden face and gives the character its own setting. Iron and Parchment remain the primary text/surface pair.

| Token | Hex | Role |
| --- | --- | --- |
| Oxide | `#B64C32` | Brand anchor, flat light mark, light-mode links; lower gradient shading |
| Maroon | `#542D35` | App tile base; details in the Amber flat mark |
| Deep maroon | `#352028` | Tile shadows; small-icon facial details |
| Gold | `#FFD477` | Soft upper-left light |
| Amber | `#F3A747` | Warm upper face and flat dark-surface mark |
| Coral | `#EB795D` | Warm midtone transition |
| Iron | `#292521` | Main light-mode text; dark-mode page and terminal surfaces |
| Parchment | `#F5F0E7` | Light page; dark-mode main text |
| Umber / Chalk | `#65594F` / `#D8CEC2` | Secondary light / dark text |
| Clay | `#E6AA91` | Dark-mode links |
| Sand / Coal | `#EBE3D7` / `#1F1C19` | Raised light surface / terminal pane |

The icon's continuous light field moves **Gold → Amber → Coral → Oxide**, with maroon in the deepest shadows. The flat optical source uses stops at 0%, 38%, 70%, 100%. Those are art-direction targets; the approved raster includes continuous lighting and its sampled pixels are not restricted to four hex values. The generated source uses palette optimisation to keep file size small.

Status accents are quieter than the retired pane colours: **Copper `#E78155`** for `● working`, **Sage `#B8C6AC`** for `✓ done`, **Wheat `#E8C77D`** for `! blocked`, and **Rose `#E28E9B`** for a real error label. Working retains its warm accent; done and blocked return to softer supporting tones. Status always includes a word and distinct symbol. The demo does not invent an error state. Iris and Lagoon are retired from production identity use.

Approved text contrast: Iron/Parchment **13.40:1**, Umber/Parchment **5.98:1**, Oxide/Parchment **4.55:1**, Chalk/Iron **9.80:1**, Clay/Iron **7.62:1**. Copper, Sage, Wheat and Rose on Iron are **5.55, 8.48, 9.34 and 6.21:1** respectively. Amber/Maroon is **5.78:1**. Gold, Amber and Coral are decorative on light surfaces, not light-mode text colours.

## Icon sources and small sizes

The canonical [1024 PNG master](icon/leon-1024.png) is the approved imagegen render with alpha outside the macOS rounded tile. The [1024 SVG](icon/leon-1024.svg) is a standalone **raster wrapper**, explicitly embedding that PNG; it is not editable vector artwork. This raster master is permitted by the production brief.

The [optical SVG](icon/leon-small.svg) supplies the 16 pt slots at 1× and 2× and the page favicon. It replaces volumetric folds with one rounded mane shape and a simple warm gradient, removes the tile rim/shadow, and enlarges the lids, nose and half-smile. The 32 pt and larger slots use the raster master. Fine facial expression softens at 16 px, so the mane silhouette and broad dark details do the work.

![Native 16, 32, and 128 px icons on light and dark backgrounds](icon/size-check.png)

The proof shows 16/32/128 px on Parchment and Iron, plus direct reduction versus the optical 16 px source enlarged 8×. [Leon.iconset](icon/Leon.iconset) contains all ten standard slots; [Leon.icns](icon/Leon.icns) uses Apple's native encoder. The existing bundle script already copies it.

[background.svg](icon/background.svg) and [foreground.svg](icon/foreground.svg) are simplified, unmasked vector references for possible future layered work. They are not the exact layers of the rendered icon. The delivered ICNS is static; it does not claim native adaptive Liquid Glass behaviour or an Icon Composer package.

## Marketing, type and rebuild

- [Hero](marketing/hero.png) / [SVG](marketing/hero.svg): **1600 × 600**.
- [OG](marketing/og.png) / [SVG](marketing/og.svg): **1200 × 630**; the SVG embeds the approved app icon.
- [Landing page](marketing/index.html): single HTML with inline CSS, embedded font/icon/favicon, phone layout and dark mode.
- [Copy](marketing/copy.md): unchanged tagline, alternatives, 50-word pitch and 150-word description.

Instrument Sans remains the UI/marketing family, with the local Latin WOFF2 and SIL [OFL licence](fonts/OFL.txt). Menlo remains the terminal/code family with a monospace web fallback. The custom wordmark remains vector paths. No app typography or application code changes.

The page and hero retain the illustrated workflow and labelled statuses; the OG shows the current icon. A continuous warm rule replaces the old pane-colour strip. Checkout-relative documentation links and OG metadata remain deployment-time concerns.

```sh
npm install --prefix /tmp/leon-brand-build sharp playwright
NODE_PATH=/tmp/leon-brand-build/node_modules node brand/build-assets.cjs
```

On macOS with Google Chrome and `iconutil`, the builder reads the committed raster master and optical SVG, then regenerates the wrapper, ten icon slots, ICNS, size proof, page embed, OG embed and both banner PNGs. It never regenerates or overwrites the canonical raster and never calls imagegen. Flat logo SVGs and banner layouts are editable sources.
