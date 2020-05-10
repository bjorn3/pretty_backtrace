use findshlibs::Avma;

use crate::{Address, StackFrame, FrameIndex};

struct FrameIterator<'a>(std::iter::Enumerate<std::slice::Iter<'a, backtrace::BacktraceFrame>>);

impl<'a> FrameIterator<'a> {
    fn new(backtrace: &'a backtrace::Backtrace) -> Self {
        Self(backtrace.frames().iter().enumerate())
    }
}

impl<'a> std::iter::Iterator for FrameIterator<'a> {
    type Item = Result<StackFrame, String>;

    fn next(&mut self) -> Option<Result<StackFrame, String>> {
        if let Some((i, stack_frame)) = self.0.next().map(|(i, frame)| (FrameIndex(i), frame)) {
            let addr = if let Some(addr) = Address::from_avma(Avma(stack_frame.ip() as *const u8)) {
                addr
            } else {
                if stack_frame.ip() as usize == 0 {
                    return Some(Err(format!("{} \x1b[2m<end of stack> (0)\x1b[0m", i)));
                } else {
                    return Some(Err(format!("{} \x1b[91m<could not get svma> ({:016p})\x1b[0m", i, stack_frame.ip())));
                }
            };

            // Wait a second each 100 frames to prevent filling the screen in case of a stackoverflow
            if i.0 % 100 == 99 {
                eprintln!("Backtrace is very big, sleeping 1s...");
                std::thread::sleep(std::time::Duration::from_secs(1));
            }

            Some(Ok(StackFrame {
                index: i,
                addr,
            }))
        } else {
            None
        }
    }
}

pub fn print_backtrace() {
    let context = crate::locate_debuginfo::get_context();

    let backtrace = backtrace::Backtrace::new_unresolved();
    let mut stack_state = StackState::PanicStack;
    for stack_frame in FrameIterator::new(&backtrace) {
        let stack_frame = match stack_frame {
            Ok(stack_frame) => stack_frame,
            Err(err) => {
                eprintln!("{:?}", err);
                continue;
            }
        };

        display_frame(&context, &mut stack_state, stack_frame);

        if stack_state == StackState::AfterUserStack {
            break;
        }
    }
    crate::var_guard::print_all();
}

#[derive(Eq, PartialEq)]
pub enum StackState {
    PanicStack,
    UserStack,
    AfterUserStack,
}

fn is_end_of_user_stack(name: &str) -> bool {
    name.ends_with("__rust_begin_short_backtrace") || name.starts_with("std::rt::lang_start")
}


pub(crate) fn display_frame(context: &crate::Context, stack_state: &mut StackState, stack_frame: StackFrame) {
    let mut iter = context.addr2line.find_frames(stack_frame.addr.svma.0 as u64).unwrap();
    let mut first_frame_of_function = true;
    while let Some(frame) = iter.next().unwrap() {
        first_frame_of_function = false;

        let function_name = frame.function.as_ref().map(|n| n.demangle().unwrap()).unwrap_or("<??>".into());

        /*if function_name == "core::panicking::panic" || function_name.starts_with("std::panicking::begin_panic") {
            *stack_state = StackState::UserStack;
            println!("      \x1b[96mseveral panic frames hidden\x1b[0m");
        }

        if *stack_state == StackState::PanicStack {
            continue;
        }*/

        crate::display_frame::display_subframe(context, &stack_frame, Some(&frame), first_frame_of_function, function_name.as_ref(), false);

        if is_end_of_user_stack(&function_name) {
            *stack_state = StackState::AfterUserStack;
            println!("      \x1b[96mseveral runtime init frames hidden\x1b[0m");
            return;
        }
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
