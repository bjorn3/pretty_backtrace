use std::collections::HashMap;
use std::sync::Mutex;
use std::path::PathBuf;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Color, Style, ThemeSet};
use syntect::parsing::SyntaxSet;

use self::rent_highlight_cache::*;


lazy_static::lazy_static! {
    static ref SYNTAX_SET: SyntaxSet = SyntaxSet::load_defaults_nonewlines();
    static ref THEME_SET: ThemeSet = ThemeSet::load_defaults();
    static ref CACHED_HIGHLIGHTED_FILES: Mutex<HashMap<PathBuf, Option<HighlightCacheEntry>>> = {
        Mutex::new(HashMap::new())
    };
}


rental! {
    mod rent_highlight_cache {
        use syntect::highlighting::Style;
        #[rental]
        pub struct HighlightCacheEntry {
            string: String,
            highlighted: Vec<Vec<(Style, &'string str)>>,
        }
    }
}

pub fn with_highlighted_source(file: PathBuf, f: impl FnOnce(&[Vec<(Style, &str)>])) {
    CACHED_HIGHLIGHTED_FILES
        .lock()
        .unwrap()
        .entry(file.clone())
        .or_insert_with(|| {
            if let Ok(src) = std::fs::read_to_string(&file) {
                Some(HighlightCacheEntry::new(src, |src| {
                    syntax_highlight(src)
                }))
            } else {
                None
            }
        })
        .as_ref()
        .unwrap()
        .rent(|highlighted| f(highlighted));
}

fn syntax_highlight(src: &str) -> Vec<Vec<(Style, &str)>> {
    let t = &THEME_SET.themes["Solarized (dark)"];
    let mut h = HighlightLines::new(&SYNTAX_SET.find_syntax_by_extension("rs").unwrap().to_owned(), t);

    let mut lines = Vec::new();
    for line in src.lines() {
        lines.push(h.highlight(line, &SYNTAX_SET));
    }
    lines
}

pub fn as_16_bit_terminal_escaped(v: &[(Style, &str)]) -> String {
    use std::fmt::Write;

    let mut s: String = String::new();
    for &(ref style, text) in v.iter() {
        // 256/6 = 42
        write!(
            s,
            "\x1b[38;5;{}m{}",
            16u8 + 36 * (style.foreground.r / 42) + 6 * (style.foreground.g / 42) + (style.foreground.b / 42),
            text
        ).unwrap();
    }
    s.push_str("\x1b[0m");
    s
}
