use findshlibs::Avma;

use crate::{Address, StackFrame, FrameIndex};

pub fn print_backtrace() {
    let context = crate::locate_debuginfo::get_context();

    let backtrace = backtrace::Backtrace::new_unresolved();
    for (i, stack_frame) in backtrace.frames().iter().enumerate().map(|(i, frame)| (FrameIndex(i), frame)) {
        let addr = if let Some(addr) = Address::from_avma(Avma(stack_frame.ip() as *const u8)) {
            addr
        } else {
            if stack_frame.ip() as usize == 0 {
                eprintln!("{} \x1b[2m<end of stack> (0)\x1b[0m", i);
            } else {
                eprintln!("{} \x1b[91m<could not get svma> ({:016p})\x1b[0m", i, stack_frame.ip());
            }

            continue;
        };

        display_frame(&context, StackFrame {
            index: i,
            addr,
        });

        // Wait a second each 100 frames to prevent filling the screen in case of a stackoverflow
        if i.0 % 100 == 99 {
            eprintln!("Backtrace is very big, sleeping 1s...");
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    crate::var_guard::print_all();
}

pub(crate) fn display_frame(context: &crate::Context, stack_frame: StackFrame) {
    let mut iter = context.addr2line.find_frames(stack_frame.addr.svma.0 as u64).unwrap();
    let mut first_frame_of_function = true;
    while let Some(frame) = iter.next().unwrap() {
        first_frame_of_function = false;

        let function_name = frame.function.as_ref().map(|n| n.demangle().unwrap()).unwrap_or("<??>".into());

        crate::display_frame::display_subframe(context, &stack_frame, Some(&frame), first_frame_of_function, function_name.as_ref(), false);
    }

    if first_frame_of_function {
        // No debug info
        backtrace::resolve(stack_frame.addr.avma.0 as *mut _, |symbol| {
            if let Some(symbol_name) = symbol.name() {
                let mangled_name = symbol_name.as_str().unwrap();
                let name = addr2line::demangle_auto(mangled_name.into(), None);
                crate::display_frame::display_subframe(context, &stack_frame, None, true, name.as_ref(), false);
            } else {
                crate::display_frame::display_subframe(context, &stack_frame, None, true, "<unknown function name>", true);
            }
        });
    }
}
