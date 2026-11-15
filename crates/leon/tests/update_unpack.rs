//! Update criterion 9 (.agent/plans/2026-11-16-self-update-core.md): an archive is unpacked with
//! care even though its signature verified. An entry with an absolute path, with `..`, or a link
//! that leaves the directory refuses the whole archive; a good one keeps its executable bit.

mod release_kit;
mod update_kit;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use tar::EntryType;
use update_kit::{assert_refused, fake_leon, name, Install, Release, VERSION};

type Tar = tar::Builder<flate2::write::GzEncoder<Vec<u8>>>;

/// Add an entry whose name and link are written as given: tar's own setters refuse the bad ones.
fn entry(tar: &mut Tar, path: &str, kind: EntryType, link: &str, data: &[u8]) {
    let mut h = tar::Header::new_gnu();
    let gnu = h.as_gnu_mut().unwrap();
    assert!(path.len() < 100 && link.len() < 100, "the fixture's names fit a tar header: {path} {link}");
    gnu.name[..path.len()].copy_from_slice(path.as_bytes());
    gnu.linkname[..link.len()].copy_from_slice(link.as_bytes());
    h.set_mode(0o755);
    h.set_size(data.len() as u64);
    h.set_entry_type(kind);
    h.set_cksum();
    tar.append(&h, data).unwrap();
}

/// A CLI tarball with a good leon, then whatever `more` adds.
fn tarball(more: impl FnOnce(&mut Tar, &str)) -> Vec<u8> {
    let top = name("cli").trim_end_matches(".tar.gz").to_string();
    let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
    entry(&mut tar, &format!("{top}/"), EntryType::Directory, "", b"");
    entry(&mut tar, &format!("{top}/leon"), EntryType::Regular, "", &fake_leon(VERSION));
    more(&mut tar, &top);
    tar.into_inner().unwrap().finish().unwrap()
}

/// Every file named `name` under `dir`.
fn find(dir: &Path, name: &str, found: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let (path, kind) = (e.path(), e.file_type().unwrap());
        if e.file_name() == name {
            found.push(path.clone());
        }
        if kind.is_dir() {
            find(&path, name, found);
        }
    }
}

/// Serve `archive` as this machine's CLI tarball, correctly signed; the update must refuse it for
/// `why` and write no file named `planted` anywhere.
fn assert_archive_refused(leon: &Install, archive: Vec<u8>, why: &str, planted: &str) {
    let rel = Release::unsigned();
    rel.put(&name("cli"), &archive);
    rel.sign();
    let srv = rel.serve();
    let before = leon.state();
    let o = leon.update(&rel, &srv, &[]);
    assert_refused(&o, &[&name("cli"), "refusing", why]);
    leon.assert_unchanged(before, why);
    let mut found = vec![];
    find(&leon.home, planted, &mut found);
    assert!(found.is_empty(), "the entry was written: {found:?}");
}

#[test]
fn an_entry_with_an_absolute_path_is_refused() {
    let leon = Install::cli();
    let outside = leon.home.join("abs-evil");
    let archive = tarball(|tar, _| entry(tar, outside.to_str().unwrap(), EntryType::Regular, "", b"planted"));
    assert_archive_refused(&leon, archive, "absolute path", "abs-evil");
}

#[test]
fn an_entry_with_dot_dot_is_refused() {
    let leon = Install::cli();
    let archive =
        tarball(|tar, top| entry(tar, &format!("{top}/../../../dotdot-evil"), EntryType::Regular, "", b"planted"));
    assert_archive_refused(&leon, archive, "\"..\"", "dotdot-evil");
}

#[test]
fn a_symlink_that_leaves_the_directory_is_refused() {
    // the link alone is enough: nothing has to be written through it
    for target in ["../../..", "/", "a/../../../.."] {
        let leon = Install::cli();
        let archive = tarball(|tar, top| entry(tar, &format!("{top}/out"), EntryType::Symlink, target, b""));
        assert_archive_refused(&leon, archive, "leaves the archive's directory", "out");
    }
}

