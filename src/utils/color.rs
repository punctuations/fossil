use std::env;
use std::io::{self, IsTerminal};

pub fn should_color() -> bool {
    if env::var_os("NO_COLOR").is_some() {
        return false;
    }

    if env::var("CLICOLOR_FORCE").ok().as_deref() == Some("1") {
        return true;
    }

    if env::var("TERM").ok().as_deref() == Some("dumb") {
        return false;
    }

    return io::stdout().is_terminal();
}

pub fn paint(s: &str, code: &str) -> String {
    if should_color() {
        return format!("\x1b[{code}m{s}\x1b[0m");
    } else {
        return s.to_string();
    }
}

fn hyperlinks_supported() -> bool {
    if env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if !io::stderr().is_terminal() {
        return false;
    }

    let term_program = env::var("TERM_PROGRAM").unwrap_or_default();
    if term_program == "Apple_Terminal" {
        // Terminal.app still dumb as hell
        return false;
    }

    if env::var_os("WT_SESSION").is_some() || env::var_os("KITTY_WINDOW_ID").is_some() {
        return true;
    }
    if env::var("TERM").unwrap_or_default() == "xterm-kitty" {
        return true;
    }
    if matches!(
        term_program.as_str(),
        "iTerm.app" | "WezTerm" | "vscode" | "Hyper" | "ghostty" | "rio"
    ) {
        return true;
    }
    if let Ok(v) = env::var("VTE_VERSION") {
        if v.parse::<u32>().map(|n| n >= 5000).unwrap_or(false) {
            return true;
        }
    }

    return false;
}

pub fn link(label: &str, url: &str) -> String {
    if hyperlinks_supported() {
        return format!("\x1b]8;;{url}\x1b\\{label}\x1b]8;;\x1b\\");
    } else {
        return url.to_string();
    }
}

// yea basically just colorize package
pub trait Color {
    fn accent(&self) -> String;
    fn coral(&self) -> String;
    fn header(&self) -> String;
    fn dim(&self) -> String;

    fn bold(&self) -> String;

    fn red(&self) -> String;
    fn lred(&self) -> String;
    fn blue(&self) -> String;
    fn cyan(&self) -> String;
    fn reset(&self) -> String;
}

impl Color for str {
    fn accent(&self) -> String {
        return paint(self, "38;5;180");
    }

    fn coral(&self) -> String {
        return paint(self, "38;5;173");
    }

    fn header(&self) -> String {
        return paint(self, "38;5;187");
    }

    fn dim(&self) -> String {
        return paint(self, "38;5;244");
    }

    fn bold(&self) -> String {
        return paint(self, "1");
    }

    fn red(&self) -> String {
        return paint(self, "31");
    }

    fn lred(&self) -> String {
        return paint(self, "91");
    }

    fn blue(&self) -> String {
        return paint(self, "34");
    }

    fn cyan(&self) -> String {
        return paint(self, "36");
    }

    fn reset(&self) -> String {
        if should_color() {
            return format!("{}\x1b[0m", self);
        } else {
            return self.to_string();
        }
    }
}

impl Color for String {
    fn accent(&self) -> String {
        return self.as_str().accent();
    }

    fn coral(&self) -> String {
        return self.as_str().coral();
    }

    fn header(&self) -> String {
        return self.as_str().header();
    }

    fn dim(&self) -> String {
        return self.as_str().dim();
    }

    fn bold(&self) -> String {
        return self.as_str().bold();
    }

    fn red(&self) -> String {
        return self.as_str().red();
    }

    fn lred(&self) -> String {
        return self.as_str().lred();
    }

    fn blue(&self) -> String {
        return self.as_str().blue();
    }

    fn cyan(&self) -> String {
        return self.as_str().cyan();
    }

    fn reset(&self) -> String {
        return self.as_str().reset();
    }
}
