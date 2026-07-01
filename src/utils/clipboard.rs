use std::io;
use std::path::Path;
use std::process::Command;

fn first_url(text: &str) -> Option<&str> {
    let first = text.trim().split_whitespace().next()?;
    if first.starts_with("http://") || first.starts_with("https://") {
        Some(first)
    } else {
        None
    }
}

#[allow(dead_code)]
fn file_uri_to_path(uri: &str) -> String {
    let path = uri.trim().strip_prefix("file://").unwrap_or(uri.trim());
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&path[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn image_ext(data: &[u8]) -> Option<String> {
    if data.starts_with(b"\x89PNG") {
        return Some("png".to_string());
    }
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg".to_string());
    }
    if data.starts_with(b"GIF8") {
        return Some("gif".to_string());
    }
    if data.len() >= 12 && data.starts_with(b"RIFF") && data[8..12] == *b"WEBP" {
        return Some("webp".to_string());
    }
    if data.starts_with(b"BM") {
        return Some("bmp".to_string());
    }
    None
}

#[allow(dead_code)]
fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    if dib.len() < 40 {
        return None;
    }
    let bi_size = u32::from_le_bytes(dib[0..4].try_into().ok()?) as usize;
    let bit_count = u16::from_le_bytes([dib[14], dib[15]]);
    let compression = u32::from_le_bytes(dib[16..20].try_into().ok()?);
    let clr_used = u32::from_le_bytes(dib[32..36].try_into().ok()?);

    let palette = if bit_count <= 8 {
        let n = if clr_used != 0 {
            clr_used as usize
        } else {
            1usize << bit_count
        };
        n * 4
    } else {
        0
    };
    let masks = if compression == 3 && bi_size == 40 { 12 } else { 0 };

    let pixel_offset = 14 + bi_size + palette + masks;
    let file_size = 14 + dib.len();

    let mut out = Vec::with_capacity(file_size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(pixel_offset as u32).to_le_bytes());
    out.extend_from_slice(dib);
    Some(out)
}

fn download_image(url: &str) -> Option<(Vec<u8>, String)> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    let out = Command::new("curl")
        .args([
            "-sSL",
            "--proto",
            "=http,https",
            "--proto-redir",
            "=http,https",
            "--max-redirs",
            "5",
            "--max-time",
            "60",
            "--max-filesize",
            "52428800",
            url,
        ])
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    let ext = image_ext(&out.stdout)?;
    Some((out.stdout, ext))
}

#[cfg(target_os = "macos")]
mod imp {
    use std::io;
    use std::path::Path;

    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{
        NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeString,
        NSPasteboardWriting, NSWorkspace,
    };
    use objc2_foundation::{NSArray, NSString, NSURL};

    fn general() -> Retained<NSPasteboard> {
        NSPasteboard::generalPasteboard()
    }

    fn url_for(path: &Path) -> io::Result<Retained<NSURL>> {
        let abs = std::fs::canonicalize(path)?;
        let s = NSString::from_str(&abs.to_string_lossy());
        Ok(NSURL::fileURLWithPath(&s))
    }

    pub fn paste() -> io::Result<(Vec<u8>, String)> {
        let pb = general();

        if let Some(s) = unsafe { pb.stringForType(NSPasteboardTypeFileURL) } {
            let path = super::file_uri_to_path(&s.to_string());
            let p = Path::new(&path);
            if p.is_file() {
                let bytes = std::fs::read(p)?;
                let ext = p
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();
                return Ok((bytes, ext));
            }
        }

        if let Some(data) = unsafe { pb.dataForType(NSPasteboardTypePNG) } {
            let bytes = data.to_vec();
            if !bytes.is_empty() {
                return Ok((bytes, "png".to_string()));
            }
        }

        if let Some(s) = unsafe { pb.stringForType(NSPasteboardTypeString) } {
            if let Some(url) = super::first_url(&s.to_string()) {
                if let Some(res) = super::download_image(url) {
                    return Ok(res);
                }
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "nothing on the clipboard to fossilize (copy a file, image, or image link first)",
        ))
    }

    pub fn copy(path: &Path) -> io::Result<()> {
        let url = url_for(path)?;
        let writing: &ProtocolObject<dyn NSPasteboardWriting> = ProtocolObject::from_ref(&*url);
        let objs = NSArray::from_slice(&[writing]);
        let pb = general();
        pb.clearContents();
        if pb.writeObjects(&objs) {
            Ok(())
        } else {
            Err(io::Error::other("could not set the clipboard"))
        }
    }

