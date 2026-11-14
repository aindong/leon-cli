# leon identity

**Chosen direction: Open corner.** A solid L-shaped rail holds a separate square pane in an open corner. The rail gives the workspace structure; the separate pane gives another job room to run. It reads as both the initial and a small arrangement of terminals, with two solid shapes that survive reduction to an icon.

**Tagline:** Keep your work in view.

![leon hero](marketing/hero.png)

## Three directions explored

The [imagegen concept sheet](exploration/directions.png) explores three routes. The [research](research.md) documents the reference brands; the [generation prompts](exploration/prompts.md) record both built-in imagegen calls.

1. **Open corner — selected.** A rail and one detached pane suggest orientation, parallel work, and calm control. It has the clearest relation to the name, works in one colour, and stays recognisable without detail.
2. **Parallel current.** Three advancing rails suggest speed and simultaneous jobs. Rejected because it becomes a generic speed symbol and says less about a place to work.
3. **Focus aperture.** A broken square ring surrounds an active pane. Rejected because it is closer to a generic scan/focus control and its thin gaps weaken at small sizes.

The final vectors distil the first exploration into an 80-unit square: a 64-unit occupied area, an 18-unit rail, a 36-unit square, and a 10-unit channel. The square and rail share a colour. This corrects the concept sheet's low-contrast, two-colour icon treatment. The final wordmark is original hand-drawn SVG geometry, not lettering extracted from the generated image.

## Name and mark

Use **leon** in running product copy and the lowercase wordmark. Use **Leon.app** for the actual macOS bundle name and `leon` for commands. The wordmark's open e, round o, and quiet lowercase forms keep the identity approachable without a mascot.

| Asset | For light surfaces | For dark surfaces |
| --- | --- | --- |
| Full lockup | [logo.svg](logo.svg) | [logo-dark.svg](logo-dark.svg) |
| Standalone mark | [logo-mark.svg](logo-mark.svg) | [logo-mark-dark.svg](logo-mark-dark.svg) |
| Standalone wordmark | [wordmark.svg](wordmark.svg) | [wordmark-dark.svg](wordmark-dark.svg) |

All six files are clean, hand-authored SVG paths and shapes, with no font or bitmap dependency. The unsuffixed versions are the light variants. Keep at least one rail-width of clear space around the occupied mark; do not stretch, rotate, add a prompt, or recolour its two parts separately. For a monochrome use, give both parts the same foreground colour. The flat mark works down to 16 px; use the full lockup at 110 px wide or larger. On busy surfaces use a solid paper or iron field.

## Palette

| Token | Hex | Role |
| --- | --- | --- |
| Parchment | `#F5F0E7` | Light background, inverse text, icon symbol |
| Iron | `#292521` | Primary text, dark background |
| Oxide | `#B64C32` | Main mark and light-mode accent |
| Clay | `#E6AA91` | Dark-mode accent, working state |
| Umber | `#65594F` | Secondary light-mode text |
| Chalk | `#D8CEC2` | Secondary dark-mode text |
| Sand | `#EBE3D7` | Light raised surface |
| Coal | `#1F1C19` | Terminal pane surface |
| Sage | `#B8C6AC` | Done status on dark surfaces |
| Wheat | `#E8C77D` | Blocked status on dark surfaces |

Calculated from sRGB relative luminance using `(Llighter + 0.05) / (Ldarker + 0.05)`, rounded to two decimals. These approved text pairs meet WCAG AA's 4.5:1 normal-text threshold; the icon's lighting is decorative rather than a text colour specification.

| Text / background | Contrast |
| --- | ---: |
| Iron / Parchment | 13.40:1 |
| Umber / Parchment | 5.98:1 |
| Oxide / Parchment | 4.55:1 |
| Parchment / Oxide | 4.55:1 |
| Parchment / Iron | 13.40:1 |
| Chalk / Iron | 9.80:1 |
| Clay / Iron | 7.62:1 |
| Sage / Iron | 8.48:1 |
| Wheat / Iron | 9.34:1 |
| Chalk / Coal | 10.92:1 |

Use the lighter Clay accent on dark backgrounds. Do not use Oxide as small text on Iron. Pair every status colour with its word and a distinct symbol, as the demo does; colour alone is insufficient.

## Type

