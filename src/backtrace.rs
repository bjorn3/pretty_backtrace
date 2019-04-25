use findshlibs::Avma;

use crate::{Address, Frame, FrameIndex};

pub fn print_backtrace() {
    use unwind::Unwinder;
    use fallible_iterator::FallibleIterator;

    let context = crate::locate_debuginfo::get_context();

    unwind::DwarfUnwinder::default().trace(|x| {
        let mut i = FrameIndex(0);
        while let Some(frame) = x.next().unwrap() {
            let ip = x.registers()[16].unwrap();

            let addr = if let Some(addr) = Address::from_avma(Avma(ip as *const u8)) {
                addr
            } else {
                if ip as usize == 0 {
                    eprintln!("{} \x1b[2m<end of stack> (0)\x1b[0m", i);
                } else {
                    eprintln!("{} \x1b[91m<could not get svma> ({:016p})\x1b[0m", i, ip as *const u8);
                }

                continue;
            };

            crate::display_frame::display_frame(&context, Frame {
                index: i,
                addr,
                regs: x.registers().clone(),
            });

            // Wait a second each 100 frames to prevent filling the screen in case of a stackoverflow
            if i.0 % 100 == 99 {
                eprintln!("Backtrace is very big, sleeping 1s...");
                std::thread::sleep_ms(1000);
            }

            i.0 += 1;
        }
    });
}
