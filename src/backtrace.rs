use findshlibs::Avma;

use crate::{Address, Frame, FrameIndex};

#[cfg(feature = "unwinder_backtrace")]
pub fn print_backtrace() {
    let context = crate::locate_debuginfo::get_context();

    let backtrace = backtrace::Backtrace::new_unresolved();
    for (i, stack_frame) in backtrace.frames().iter().enumerate().map(|(i, frame)| (FrameIndex(i), frame)) {
        let addr = if let Some(addr) = ip_to_address(stack_frame.ip() as *const u8, i) {
            addr
        } else {
            continue;
        };

        crate::display_frame::display_frame(&context, Frame {
            index: i,
            addr,
        });

        stackoverflow_wait(i);
    }
}

#[cfg(feature = "unwinder_unwind_rs")]
pub fn print_backtrace() {
    use unwind::Unwinder;
    use fallible_iterator::FallibleIterator;

    let context = crate::locate_debuginfo::get_context();

    unwind::DwarfUnwinder::default().trace(|x| {
        let mut i = FrameIndex(0);
        while let Some(_frame) = x.next().unwrap() {
            let ip = x.registers()[16].unwrap();

            let addr = if let Some(addr) = ip_to_address(ip as *const u8, i) {
                addr
            } else {
                continue;
            };

            crate::display_frame::display_frame(&context, Frame {
                index: i,
                addr,
                regs: x.registers().clone(),
            });

            stackoverflow_wait(i);

            i.0 += 1;
        }
    });
}

fn ip_to_address(ip: *const u8, i: FrameIndex) -> Option<Address> {
    if let Some(addr) = Address::from_avma(Avma(ip)) {
        Some(addr)
    } else {
        if ip as usize == 0 {
            eprintln!("{} \x1b[2m<end of stack> (0)\x1b[0m", i);
        } else {
            eprintln!("{} \x1b[91m<could not get svma> ({:016p})\x1b[0m", i, ip);
        }

        None
    }
}

fn stackoverflow_wait(i: FrameIndex) {
    // Wait a second each 100 frames to prevent filling the screen in case of a stackoverflow
    if i.0 % 100 == 99 {
        eprintln!("Backtrace is very big, sleeping 1s...");
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
