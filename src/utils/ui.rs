use std::sync::atomic::{AtomicBool, Ordering};

pub static FAILED: AtomicBool = AtomicBool::new(false);

pub fn had_error() -> bool {
    FAILED.load(Ordering::Relaxed)
}

#[macro_export]
macro_rules! error {
    ($msg:expr) => {{
        use crate::utils::color::Color;

        eprintln!("{} {}", "error:".red().bold(), $msg.lred());
        $crate::utils::ui::FAILED.store(true, ::std::sync::atomic::Ordering::Relaxed);
    }};

    ($fmt:expr, $($arg:tt)*) => {{
        use crate::utils::color::Color;

        eprintln!("{} {}", "error:".red().bold(), format!($fmt, $($arg)*).lred());
        $crate::utils::ui::FAILED.store(true, ::std::sync::atomic::Ordering::Relaxed);
    }};
}


#[macro_export]
macro_rules! info {
    ($msg:expr, usage=true) => {{
        use crate::utils::color::Color;

        eprintln!("{} {}", "usage:".blue().bold(), $msg.cyan());
    }};

    ($fmt:expr, $($arg:tt)*, usage=true) => {{
        use crate::utils::color::Color;

        eprintln!("{} {}", "usage:".red().bold(), format!($fmt, $($arg)*).lred());
    }};

    ($msg:expr) => {{
        use crate::utils::color::Color;

        eprintln!("{} {}", "info:".blue().bold(), $msg.cyan());
    }};

    ($fmt:expr, $($arg:tt)*) => {{
        use crate::utils::color::Color;

        eprintln!("{} {}", "info:".red().bold(), format!($fmt, $($arg)*).lred());
    }};
}

#[macro_export]
macro_rules! n {
    () => {{
        println!();
    }}
}