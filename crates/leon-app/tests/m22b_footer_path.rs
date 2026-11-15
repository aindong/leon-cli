//! The Settings footer names init.lua with the home folder as `~`, so a real install's login name
//! never shows in the window or a screenshot of it.

use std::path::Path;

use leon_app::settings_window::home_relative;

#[test]
fn the_home_folder_is_a_tilde() {
    let home = Some(Path::new("/Users/someone"));
    let at = |p: &str| home_relative(Path::new(p), home);
    assert_eq!(at("/Users/someone/.config/leon/init.lua"), "~/.config/leon/init.lua");
    assert_eq!(at("/Users/someone"), "~");
    assert_eq!(at("/Users/someone-else/init.lua"), "/Users/someone-else/init.lua", "a folder only named like it");
    assert_eq!(at("/etc/leon/init.lua"), "/etc/leon/init.lua", "outside home: as it is");
    assert_eq!(home_relative(Path::new("/Users/someone/init.lua"), None), "/Users/someone/init.lua", "no HOME");
}
