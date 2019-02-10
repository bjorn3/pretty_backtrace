#[macro_use]
extern crate rental;

use std::panic::{PanicInfo, set_hook, take_hook};
use std::path::PathBuf;

mod syntax_highlight;

lazy_static::lazy_static! {
    static ref HOOK: Box<for<'a> Fn(&'a PanicInfo) + Sync + Send + 'static> = {
        let prev = take_hook();
        set_hook(Box::new(the_hook));
        prev
    };
}

pub fn setup() {
    lazy_static::initialize(&HOOK);
}

fn the_hook(info: &PanicInfo) {
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("<unnamed>");
    let msg = match info.payload().downcast_ref::<&'static str>() {
        Some(s) => *s,
        None => match info.payload().downcast_ref::<String>() {
            Some(s) => &s[..],
            None => "Box<Any>",
        }
    };
    let location = info.location().unwrap();
    eprintln!("thread '{}' \x1b[91m\x1b[1mpanicked\x1b[0m at '{}', {}", name, msg, location);
    eprintln!("stack backtrace:");

    with_context(|context| {
        let backtrace = backtrace::Backtrace::new_unresolved();
        for (i, stack_frame) in backtrace.frames().iter().enumerate() {
            let mut iter = context.find_frames(stack_frame.ip() as u64).unwrap();
            let mut first_frame = true;
            while let Some(frame) = iter.next().unwrap() {
                let function_name = frame.function.map(|n|n.demangle().unwrap().to_string()).unwrap_or("<??>".to_string());

                if first_frame {
                    eprintln!("\x1b[2m{:>4}:\x1b[0m {:<80}  \x1b[2m({:p})\x1b[0m", i, function_name, stack_frame.ip());
                } else {
                    eprintln!("      {}", function_name);
                }

                if let Some(location) = frame.location {
                    print_location(location);
                } else {
                    eprintln!("             at <no debuginfo>");
                }
                first_frame = false;
            }

            if i % 100 == 99 {
                eprintln!("Backtrace is very big, sleeping 1s...");
                ::std::thread::sleep_ms(1000);
            }
        }
    });

    eprintln!();
    (*HOOK)(info);
}

fn with_context(f: impl FnOnce(&addr2line::Context)) {
    // Locate .dSYM dwarf debuginfo
    let bin_file_name = std::env::current_exe().expect("current bin");
    let dsym_dir = std::fs::read_dir(bin_file_name.parent().expect("parent"))
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| p.extension() == Some(std::ffi::OsStr::new("dSYM")))
        .next()
        .unwrap();
    let debug_file_name = std::fs::read_dir(dsym_dir.join("Contents/Resources/DWARF"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();

    let debug_file = std::fs::read(debug_file_name).expect("read current bin");
    let debug_file = object::File::parse(&debug_file).expect("parse file");
    let context = addr2line::Context::new(&debug_file).expect("create context");
    f(&context);
}

lazy_static::lazy_static! {
    static ref RUST_SOURCE: regex::Regex = regex::Regex::new("/rustc/\\w+/").unwrap();
}

fn print_location(location: addr2line::Location) {
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
            syntax_highlight::with_highlighted_source(PathBuf::from(file.clone()), move |highlighted| {
                for (line_num, line_str) in highlighted.iter().enumerate().map(|(line_num, line_str)|(line_num as u64 + 1, line_str)) {
                //if let Ok(file_content) = std::fs::read_to_string(&file) {
                //    for (line_num, line_str) in file_content.lines().enumerate().map(|(line_num, line_str)|(line_num as u64 + 1, line_str)) {
                        if line_num < line - 2 || line_num > line + 2 {
                            continue;
                        }

                        let line_marker = if line_num as u64 == line { "\x1b[91m>\x1b[0m" } else { " \x1b[2m" };
                        eprintln!("{} {:>6} | {}", line_marker, line_num, syntax_highlight::as_24_bit_terminal_escaped(line_str, false));
                        if line_num as u64 == line {
                            eprintln!("         | {:width$}\x1b[91m^\x1b[0m", " ", width=location.column.unwrap() as usize);
                        }
                //    }
                }
            });
        }
    }
}