- **UI and marketing:** [Instrument Sans](https://fonts.google.com/specimen/Instrument+Sans), a free SIL Open Font License family, weights 400–700. Use 400 for body text, 500–600 for headings, and 600 for actions. The supplied 29 KB Latin WOFF2 and [licence](fonts/OFL.txt) are embedded in the standalone page and SVG marketing sources; no font service is called.
- **Terminal and code:** **Menlo**, the font already used by `FONT_FAMILY` in `crates/leon-app/src/view.rs`. It ships with macOS and is not redistributed here. Web fallback is `monospace`. Instrument Sans is a brand/UI recommendation, not a change to the app's existing system UI font.
- **Logo lettering:** bespoke vector paths. Do not typeset a replacement with a similar font.

## macOS icon

The [1024×1024 RGBA master](icon/leon-1024.png) is original imagegen raster art: an oxide rounded square, restrained glass rim, and raised parchment shapes. The [complete iconset](icon/Leon.iconset) and [Leon.icns](icon/Leon.icns) are ready for the existing macOS packaging flow. The bitmap has transparent space outside the tile.

![16, 32, and 128 pixel icon checks on light and dark backgrounds](icon/size-check.png)

I checked **16, 32, and 128 px side by side**, on light and dark backgrounds. A plain 16 px reduction softened the channel and let the bright outer rim compete with the mark. The [optical small-size source](icon/leon-small.svg) removes the fine rim and cast shadow, uses a larger flat symbol, and keeps a 1.5 px channel on its 16-unit grid. Both 16 pt and 32 pt slots, including their Retina versions, use that correction; 128 pt and larger use the dimensional raster master. There is a deliberately simpler small-icon appearance.

All ten required PNG names are present: 16, 32, 128, 256, and 512 pt, each at 1× and 2×. `iconutil` built the ICNS successfully. [Unmasked background](icon/background.svg) and [foreground](icon/foreground.svg) are also supplied for future Icon Composer work. The delivered ICNS is static; it does not claim native adaptive Liquid Glass behaviour or include a compiled `.icon` package.

`scripts/bundle.sh` was left unchanged: its current two-step Swift/iconutil flow still produces the existing app icon. The coordinator can adopt the supplied ICNS when integrating the brand. Root `README.md` was not edited.

## Marketing kit

- [README hero PNG](marketing/hero.png), **1600×600**, with [editable SVG source](marketing/hero.svg).
- [Social / OG PNG](marketing/og.png), **1200×630**, with [editable SVG source](marketing/og.svg).
- [Copy sheet](marketing/copy.md): primary tagline, three alternatives, exactly 50 words of pitch and exactly 150 words of description, using whitespace word counts.
- [Landing page](marketing/index.html): one HTML file with inline CSS, inline vector identity, embedded font and icon, and zero scripts or runtime network requests. Includes six product capabilities, a terminal demo, the real source-install command, and an agent section.

The workflow panels are original illustrations, explicitly labelled as such. They are not screenshots, measured test results, or evidence that every program reports status automatically. The page uses the architecture's restart behaviour: quitting the app leaves programs running; when the server stops, layouts restore and programs don't.

The page can be opened directly from the checkout. Its documentation links are repository-relative; before public hosting, point those links at the hosted docs and set `og:image` to the deployed absolute URL. No deployment hostname was invented.

## Rebuild and checks

On macOS with Node and Google Chrome installed:

```sh
npm install --prefix /tmp/leon-brand-build sharp playwright
NODE_PATH=/tmp/leon-brand-build/node_modules node brand/build-assets.cjs
```

The build script renders the SVG banners with their embedded font, optimises PNGs, builds all icon slots and the ICNS, and renders the optical-size comparison. It treats the generated 1024 px master as an input. The page is directly editable HTML; its embedded font and icon keep it portable.

Verified in Chrome at **1440, 390, and 320 px**, in both light and dark modes: no page errors, no horizontal overflow, six feature items, loaded font and imagery, no scripts, and no external resource requests. Desktop and 390 px renders were visually inspected, alongside both exported banners and the icon proof. SVGs parse as XML; PNG dimensions, alpha, iconset slots, copy lengths, local page anchors, and documentation destinations were checked. See [verification.md](verification.md) for the recorded result and [FILES.md](FILES.md) for every delivered file.

## Human review

The chosen mark is ready; there is no unresolved choice between final marks. Judge whether the oxide icon's warmth and glass depth feel right beside the user's other Dock apps, and whether the custom rounded lettering feels sufficiently technical. The optical small-icon treatment and dimensional master should also be judged together on a real Retina display. These are aesthetic review points, not uncompleted deliverables.
