use std::path::PathBuf;

use crate::{Address, FrameIndex};

pub(crate) fn display_frame(context: &addr2line::Context, i: FrameIndex, addr: Address) {
    let mut iter = context.find_frames(addr.svma.0 as u64).unwrap();
    let mut first_frame = true;
    while let Some(frame) = iter.next().unwrap() {
        let function_name = frame.function.map(|n|n.demangle().unwrap().to_string()).unwrap_or("<??>".to_string());

        if first_frame {
            write_frame_line(i, &function_name, &addr, false);
        } else {
            eprintln!("      {}", function_name);
        }

        print_location(frame.location);

        first_frame = false;
    }

    if first_frame == true {
        // No debug info
        write_frame_line(i, "<missing debuginfo>", &addr, true);
    }

    // Wait a second each 100 frames to prevent filling the screen in case of a stackoverflow
    if i.0 % 100 == 99 {
        eprintln!("Backtrace is very big, sleeping 1s...");
        ::std::thread::sleep_ms(1000);
    }
}

fn write_frame_line(i: FrameIndex, function_name: &str, addr: &Address, err: bool) {
    eprintln!(
        "{} {}{:<80}\x1b[0m  \x1b[2m({})\x1b[0m",
        i,
        if err { "\x1b[91m" } else { "" },
        function_name,
        addr,
    );
}

lazy_static::lazy_static! {
    static ref RUST_SOURCE: regex::Regex = regex::Regex::new("/rustc/\\w+/").unwrap();
}

fn print_location(location: Option<addr2line::Location>) {
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

    if !file.starts_with("<") {
        if let Some(line) = location.line {
            crate::syntax_highlight::with_highlighted_source(PathBuf::from(file.clone()), move |highlighted| {
                for (line_num, line_str) in highlighted.iter().enumerate().map(|(line_num, line_str)|(line_num as u64 + 1, line_str)) {
                    if line_num < line - 2 || line_num > line + 2 {
                        continue;
                    }

                    let line_marker = if line_num as u64 == line { "\x1b[91m>\x1b[0m" } else { " \x1b[2m" };
                    eprintln!("{} {:>6} | {}", line_marker, line_num, crate::syntax_highlight::as_16_bit_terminal_escaped(line_str));
                    if line_num as u64 == line {
                        eprintln!("         | {:width$}\x1b[91m^\x1b[0m", " ", width=location.column.unwrap() as usize);
                    }
                }
            });
        }
    }
}
