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

// yea basically just colorize package
pub trait Color {
    fn accent(&self) -> String;
    fn coral(&self) -> String;
    fn header(&self) -> String;

    fn bold(&self) -> String;

    fn red(&self) -> String;
    fn lred(&self) -> String;
    fn green(&self) -> String;
    fn yellow(&self) -> String;
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

    fn bold(&self) -> String {
        return paint(self, "1");
    }

    fn red(&self) -> String {
        return paint(self, "31");
    }

    fn lred(&self) -> String {
        return paint(self, "91");
    }

    fn green(&self) -> String {
        return paint(self, "32");
    }

    fn yellow(&self) -> String {
        return paint(self, "33");
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

    fn bold(&self) -> String {
        return self.as_str().bold();
    }

    fn red(&self) -> String {
        return self.as_str().red();
    }

    fn lred(&self) -> String {
        return self.as_str().lred();
    }

    fn green(&self) -> String {
        return self.as_str().green();
    }

    fn yellow(&self) -> String {
        return self.as_str().yellow();
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