#[test]
fn an_entry_written_through_a_link_is_refused() {
    // a link that stays inside is fine; an entry whose path goes through it is not
    let leon = Install::cli();
    let archive = tarball(|tar, top| {
        entry(tar, &format!("{top}/docs/"), EntryType::Directory, "", b"");
        entry(tar, &format!("{top}/same"), EntryType::Symlink, "docs", b"");
        entry(tar, &format!("{top}/same/through-evil"), EntryType::Regular, "", b"planted");
    });
    assert_archive_refused(&leon, archive, "written through a link", "through-evil");
}

#[test]
fn an_archive_that_says_it_unpacks_to_too_much_is_refused() {
    // the header's size is what counts: the archive is refused before the entry is written
    let leon = Install::cli();
    let archive = tarball(|tar, top| {
        let mut h = tar::Header::new_gnu();
        h.set_path(format!("{top}/huge")).unwrap();
        h.set_mode(0o644);
        h.set_size(2 << 30);
        h.set_cksum();
        tar.append(&h, &b"only a few bytes follow"[..]).unwrap();
    });
    let rel = Release::unsigned();
    rel.put(&name("cli"), &archive);
    rel.sign();
    let before = leon.state();
    assert_refused(&leon.update(&rel, &rel.serve(), &[]), &[&name("cli"), "unpacks to more than"]);
    leon.assert_unchanged(before, "an archive that unpacks to too much");
    let mut found = vec![];
    find(&leon.home, "huge", &mut found);
    assert!(found.is_empty(), "the entry was written: {found:?}");
}

#[test]
fn an_archive_without_a_leon_is_refused() {
    let top = name("cli").trim_end_matches(".tar.gz").to_string();
    let archive = |entries: &dyn Fn(&mut Tar)| {
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        entries(&mut tar);
        tar.into_inner().unwrap().finish().unwrap()
    };
    let leon_at = |tar: &mut Tar, path: &str| entry(tar, path, EntryType::Regular, "", &fake_leon(VERSION));
    type Entries<'a> = &'a dyn Fn(&mut Tar);
    let cases: [(&str, Entries); 5] = [
        ("nothing named leon", &|tar| entry(tar, &format!("{top}/README.md"), EntryType::Regular, "", b"read me")),
        ("a leon in another directory", &|tar| leon_at(tar, "elsewhere/leon")),
        ("a leon that is a link", &|tar| {
            leon_at(tar, &format!("{top}/real"));
            entry(tar, &format!("{top}/leon"), EntryType::Symlink, "real", b"");
        }),
        ("a leon behind a linked directory", &|tar| {
            leon_at(tar, "elsewhere/leon");
            entry(tar, &top, EntryType::Symlink, "elsewhere", b"");
        }),
        ("a leon that is not a program", &|tar| {
            let mut h = tar::Header::new_gnu();
            h.set_path(format!("{top}/leon")).unwrap();
            h.set_mode(0o644);
            h.set_size(4);
            h.set_cksum();
            tar.append(&h, &b"text"[..]).unwrap();
        }),
    ];
    for (what, entries) in cases {
        let leon = Install::cli();
        let rel = Release::unsigned();
        rel.put(&name("cli"), &archive(entries));
        rel.sign();
        let before = leon.state();
        let o = leon.update(&rel, &rel.serve(), &[]);
        assert_eq!(o.code, 1, "{what}: {}", o.text());
        assert!(o.err.contains("holds no leon"), "{what}: {}", o.err);
        leon.assert_unchanged(before, what);
    }
}

