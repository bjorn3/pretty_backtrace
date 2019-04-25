use findshlibs::Avma;

use crate::{Address, Frame, FrameIndex};

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

        crate::display_frame::display_frame(&context, Frame {
            index: i,
            addr,
        });

        // Wait a second each 100 frames to prevent filling the screen in case of a stackoverflow
        if i.0 % 100 == 99 {
            eprintln!("Backtrace is very big, sleeping 1s...");
            std::thread::sleep_ms(1000);
        }
    }
}
