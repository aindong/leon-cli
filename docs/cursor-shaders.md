# Cursor shaders

Leon.app can draw a Shadertoy-style cursor shader over its window: a small GLSL file that draws a
trail, a smear or a glow as the cursor moves. You name the file in init.lua; leon translates and
compiles it, and draws it while the cursor's move animates. macOS only for now.

**Download shaders from their authors; leon ships none.** Many are published with a licence that
lets you use your own copy; some have none, so keep yours for yourself.

## Use

```lua
leon.set("cursor_shader", "~/.config/leon/shaders/cursor_smear.glsl")
leon.set("cursor_shader_duration", 0.5) -- seconds it draws after a move, 0.1 to 5
```

`cursor_shader` is the file: `~/` is your home, and a relative path is relative to init.lua's
folder. `""` (the default) is none. `cursor_shader_duration` (default 1) is how long the overlay
keeps drawing after the cursor changes; most shaders fade within half a second.

`leon config check` translates the file as the app will and prints `cursor_shader: PATH: ok`, or
the reason with the line in your file (`cursor_shader: cursor_smear.glsl:12: …`) and exits 1. The
app shows the same reason in its notice at the bottom right and keeps running without the effect.
It reloads the shader when the setting changes and when you save the file, so you can edit it
with the app open; a file that fails keeps the last good shader running. With Reduce Motion on
(System Settings › Accessibility › Display) the effect is off. The Settings window (⌘,) has
the file under Appearance, with Choose… and None, and the duration.

## The shader

The file defines one function, which leon calls for every pixel of the window:

```glsl
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec4 base = texture(iChannel0, fragCoord / iResolution.xy);
    fragColor = base; // draw your trail over base here
}
```

It is GLSL 4.5 for a fragment shader. leon puts the uniforms below, `iChannel0` and the
`CURSORSTYLE_*` names before your file, and its own `main`, so the file must not declare them.

| Uniform | Type | What it is |
|---|---|---|
| `iResolution` | vec3 | the window's size in device pixels (2× points on a Retina display); z is 1 |
| `iTime` | float | seconds since the overlay started |
| `iTimeDelta` | float | seconds since the frame before, 0 on an animation's first frame |
| `iFrame` | int | frames drawn so far |
| `iChannelResolution[0]` | vec3 | the same as `iResolution` |
| `iCurrentCursor` | vec4 | the cursor: x and y are its left and **bottom** edge, z and w its width and height, in device pixels from the window's top left, y down |
| `iPreviousCursor` | vec4 | the cursor before the last change, the same way |
| `iCurrentCursorColor`, `iPreviousCursorColor` | vec4 | the cursor's colour, 0 to 1, alpha 1 |
| `iCurrentCursorStyle`, `iPreviousCursorStyle` | int | `CURSORSTYLE_BLOCK` 0, `CURSORSTYLE_BLOCK_HOLLOW` 1, `CURSORSTYLE_BAR` 2, `CURSORSTYLE_UNDERLINE` 3 |
| `iCursorVisible` | int | 1, or 0 while the program hides the cursor |
| `iTimeCursorChange` | float | `iTime` when the cursor last moved or changed style or colour |
| `iTimeFocus` | float | `iTime` when drawing last turned on (the window became active) |
| `iFocus` | int | 1 while the overlay draws |
| `iPalette[256]` | vec3 | the theme's 16 ANSI colours, then zeros |
| `iBackgroundColor`, `iForegroundColor`, `iCursorColor`, `iCursorText` | vec3 | the theme's colours, 0 to 1 |
| `iChannelTime[4]`, `iMouse`, `iDate`, `iSampleRate`, `iFrameRate`, `iSelectionForegroundColor`, `iSelectionBackgroundColor` | | declared, always 0 |

`iChannel0` is the picture under the shader, as Shadertoy's channel 0 (also `texture2D`).

## What works and what doesn't

The shader runs on a transparent layer of leon's own above the window, not inside leon's own
drawing, so it never sees the picture. leon runs `mainImage` twice per pixel, once over flat black
and once over flat white, and works out from the two what the shader added and how opaque it is.

- **Works:** shaders that draw over the picture: a trail, smear, glow, sweep or ripple that ends
  in `mix(base, colour, amount)` or adds to `base`. A glow mixed toward a colour brighter than 1.0
  is close, not exact: it can look a little stronger at its edge.
- **Doesn't work:** shaders that read or bend the picture: CRT, bloom, blur, scanlines, a trail
  coloured with the inverse of the text under it (`1.0 - texture(iChannel0, uv)`). They draw
  wrong, and `leon config check` can't tell them apart.
- Only the focused pane's cursor moves the effect, and only in the active window. A move to
  another pane, or a pane sliding, draws no trail; while a float is open over the tiles only the
  float's cursor does. The overview, the command palette and Reduce Motion turn it off.
- The effect is one display frame behind the cursor (16 ms at 60 Hz), so the first frame of a
  long jump shows the cursor without its trail.
- leon translates GLSL with naga, which is stricter than some GLSL compilers: an implicit
  int-to-float conversion, or a `return` early in `mainImage`, can fail with naga's message.

## Cost

Nothing while the cursor is still: the layer's display link runs only for
`cursor_shader_duration` after a change. While it runs, a shader costs its own GPU time twice a
pixel (the two passes): 0.1 to 0.6 ms a frame at 2880×1800 for the trail shaders tried, and
about 4 ms for a heavy noise shader. Typing stays as fast: the overlay draws on its own thread.

How it is built: [architecture/cursor-shaders.md](architecture/cursor-shaders.md).
