//! `leon self-update`: what it prints. The checking and installing are `leon_update`'s, which the
//! app shares; docs/updates.md is the user's side.

use leon_update::{Check, Config, Method, Target};

use crate::client::Fail;

/// `--check` exits with this when a newer release exists, so a script can tell it from a failure.
const AVAILABLE: i32 = 10;

pub fn run(check: bool) -> Result<(), Fail> {
    let cfg = Config::load();
    let target = Target::running().map_err(Fail::Other)?;
    let release = match leon_update::check(&cfg).map_err(Fail::Other)? {
        Check::NoRelease => {
            println!("no release yet: leon {} is the newest there is.", cfg.version());
            return Ok(());
        }
        Check::UpToDate { latest } => {
            if !check {
                leon_update::tidy(&target); // what a killed update left
            }
            match latest == cfg.version() {
                true => println!("leon {latest} is up to date."),
                false => println!("leon {} is up to date (the latest release is {latest}).", cfg.version()),
            }
            return Ok(());
        }
        Check::Available(release) => release,
    };
    println!("leon {} is available; you have {}.", release.version, cfg.version());
    match (target.method(), check) {
        (Method::Homebrew { cask }, _) => {
            let cask = if cask { "--cask " } else { "" };
            println!("Homebrew installed this leon, and updates it:\n  brew upgrade {cask}aindong/leon/leon");
        }
        (Method::Dpkg, _) => {
            let deb = release.deb_url(&cfg).map_err(Fail::Other)?;
            println!(
                "This leon came from a .deb, so dpkg updates it. Download\n  {deb}\nand run `dpkg -i` on it as root."
            );
        }
        (Method::Own, true) => println!("Install it with: leon self-update"),
        (Method::Own, false) => {
            leon_update::install(&cfg, &release, &target, &mut |line| println!("{line}")).map_err(Fail::Other)?;
            println!("installed leon {} at {} (it was {}).", release.version, target.root.display(), cfg.version());
            if target.kind == "app" {
                println!("Quit Leon and open it again to use the new app.");
            }
            println!(
                "A leon server that is running keeps the old version, and your sessions, until it restarts.\n\
                 For the new version: leon server stop   (your sessions are saved; the programs in them end)\n\
                 The next leon command starts the new server and brings the sessions back."
            );
        }
    }
    match check {
        true => Err(Fail::Exit(AVAILABLE)),
        false => Ok(()),
    }
}