#[test]
fn a_hard_link_that_leaves_the_directory_is_refused() {
    let leon = Install::cli();
    let secret = leon.home.join("secret");
    std::fs::write(&secret, "not the archive's to link to").unwrap();
    for target in [secret.to_str().unwrap().to_string(), "../../../secret".to_string()] {
        let archive = tarball(|tar, top| entry(tar, &format!("{top}/hardlink-evil"), EntryType::Link, &target, b""));
        assert_archive_refused(&leon, archive, "link", "hardlink-evil");
    }
}

#[test]
fn a_good_archive_keeps_the_executable_bit_and_links_that_stay_inside() {
    let leon = Install::cli();
    let rel = Release::unsigned();
    let archive = tarball(|tar, top| {
        entry(tar, &format!("{top}/README.md"), EntryType::Regular, "", b"read me");
        entry(tar, &format!("{top}/readme-link"), EntryType::Symlink, "README.md", b"");
    });
    rel.put(&name("cli"), &archive);
    rel.sign();
    let o = leon.update(&rel, &rel.serve(), &[]);
    assert_eq!(o.code, 0, "a good archive installs: {}", o.text());
    let mode = std::fs::metadata(&leon.exe).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0o111, "the installed leon is executable: {mode:o}");
    assert_eq!(leon.version(), format!("leon {VERSION} (fake)"));
}

// macOS only: the app kind is a zip, and only macOS has it
#[cfg(target_os = "macos")]
#[test]
fn a_zip_with_a_bad_entry_is_refused() {
    use zip::write::SimpleFileOptions;
    type Zip = zip::ZipWriter<std::io::Cursor<Vec<u8>>>;
    let exe = SimpleFileOptions::default().unix_permissions(0o755);
    let zip_with = |more: &dyn Fn(&mut Zip)| {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file("Leon.app/Contents/MacOS/leon", exe).unwrap();
        zip.write_all(&fake_leon(VERSION)).unwrap();
        more(&mut zip);
        zip.finish().unwrap().into_inner()
    };
    type Plant<'a> = &'a dyn Fn(&mut Zip, &Path);
    let cases: [(&str, &str, Plant); 4] = [
        ("absolute path", "zip-abs-evil", &|zip, home| {
            zip.start_file(home.join("zip-abs-evil").to_str().unwrap(), exe).unwrap();
            zip.write_all(b"planted").unwrap();
        }),
        ("\"..\"", "zip-dotdot-evil", &|zip, _| {
            zip.start_file("Leon.app/../../../zip-dotdot-evil", exe).unwrap();
            zip.write_all(b"planted").unwrap();
        }),
        ("leaves the archive's directory", "zip-out", &|zip, _| {
            zip.add_symlink("Leon.app/zip-out", "../../..", exe).unwrap();
        }),
        ("written through a link", "zip-through-evil", &|zip, _| {
            zip.add_directory("Leon.app/Contents/Resources/", exe).unwrap();
            zip.add_symlink("Leon.app/same", "Contents/Resources", exe).unwrap();
            zip.start_file("Leon.app/same/zip-through-evil", exe).unwrap();
            zip.write_all(b"planted").unwrap();
        }),
    ];
    // the control: the same builder with no bad entry makes a zip that installs
    let leon = Install::app();
    let rel = Release::unsigned();
    rel.put(&name("app"), &zip_with(&|_| {}));
    rel.sign();
    let o = leon.update(&rel, &rel.serve(), &[]);
    assert_eq!(o.code, 0, "a good zip installs: {}", o.text());
    assert_eq!(leon.version(), format!("leon {VERSION} (fake)"));
    for (why, planted, more) in cases {
        let leon = Install::app();
        let rel = Release::unsigned();
        rel.put(&name("app"), &zip_with(&|zip| more(zip, &leon.home)));
        rel.sign();
        let before = leon.state();
        let o = leon.update(&rel, &rel.serve(), &[]);
        assert_refused(&o, &[&name("app"), "refusing", why]);
        leon.assert_unchanged(before, why);
        let mut found = vec![];
        find(&leon.home, planted, &mut found);
        assert!(found.is_empty(), "the entry was written: {found:?}");
    }
}
