# Performance: memory

The server's footprint after a pane closes, and what the app leaks. The machine, the scripts and
how they run are in docs/performance.md.

## Two leaks in GPUI

The app's `leaks` (`MallocStackLogging=1`, release build, agents reporting status) are all
allocated under GPUI 0.2.2, none in leon's code: 44 objects (2.8 KB) after 6 minutes, a few more
an hour (56, 2.5 KB, after the 2-hour soak). leon carries no patched GPUI for that; these are the
fixes, ready to send upstream.

- `src/platform/mac/open_type.rs`, `apply_features_and_fallbacks`: the arrays from
  `generate_feature_array` and `generate_fallback_array` come from `CFArrayCreateMutable` (the
  caller owns them) and go into a dictionary that retains them, but are never released: arrays
  leak once per font resolved. Fix, after `CFDictionaryCreate(...)`:
  `for value in &values { CFRelease(*value as _); }`
- `src/platform/mac/window.rs`, `display_id_for_screen`: the `NSScreenNumber` key is made with
  `NSString::alloc(nil).init_str(...)` (owned) and never released; `start_display_link` calls it
  at every key-window and occlusion change. Fix:
  `let screen_number_key: id = NSString::alloc(nil).init_str("NSScreenNumber").autorelease();`
  `MacWindow::display` makes the same key the same way; the same `.autorelease()` fixes it.
