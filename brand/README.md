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

The flat lion is a manual SVG contour transcription of an earlier cleaned render of the same character: one square mane silhouette with rounded top bumps, relaxed lids, a broad nose and a one-sided smile. Its outline is simpler than the app icon's art: the base is level where the original's bottom is rounded with a small dip in the middle, the cheek tufts are smaller and sit higher, and the curl on top is smaller. Light marks use Oxide with Parchment details; dark marks use Amber with Maroon details. Monochrome marks cut the features out of one solid colour. The flat logo is an intentional simplification of the shaded character, not a miniature raster embedded in a logo.

These rules are for the flat marks; the app icon is the original thumbnail, with its own curl and rounded bottom, and is never redrawn. Keep at least 10 design units of space around the lion. Use flat marks at 24 px or above and lockups at 108 px or above; use the optical app icon for 16 px. Preserve the sideways smile, low lids, original rounded ear bumps, cheek tufts, broad base and single colour field. Do not add a swept top curl, pointed chin, fangs, angry brows, shields, panels or separated colour chips. Do not stretch, rotate or give individual mane parts their own gloss. Keep depth on the app icon; keep logos flat.

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

The icon's continuous light field moves **Gold → Amber → Coral → Oxide**, with maroon in the deepest shadows. The flat optical source uses stops at 0%, 38%, 70%, 100%. Those are art-direction targets; the app icon's art has continuous lighting and grain, so its pixels are not restricted to four hex values. The master is a full-colour PNG; the icon slots and web embeds made from it use a 256-colour palette to stay small.

Status accents are quieter than the retired pane colours: **Copper `#E78155`** for `● working`, **Sage `#B8C6AC`** for `✓ done`, **Wheat `#E8C77D`** for `! blocked`, and **Rose `#E28E9B`** for a real error label. Working retains its warm accent; done and blocked return to softer supporting tones. Status always includes a word and distinct symbol. The demo does not invent an error state. Iris and Lagoon are retired from production identity use.

Approved text contrast: Iron/Parchment **13.40:1**, Umber/Parchment **5.98:1**, Oxide/Parchment **4.55:1**, Chalk/Iron **9.80:1**, Clay/Iron **7.62:1**. Copper, Sage, Wheat and Rose on Iron are **5.55, 8.48, 9.34 and 6.21:1** respectively. Amber/Maroon is **5.78:1**. Gold, Amber and Coral are decorative on light surfaces, not light-mode text colours.

## Icon sources and small sizes

The art is the original thumbnail 11, [leon-original-280.png](icon/leon-original-280.png): 280 × 280, with its grain and soft mane edge. Production uses it as it is, scaled up. No image generator and no AI upscaler touches it. An earlier cleaned render is in git history only.

`build-assets.cjs` makes the [1024 PNG master](icon/leon-1024.png) from it in three steps:

1. Enlarge the thumbnail 3× to 840 × 840 with a Lanczos-3 filter. At 3× every original pixel is still in the result, unchanged, at the centre of its 3 × 3 block.
2. Place it on a clear 1024 × 1024 canvas with its top-left corner at (92, 94).
3. Keep the thumbnail's own rounded tile and clear everything else: an 812 × 826 rectangle at (106, 99) with corner radius 126. That is the thumbnail's tile pulled in about 1.3 of its pixels, which cuts off its blend with the parchment around it, so no light fringe is left. Apple's grid has an 824 × 824 tile; the thumbnail's is slightly taller than wide and keeps that shape.

[check-icon.cjs](check-icon.cjs) holds those numbers and compares the pixels: the tile with the filter's output, the original's pixels at the block centres, the clear canvas, the tile's edge, the icon slots and the README banner. The builder runs it as its last step and exits with an error if it fails. The [1024 SVG](icon/leon-1024.svg) is a standalone **raster wrapper**, explicitly embedding that PNG; it is not editable vector artwork.

On macOS 27 the system redraws an app icon in its own rounded shape. This tile is close enough to the grid that the system fills that shape with it; it does not set it on a grey plate.

The [optical SVG](icon/leon-small.svg) supplies one file, the 16 × 16 px slot (16 pt at 1×), and the page favicon. Reduced to 16 px the original's face is a smudge, so that size keeps the optical icon: one rounded mane shape and a simple warm gradient, no tile rim or shadow, and larger lids, nose and half-smile. Every slot of 32 px and up, 16 pt at 2× included, is the original.

![Native 16, 32, and 128 px icons on light and dark backgrounds](icon/size-check.png)

The proof shows 16/32/128 px on Parchment and Iron, plus direct reduction versus the optical 16 px source enlarged 8×. [Leon.iconset](icon/Leon.iconset) contains all ten standard slots; [Leon.icns](icon/Leon.icns) uses Apple's native encoder. The existing bundle script already copies it.

[background.svg](icon/background.svg) and [foreground.svg](icon/foreground.svg) are simplified, unmasked vector references for possible future layered work. They are not the exact layers of the rendered icon. The delivered ICNS is static; it does not claim native adaptive Liquid Glass behaviour or an Icon Composer package.

## Marketing, type and rebuild

- [Hero](marketing/hero.png) / [SVG](marketing/hero.svg): **1600 × 600**.
- [OG](marketing/og.png) / [SVG](marketing/og.svg): **1200 × 630**; the SVG embeds the app icon.
- [Landing page](marketing/index.html): single HTML with inline CSS, embedded font/icon/favicon, phone layout and dark mode.
- [Copy](marketing/copy.md): unchanged tagline, alternatives, 50-word pitch and 150-word description.

Instrument Sans remains the UI/marketing family, with the local Latin WOFF2 and SIL [OFL licence](fonts/OFL.txt). Menlo remains the terminal/code family with a monospace web fallback. The custom wordmark remains vector paths. No app typography or application code changes.

The page and hero retain the illustrated workflow and labelled statuses; the OG shows the current icon. A continuous warm rule replaces the old pane-colour strip. Checkout-relative documentation links and OG metadata remain deployment-time concerns.

```sh
npm install --prefix /tmp/leon-brand-build sharp playwright
NODE_PATH=/tmp/leon-brand-build/node_modules node brand/build-assets.cjs
```

On macOS with Google Chrome and `iconutil`, the builder reads the original thumbnail and the optical SVG, then makes the master, the wrapper, ten icon slots, ICNS, size proof, page embed, OG embed, both banner PNGs and the icon on the repository README's banner (`docs/images/banner.png`; its layout exists only as that PNG, so the icon is redrawn in place). It never calls an image generator. The page and the OG image show the master's middle 944 px, where the tile fills 87.5% of the frame as their layouts expect; the banner draws the tile alone. Flat logo SVGs and banner layouts are editable sources.
