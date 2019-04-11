use findshlibs::Avma;

use crate::{Address, FrameIndex};

pub fn print_backtrace() {
    let context = crate::locate_debuginfo::get_context();


    /*use fallible_iterator::FallibleIterator;
    use unwind::{DwarfUnwinder, Unwinder};

    DwarfUnwinder::default().trace(|frames| {
        let mut i = 0;
        while let Some(frame) = frames.next().unwrap() {
            let ip = frames.registers()[16].unwrap() as *mut std::os::raw::c_void;
            println!("frame = {:?}", frame);

            let addr = if let Some(addr) = Address::from_avma(Avma(ip as *const u8)) {
                addr
            } else {
                eprintln!("{} \x1b[91m<could not get svma> ({:p})\x1b[0m", i, ip);
                continue;
            };

            crate::display_frame::display_frame(&context, FrameIndex(i), addr);

            i += 1;
        }

        println!("\n\n");
    });*/


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

        crate::display_frame::display_frame(&context, i, addr);
    }
}
