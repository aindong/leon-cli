//! Where a terminal's program is: its process's cwd, and the paths in OSC 7 reports.

use std::path::PathBuf;

use super::clean;

/// The path of an OSC 7 `file://host/path` URL, percent-decoded, if the host is this one (an
/// ssh session reports the remote host's paths).
pub(super) fn pwd_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let (host, path) = rest.split_at(rest.find('/')?);
    if !(host.is_empty() || host == "localhost" || Some(host) == hostname().as_deref()) {
        return None;
    }
    String::from_utf8(percent_decode(path.as_bytes())).ok().map(PathBuf::from)
}

pub(super) fn hostname() -> Option<String> {
    let mut buf = [0u8; 256];
    // SAFETY: gethostname writes at most buf.len() bytes into our buffer.
    if unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) } != 0 {
        return None;
    }
    let end = buf.iter().position(|&b| b == 0)?;
    String::from_utf8(buf[..end].to_vec()).ok()
}

#[cfg(target_os = "macos")]
pub(super) fn process_cwd(pid: i32) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as libc::c_int;
    // SAFETY: proc_pidinfo writes at most `size` bytes into our zeroed struct; the path is then
    // read as the struct's own MAXPATHLEN bytes.
    unsafe {
        let mut info: libc::proc_vnodepathinfo = std::mem::zeroed();
        let ptr = &mut info as *mut libc::proc_vnodepathinfo as *mut libc::c_void;
        if libc::proc_pidinfo(pid, libc::PROC_PIDVNODEPATHINFO, 0, ptr, size) != size {
            return None;
        }
        let raw = &info.pvi_cdir.vip_path;
        let bytes = std::slice::from_raw_parts(raw.as_ptr() as *const u8, std::mem::size_of_val(raw));
        let path = &bytes[..bytes.iter().position(|&b| b == 0)?];
        (!path.is_empty()).then(|| PathBuf::from(std::ffi::OsStr::from_bytes(path)))
    }
}

#[cfg(target_os = "linux")]
pub(super) fn process_cwd(pid: i32) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(super) fn process_cwd(_pid: i32) -> Option<PathBuf> {
    None
}

/// An OSC 7 URL to report, or None. It is dropped whole when it isn't UTF-8 or when it, or its
/// percent-decoding (which whoever shows the path does), holds anything `clean` would remove.
pub(super) fn pwd_url(raw: &[u8]) -> Option<String> {
    let url = std::str::from_utf8(raw).ok()?;
    let decoded = String::from_utf8(percent_decode(raw)).ok()?;
    (clean(url) == url && clean(&decoded) == decoded).then(|| url.to_string())
}

/// `%XX` escapes as bytes; anything else (a stray `%` too) as is.
pub(super) fn percent_decode(s: &[u8]) -> Vec<u8> {
    let hex = |b: u8| (b as char).to_digit(16);
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        match (s[i], s.get(i + 1).copied().and_then(hex), s.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(hi), Some(lo)) => {
                out.push((hi * 16 + lo) as u8);
                i += 3;
            }
            (b, _, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    out
}
