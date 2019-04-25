#[macro_use]
extern crate rental;

mod backtrace;
mod display_frame;
mod syntax_highlight;
mod locate_debuginfo;

use std::cell::Cell;
use std::fmt;
use std::panic::{PanicInfo, set_hook, take_hook};
use std::path::PathBuf;

use findshlibs::{Avma, Svma, SharedLibrary, Segment};

use locate_debuginfo::Context;

lazy_static::lazy_static! {
    static ref HOOK: Box<for<'a> Fn(&'a PanicInfo) + Sync + Send + 'static> = {
        let prev = take_hook();
        set_hook(Box::new(the_hook));
        prev
    };
}

thread_local! {
    static IS_PROCESSING_PANIC: Cell<bool> = Cell::new(false);
}

/// Enable pretty backtraces for the current thread if the `RUST_BACKTRACE` env var is set to `pretty`.
pub fn setup() {
    if let Ok(val) = std::env::var("RUST_BACKTRACE") {
        if val == "pretty" {
            force_setup();
        }
    }
}

/// Always enable pretty backtraces for the current thread.
pub fn force_setup() {
    if !findshlibs::TARGET_SUPPORTED {
        eprintln!("findshlibs doesn't support your platform, using default panic hook");
    } else {
        lazy_static::initialize(&HOOK);
    }
}

fn the_hook(info: &PanicInfo) {
    IS_PROCESSING_PANIC.with(|is_processing_panic| {
        if is_processing_panic.get() {
            println!("\x1b[0m"); // Reset colors
            (*HOOK)(info);
            std::process::abort();
        }
        is_processing_panic.set(true);
    });

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

    crate::backtrace::print_backtrace();

    eprintln!();
    (*HOOK)(info);

    IS_PROCESSING_PANIC.with(|is_processing_panic| is_processing_panic.set(false));
}

#[derive(Copy, Clone)]
struct FrameIndex(usize);

impl fmt::Display for FrameIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\x1b[2m{:>4}:\x1b[0m", self.0)
    }
}

struct Frame {
    index: FrameIndex,
    addr: Address,
    regs: unwind::registers::Registers,
}


#[derive(Clone)]
struct Address {
    avma: Avma,
    svma: Svma,
    lib_file: PathBuf,
}

impl Address {
    fn from_avma(avma: Avma) -> Option<Self> {
        let mut res = None;
        findshlibs::TargetSharedLibrary::each(|shlib| {
            for seg in shlib.segments() {
                if seg.contains_avma(shlib, avma) {
                    let svma = shlib.avma_to_svma(avma);
                    assert!(res.is_none());
                    let lib_file = shlib.name().to_string_lossy().into_owned();
                    let lib_file = if lib_file.is_empty() {
                        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("<current exe>"))
                    } else {
                        PathBuf::from(lib_file)
                    };
                    res = Some(Address {
                        avma,
                        svma,
                        lib_file,
                    });
                }
            }
        });

        res
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let file_name = self.lib_file
            .file_name()
            .map(|s| s.to_string_lossy())
            .unwrap_or(self.lib_file.display().to_string().into());
        write!(f, "{:016p} = {:016p}@{}", self.avma.0, self.svma.0, file_name)
    }
}
