use std::path::PathBuf;

use crate::{Context, StackFrame, SubFrame};

pub(crate) fn display_subframe(
    context: &Context,
    stack_frame: &StackFrame,
    addr2line_frame: Option<&addr2line::Frame<'_, crate::dwarf::Slice>>,
    first_frame: bool,
    name: &str,
    err: bool,
) {
    if first_frame {
        eprintln!(
            "{} {}{:<80}\x1b[0m  \x1b[2m({})\x1b[0m",
            stack_frame.index,
            if err { "\x1b[91m" } else { "" },
            name,
            stack_frame.addr,
        );
    } else {
        eprintln!("      {}", name);
    }

    if let Some(addr2line_frame) = addr2line_frame {
        let frame = SubFrame {
            stack_frame,
            addr2line_frame,
        };

        let show_source =
            !name.starts_with("pretty_backtrace::")
            && !name.starts_with("std::panic")
            && !name.starts_with("std::rt");
        print_location(frame.addr2line_frame.location.as_ref(), show_source);

        crate::var_guard::print_values(context, &frame);
    }
}

lazy_static::lazy_static! {
    static ref RUST_SOURCE: regex::Regex = regex::Regex::new("/rustc/\\w+/").unwrap();
    static ref STD_SRC: Option<String> = {
        if let Ok(output) = std::process::Command::new("rustc").arg("--print").arg("sysroot").output() {
            if let Ok(sysroot) = String::from_utf8(output.stdout) {
                Some(sysroot.trim().to_string() + "/lib/rustlib/src/rust")
            } else {
                None
            }
        } else {
            None
        }
    };
}

fn print_location(location: Option<&addr2line::Location>, mut show_source: bool) {
    let location = if let Some(location) = location {
        location
    } else {
        eprintln!("             at <no debuginfo>");
        return;
    };

    let file = if let Some(file) = &location.file {
        RUST_SOURCE.replace(file, "<rust>/").to_string()
    } else {
        "<???>".to_string()
    };

    match (location.line, location.column) {
        (Some(line), Some(column)) => eprintln!("      --> {}:{}:{}", file, line, column),
        (Some(line), None) => eprintln!("      --> {}:{}", file, line),
        (None, _) => eprintln!("      --> {}", file),
    }

    if file.starts_with("<rust>") {
        show_source = false;
    }

    let file = if let Some(std_src) = &*STD_SRC {
        file.replace("<rust>", std_src)
    } else {
        file
    };

    if !show_source {
        return;
    }

    if let Some(line) = location.line {
        crate::syntax_highlight::with_highlighted_source(PathBuf::from(file.clone()), move |highlighted| {
            let highlighted = if let Some(highlighted) = highlighted {
                highlighted
            } else {
                eprintln!("          \x1b[91m<file not found>\x1b[0m");
                return;
            };

            for (line_num, line_str) in highlighted.iter().enumerate().map(|(line_num, line_str)|(line_num as u32 + 1, line_str)) {
                if line_num < line - 2 || line_num > line + 2 {
                    continue;
                }

                let line_marker = if line_num == line { "\x1b[91m>\x1b[0m" } else { " \x1b[2m" };
                eprintln!("{} {:>6} | {}", line_marker, line_num, crate::syntax_highlight::as_16_bit_terminal_escaped(line_str, line_num != line));
                if line_num == line && location.column.is_some() {
                    eprintln!("         | {:width$}\x1b[91m^\x1b[0m", " ", width=location.column.unwrap() as usize - 1);
                }
            }
        });
    }
}
