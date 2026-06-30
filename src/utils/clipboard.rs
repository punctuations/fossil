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

fn download_image(url: &str) -> Option<(Vec<u8>, String)> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    // curl handles tls and redirects; keep it bounded and http(s)-only
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
    // only accept it if what came back really is an image
    let ext = image_ext(&out.stdout)?;
    Some((out.stdout, ext))
}

#[cfg(target_os = "macos")]
mod imp {
    use std::io;
    use std::path::Path;
    use std::process::Command;

    pub fn paste() -> io::Result<(Vec<u8>, String)> {
        // a file copied in Finder shows up as a file url
        if let Some(path) = clipboard_file_path() {
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
        // fall back to image data
        if let Some(bytes) = clipboard_image() {
            return Ok((bytes, "png".to_string()));
        }
        // a copied image link
        if let Some(text) = clipboard_text() {
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

    fn clipboard_text() -> Option<String> {
        let out = Command::new("pbpaste").output().ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).into_owned();
        if s.trim().is_empty() { None } else { Some(s) }
    }

    pub fn copy(path: &Path) -> io::Result<()> {
        let abs = std::fs::canonicalize(path)?;
        let script = format!(
            "set the clipboard to (POSIX file \"{}\")",
            abs.to_string_lossy()
        );
        let status = Command::new("osascript").arg("-e").arg(&script).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other("osascript could not set the clipboard"))
        }
    }

    fn clipboard_file_path() -> Option<String> {
        let out = Command::new("osascript")
            .args(["-e", "POSIX path of (the clipboard as «class furl»)"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    }

    fn clipboard_image() -> Option<Vec<u8>> {
        let tmp = std::env::temp_dir().join("fossil-clip-image.png");
        let _ = std::fs::remove_file(&tmp);
        let script = format!(
            "set f to (open for access (POSIX file \"{}\") with write permission)\n\
             write (the clipboard as «class PNGf») to f\n\
             close access f",
            tmp.to_string_lossy()
        );
        let out = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .ok()?;
        if !out.status.success() {
            let _ = std::fs::remove_file(&tmp);
            return None;
        }
        let bytes = std::fs::read(&tmp).ok();
        let _ = std::fs::remove_file(&tmp);
        bytes.filter(|b| !b.is_empty())
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
        // a file copied in a file manager comes through as a file:// uri
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
        // an image
        if let Some(bytes) = get_target("image/png") {
            if !bytes.is_empty() {
                return Ok((bytes, "png".to_string()));
            }
        }
        // a copied image link
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
        // the format gnome-family file managers paste as a file
        let payload = format!("copy\nfile://{}", abs.to_string_lossy());
        set_target("x-special/gnome-copied-files", payload.as_bytes())
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

    fn ps(script: &str) -> io::Result<std::process::Output> {
        Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
    }

    pub fn paste() -> io::Result<(Vec<u8>, String)> {
        // a file copied in Explorer
        let out = ps(
            "$f = Get-Clipboard -Format FileDropList | Select-Object -First 1; if ($f) { $f.FullName }",
        )?;
        if out.status.success() {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path.is_empty() {
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
        // otherwise an image
        let tmp = std::env::temp_dir().join("fossil-clip-image.png");
        let _ = std::fs::remove_file(&tmp);
        let script = format!(
            "$img = Get-Clipboard -Format Image; if ($img) {{ $img.Save('{}') }}",
            tmp.to_string_lossy().replace('\'', "''")
        );
        if ps(&script)?.status.success() {
            if let Ok(bytes) = std::fs::read(&tmp) {
                let _ = std::fs::remove_file(&tmp);
                if !bytes.is_empty() {
                    return Ok((bytes, "png".to_string()));
                }
            }
        }
        // a copied image link
        if let Some(text) = clipboard_text() {
            if let Some(url) = super::first_url(&text) {
                if let Some(res) = super::download_image(url) {
                    return Ok(res);
                }
            }
        }
        let _ = std::fs::remove_file(&tmp);
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "nothing on the clipboard to fossilize (copy a file, image, or image link first)",
        ))
    }

    fn clipboard_text() -> Option<String> {
        let out = ps("Get-Clipboard -Raw").ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).into_owned();
        if s.trim().is_empty() { None } else { Some(s) }
    }

    pub fn copy(path: &Path) -> io::Result<()> {
        let abs = std::fs::canonicalize(path)?;
        let abs = abs.to_string_lossy().into_owned();
        // strip the \\?\ verbatim prefix windows canonicalize adds
        let s = abs.strip_prefix(r"\\?\").unwrap_or(abs.as_str());
        let script = format!("Set-Clipboard -Path '{}'", s.replace('\'', "''"));
        if ps(&script)?.status.success() {
            Ok(())
        } else {
            Err(io::Error::other("powershell could not set the clipboard"))
        }
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
}

pub fn paste() -> io::Result<(Vec<u8>, String)> {
    imp::paste()
}

pub fn copy(path: &Path) -> io::Result<()> {
    imp::copy(path)
}
