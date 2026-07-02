use std::sync::atomic::{ AtomicBool, Ordering };
use crate::utils::color::Color;
use terminal_size::{ terminal_size, Width };

pub static FAILED: AtomicBool = AtomicBool::new(false);

pub fn subcommand(help_msg: Vec<String>) {
    let art = [
        "      j:¬jI%!:._                  ",
        "   ·%i::iIS?IIi:· .·              ",
        "  ,i::iISISkIi:·/' j:             ",
        " 4?·:iIS$?$Si7,'  j?:.            ",
        " ji:iIS$SISS7j:  .$i:'            ",
        " ?:iS?S$?iI7j?  d?:i.         _.,·",
        ",'·:iI?$bi7j?  jIi:'”    _,pS?:'  ",
        "'b:-:iIIS?j$ªL.ª?:    ,o$S?:'     ",
        " '?:·:iI?:Z' !i?$b%$$?i:'         ",
        "  i%:iS^?iZ  j?:$L?$?:'           ",
        "  '“ 'L jI?b%$?jS$k:7             ",
        "      ?u$Ii$S?IS$?$k:             ",
        "      d$?ª4$$^S7”  °$%px•         ",
        "     j7  ,'4p$    , ?$i7          ",
        "     '“ j7  ?$ ·j'  .$?           ",
        "       d?:   ?Lji.'  $L           ",
        "       '?L:.  ?$ik,ji$            ",
        "         '4k:   $L?$7d$           ",
        "           ?$b  “'°$d$?           ",
        "            '?.:   :$S'           ",
        "              '·.  j7'            ",
    ];

    let gap = 2;

    let art_width = art
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(0);

    let help_width = help_msg
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(0);

    let needed_width = art_width + gap + help_width;

    let term_width = terminal_size()
        .map(|(Width(w), _)| w as usize)
        .unwrap_or(80);

    println!();

    if needed_width > term_width {
        let art_pad = term_width.saturating_sub(art_width) / 2;

        for line in art {
            println!("{}{}", " ".repeat(art_pad), line.accent());
        }

        println!();

        for line in help_msg {
            println!("{line}");
        }

        return;
    }

    let right_top_pad = art.len().saturating_sub(help_msg.len()) / 2;
    let rows = art.len().max(right_top_pad + help_msg.len());

    for i in 0..rows {
        let left_raw = art.get(i).copied().unwrap_or("");

        let right = if i >= right_top_pad {
            help_msg
                .get(i - right_top_pad)
                .map(String::as_str)
                .unwrap_or("")
        } else {
            ""
        };

        let left_padded = format!("{left_raw:<art_width$}");
        let left = left_padded.accent();

        println!("{left}{}{right}", " ".repeat(gap));
    }
}

pub fn had_error() -> bool {
    FAILED.load(Ordering::Relaxed)
}

#[macro_export]
macro_rules! error {
    ($msg:expr) => {
        {
        use crate::utils::color::Color;

        eprintln!("{} {}", "error:".red().bold(), $msg.lred());
        $crate::utils::ui::FAILED.store(true, ::std::sync::atomic::Ordering::Relaxed);
        }
    };

    (
        $fmt:expr,
        $($arg:tt)*
    ) => {
        {
        use crate::utils::color::Color;

        eprintln!("{} {}", "error:".red().bold(), format!($fmt, $($arg)*).lred());
        $crate::utils::ui::FAILED.store(true, ::std::sync::atomic::Ordering::Relaxed);
        }
    };
}

#[macro_export]
macro_rules! info {
    ($msg:expr, usage = true) => {
        {
        use crate::utils::color::Color;

        eprintln!("{} {}", "usage:".blue().bold(), $msg.cyan());
        }
    };

    (
        $fmt:expr,
        $($arg:tt)*,
        usage = true
    ) => {
        {
        use crate::utils::color::Color;

        eprintln!("{} {}", "usage:".red().bold(), format!($fmt, $($arg)*).lred());
        }
    };

    ($msg:expr) => {
        {
        use crate::utils::color::Color;

        eprintln!("{} {}", "info:".blue().bold(), $msg.cyan());
        }
    };

    (
        $fmt:expr,
        $($arg:tt)*
    ) => {
        {
        use crate::utils::color::Color;

        eprintln!("{} {}", "info:".red().bold(), format!($fmt, $($arg)*).lred());
        }
    };
}

#[macro_export]
macro_rules! n {
    () => {
        {
        println!();
        }
    };
}