    pub fn reveal(path: &Path) -> io::Result<()> {
        let url = url_for(path)?;
        let arr = NSArray::from_slice(&[&*url]);
        let ws = NSWorkspace::sharedWorkspace();
        ws.activateFileViewerSelectingURLs(&arr);
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::io::{self, Write};
    use std::path::Path;
    use std::process::{Command, Stdio};

    fn wayland() -> bool {
        std::env::var_os("WAYLAND_DISPLAY").is_some()
    }

    pub fn paste() -> io::Result<(Vec<u8>, String)> {
        if let Some(bytes) = get_target("text/uri-list") {
            let text = String::from_utf8_lossy(&bytes);
            if let Some(line) = text.lines().find(|l| l.starts_with("file://")) {
                let path = decode_file_uri(line.trim());
                let p = Path::new(&path);
                if p.is_file() {
                    let data = std::fs::read(p)?;
                    let ext = p
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_string();
                    return Ok((data, ext));
                }
            }
        }
        if let Some(bytes) = get_target("image/png") {
            if !bytes.is_empty() {
                return Ok((bytes, "png".to_string()));
            }
        }
        if let Some(text) = clipboard_text() {
            if let Some(url) = super::first_url(&text) {
                if let Some(res) = super::download_image(url) {
                    return Ok(res);
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "nothing on the clipboard to fossilize (need xclip or wl-clipboard, and a file, image, or image link copied first)",
        ))
    }

    fn clipboard_text() -> Option<String> {
        let out = if wayland() {
            Command::new("wl-paste").arg("--no-newline").output().ok()?
        } else {
            Command::new("xclip")
                .args(["-selection", "clipboard", "-o"])
                .output()
                .ok()?
        };
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).into_owned();
        if s.trim().is_empty() { None } else { Some(s) }
    }

    pub fn copy(path: &Path) -> io::Result<()> {
        let abs = std::fs::canonicalize(path)?;
        let payload = format!("copy\nfile://{}", abs.to_string_lossy());
        set_target("x-special/gnome-copied-files", payload.as_bytes())
    }

    pub fn reveal(path: &Path) -> io::Result<()> {
        let abs = std::fs::canonicalize(path)?;
        let dir = abs.parent().unwrap_or(&abs);
        Command::new("xdg-open").arg(dir).status()?;
        Ok(())
    }

    fn get_target(target: &str) -> Option<Vec<u8>> {
        let out = if wayland() {
            Command::new("wl-paste")
                .args(["--type", target])
                .output()
                .ok()?
        } else {
            Command::new("xclip")
                .args(["-selection", "clipboard", "-t", target, "-o"])
                .output()
                .ok()?
        };
        if !out.status.success() || out.stdout.is_empty() {
            return None;
        }
        Some(out.stdout)
    }

    fn set_target(target: &str, data: &[u8]) -> io::Result<()> {
        let mut cmd = if wayland() {
            let mut c = Command::new("wl-copy");
            c.args(["--type", target]);
            c
        } else {
            let mut c = Command::new("xclip");
            c.args(["-selection", "clipboard", "-t", target]);
            c
        };
        let mut child = cmd.stdin(Stdio::piped()).spawn().map_err(|_| {
            io::Error::other("couldn't run the clipboard tool (install xclip or wl-clipboard)")
        })?;
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| io::Error::other("no stdin"))?;
            stdin.write_all(data)?;
        }
        if child.wait()?.success() {
            Ok(())
        } else {
            Err(io::Error::other("clipboard tool failed"))
        }
    }

    fn decode_file_uri(uri: &str) -> String {
        let path = uri.strip_prefix("file://").unwrap_or(uri);
        let bytes = path.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                if let Ok(b) = u8::from_str_radix(&path[i + 1..i + 3], 16) {
                    out.push(b);
                    i += 3;
                    continue;
                }
            }
            out.push(bytes[i]);
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use std::io;
    use std::path::Path;
    use std::process::Command;

    use clipboard_win::{Clipboard, Setter, formats, get_clipboard, register_format};

    pub fn paste() -> io::Result<(Vec<u8>, String)> {
        if let Ok(files) = get_clipboard::<Vec<String>, _>(formats::FileList) {
            if let Some(first) = files.into_iter().next() {
                let p = Path::new(&first);
                if p.is_file() {
                    let bytes = std::fs::read(p)?;
                    let ext = p
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_string();
                    return Ok((bytes, ext));
                }
            }
        }
        if let Some(fmt) = register_format("PNG") {
            if let Ok(bytes) = get_clipboard::<Vec<u8>, _>(formats::RawData(fmt.get())) {
                if !bytes.is_empty() {
                    return Ok((bytes, "png".to_string()));
                }
            }
        }
        if let Ok(dib) = get_clipboard::<Vec<u8>, _>(formats::RawData(8)) {
            if let Some(bmp) = super::dib_to_bmp(&dib) {
                return Ok((bmp, "bmp".to_string()));
            }
        }
        if let Ok(text) = get_clipboard::<String, _>(formats::Unicode) {
            if let Some(url) = super::first_url(&text) {
                if let Some(res) = super::download_image(url) {
                    return Ok(res);
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "nothing on the clipboard to fossilize (copy a file, image, or image link first)",
        ))
    }

    pub fn copy(path: &Path) -> io::Result<()> {
        let abs = std::fs::canonicalize(path)?;
        let abs = abs.to_string_lossy().into_owned();
        let s = abs.strip_prefix(r"\\?\").unwrap_or(abs.as_str()).to_string();
        let _clip = Clipboard::new_attempts(10).map_err(|e| io::Error::other(e.to_string()))?;
        formats::FileList
            .write_clipboard(&[s])
            .map_err(|e| io::Error::other(e.to_string()))
    }

    pub fn reveal(path: &Path) -> io::Result<()> {
        let abs = std::fs::canonicalize(path)?;
        let abs = abs.to_string_lossy().into_owned();
        let s = abs.strip_prefix(r"\\?\").unwrap_or(abs.as_str());
        Command::new("explorer")
            .arg(format!("/select,{}", s))
            .status()?;
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod imp {
    use std::io;
    use std::path::Path;

    pub fn paste() -> io::Result<(Vec<u8>, String)> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "clipboard packing isn't supported on this platform",
        ))
    }

    pub fn copy(_path: &Path) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "clipboard packing isn't supported on this platform",
        ))
    }

    pub fn reveal(_path: &Path) -> io::Result<()> {
        Ok(())
    }
}

pub fn paste() -> io::Result<(Vec<u8>, String)> {
    imp::paste()
}

pub fn copy(path: &Path) -> io::Result<()> {
    imp::copy(path)
}

pub fn reveal(path: &Path) -> io::Result<()> {
    imp::reveal(path)
}
