#[macro_use]
extern crate rental;

mod display_frame;
mod syntax_highlight;
mod locate_debuginfo;

use std::fmt;
use std::panic::{PanicInfo, set_hook, take_hook};
use std::path::PathBuf;

use findshlibs::{Avma, Svma, SharedLibrary, Segment};

lazy_static::lazy_static! {
    static ref HOOK: Box<for<'a> Fn(&'a PanicInfo) + Sync + Send + 'static> = {
        let prev = take_hook();
        set_hook(Box::new(the_hook));
        prev
    };
}

pub fn setup() {
    if !findshlibs::TARGET_SUPPORTED {
        eprintln!("findshlibs doesn't support your platform, using default panic hook");
    } else {
        lazy_static::initialize(&HOOK);
    }
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

    let context = locate_debuginfo::get_context();
    let backtrace = backtrace::Backtrace::new_unresolved();
    eprintln!("stack backtrace:");
    for (i, stack_frame) in backtrace.frames().iter().enumerate().map(|(i, frame)| (FrameIndex(i), frame)) {
        let addr = if let Some(addr) = Address::from_avma(Avma(stack_frame.ip() as *const u8)) {
            addr
        } else {
            eprintln!("{} \x1b[91m<could not get svma> ({:p})\x1b[0m", i, stack_frame.ip());
            continue;
        };

        display_frame::display_frame(&context, i, addr);
    }

    eprintln!();
    (*HOOK)(info);
}

#[derive(Copy, Clone)]
struct FrameIndex(usize);

impl fmt::Display for FrameIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\x1b[2m{:>4}:\x1b[0m", self.0)
    }
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
                    res = Some(Address {
                        avma,
                        svma,
                        lib_file: PathBuf::from(shlib.name().to_string_lossy().into_owned()),
                    });
                }
            }
        });

        res
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:016p} = {:016p}@{}", self.svma.0, self.avma.0, self.lib_file.file_name().unwrap().to_string_lossy())
    }
}
