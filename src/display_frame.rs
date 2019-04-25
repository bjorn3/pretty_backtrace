use std::path::PathBuf;

use crate::Frame;

pub(crate) fn display_frame(context: &crate::Context, stack_frame: Frame) {
    let mut iter = context.addr2line.find_frames(stack_frame.addr.svma.0 as u64).unwrap();
    let mut first_frame = true;
    while let Some(frame) = iter.next().unwrap() {
        let function_name = frame.function.map(|n|n.demangle().unwrap().to_string()).unwrap_or("<??>".to_string());

        if first_frame {
            write_frame_line(&stack_frame, &function_name, false);
        } else {
            eprintln!("      {}", function_name);
        }

        let show_source = !function_name.starts_with("pretty_backtrace::");

        print_location(frame.location, show_source);

        first_frame = false;
    }

    if first_frame == true {
        // No debug info
        backtrace::resolve(stack_frame.addr.avma.0 as *mut _, |symbol| {
            if let Some(symbol_name) = symbol.name() {
                let mangled_name = symbol_name.as_str().unwrap();
                let name = addr2line::demangle_auto(mangled_name.into(), None);
                write_frame_line(&stack_frame, &name, false);
            } else {
                write_frame_line(&stack_frame, "<unknown function name>", true);
            }
        });
    }

    print_values(context, &stack_frame);
}

fn write_frame_line(frame: &Frame, function_name: &str, err: bool) {
    eprintln!(
        "{} {}{:<80}\x1b[0m  \x1b[2m({})\x1b[0m",
        frame.index,
        if err { "\x1b[91m" } else { "" },
        function_name,
        frame.addr,
    );
}

lazy_static::lazy_static! {
    static ref RUST_SOURCE: regex::Regex = regex::Regex::new("/rustc/\\w+/").unwrap();
    static ref STD_SRC: Option<String> = {
        if let Ok(output) = std::process::Command::new("rustc").arg("--print").arg("sysroot").output() {
            if let Ok(sysroot) = String::from_utf8(output.stdout) {
                Some(sysroot.trim().to_string() + "/lib/rustlib/src/rust")
            } else {
                None
            }
        } else {
            None
        }
    };
}

fn print_location(location: Option<addr2line::Location>, mut show_source: bool) {
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

    if file.starts_with("<rust>") {
        show_source = false;
    }

    let file = if let Some(std_src) = &*STD_SRC {
        file.replace("<rust>", std_src)
    } else {
        file
    };

    if show_source {
        if let Some(line) = location.line {
            crate::syntax_highlight::with_highlighted_source(PathBuf::from(file.clone()), move |highlighted| {
                let highlighted = if let Some(highlighted) = highlighted {
                    highlighted
                } else {
                    eprintln!("          \x1b[91m<file not found>\x1b[0m");
                    return;
                };

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

type Slice = gimli::EndianRcSlice<gimli::RunTimeEndian>;

fn print_values(context: &crate::Context, frame: &Frame) {
    use gimli::read::Reader;
    let unit = if let Some(unit) = find_unit_for_svma(&context.dwarf, frame.addr.svma) {
        unit
    } else {
        return;
    };
    find_die_for_svma(&context.dwarf, &unit, frame.addr.svma, |entry| {
        let mut entries_tree = unit.entries_tree(Some(entry.offset())).unwrap();
        process_tree(&context.dwarf, &unit, frame, entries_tree.root().unwrap(), 0);

        fn process_tree(dwarf: &gimli::Dwarf<Slice>, unit: &gimli::Unit<Slice>, frame: &Frame, mut node: gimli::EntriesTreeNode<Slice>, indent: usize) {
            {
                let entry = node.entry();
                println!("{:indent$}{:?}", "", entry.tag().static_string(), indent = indent);

                if entry.tag() == gimli::DW_TAG_formal_parameter || entry.tag() == gimli::DW_TAG_variable {
                    let name = if let Some(name) = entry.attr(gimli::DW_AT_name).unwrap() {
                        name.string_value(&dwarf.debug_str).unwrap().to_string().unwrap().into_owned()
                    } else {
                        "<unknown name>".to_string()
                    };
                    println!("{:indent$}name: {}", "", name, indent = indent);

                    /*let ty = if let Some(ty) = entry.attr(gimli::DW_AT_type).unwrap() {
                        match ty.value() {
                            gimli::AttributeValue::DebugTypesRef(type_sig) => {} // TODO
                            _ => panic!("{:?}", ty.value()),
                        };
                        ty.string_value(&dwarf.debug_str).unwrap().to_string().unwrap().into_owned()
                    } else {
                        "<unknown type>".to_string()
                    };
                    println!("{:indent$}type: {}", "", ty, indent = indent);*/

                    let exprloc = if let Some(exprloc) = entry.attr(gimli::DW_AT_location).unwrap() {
                        Some(match exprloc.value() {
                            gimli::AttributeValue::Block(data) => gimli::Expression(data),
                            gimli::AttributeValue::Exprloc(exprloc) => exprloc,
                            gimli::AttributeValue::LocationListsRef(loclistref) => {
                                //dwarf.locations(unit, loclistref);
                                println!("warning: unhandled location list");
                                return;
                            },
                            _ => panic!("{:?}", exprloc.value()),
                        })
                    } else {
                        None
                    };

                    if let Some(exprloc) = exprloc {
                        let mut eval = exprloc.clone().evaluation(unit.encoding());
                        let mut res = eval.evaluate().unwrap();
                        loop {
                            println!("{:indent$}eval: {:?}", "", res, indent = indent);
                            match res {
                                gimli::EvaluationResult::Complete => {
                                    let result = eval.result();
                                    println!("{:indent$}eval res: {:?}", "", result, indent = indent);
                                    for piece in result {
                                        use gimli::read::Location::*;
                                        match piece.location {
                                            Empty => println!("{:indent$}piece: empty", "", indent = indent),
                                            Register { register } => {
                                                println!("{:indent$}piece: register={:?}", "", register, indent = indent);
                                            }
                                            Address { address } => {
                                                println!("{:indent$}piece: address={:?}", "", address, indent = indent);
                                            }
                                            Value { value } => {
                                                println!("{:indent$}piece: value={:?}", "", value, indent = indent);
                                            }
                                            Bytes { value } => {
                                                println!("{:indent$}piece: bytes={:?}", "", value, indent = indent);
                                            }
                                            ImplicitPointer { value, byte_offset } => {
                                                println!("{:indent$}piece: implicitptr={:?}+{:?}", "", value, byte_offset, indent = indent);
                                            }
                                        }
                                    }
                                    break;
                                }
                                // FIXME use DW_AT_frame_base for register
                                gimli::EvaluationResult::RequiresFrameBase => res = eval.resume_with_frame_base(frame.regs[gimli::X86_64::RSP.0].unwrap()).unwrap(),
                                _ => break,
                            }
                        }
                    }

                    let mut attrs = entry.attrs();
                    while let Some(attr) = attrs.next().unwrap() {
                        println!("{:indent$}attr {:?} = ???", "", attr.name().static_string(), indent = indent);
                        //println!("Attribute value = {:?}", attr.value());
                    }
                }
            }
            let mut children = node.children();
            while let Some(child) = children.next().unwrap() {
                // Recursively process a child.
                process_tree(dwarf, unit, frame, child, indent + 4);
            }
        }
    }).unwrap();
}

fn find_unit_for_svma(dwarf: &gimli::Dwarf<Slice>, svma: findshlibs::Svma) -> Option<gimli::read::Unit<Slice>> {
    let mut units = dwarf.units();
    while let Some(unit) = units.next().unwrap() {
        let unit = gimli::read::Unit::new(&dwarf, unit).unwrap();
        let mut ranges = dwarf.unit_ranges(&unit).unwrap();
        while let Some(range) = ranges.next().unwrap() {
            if range.begin <= svma.0 as u64 && range.end > svma.0 as u64 {
                return Some(unit);
            }
        }
    }
    None
}

fn find_die_for_svma<'dwarf, 'unit: 'dwarf, T, F: FnMut(gimli::read::DebuggingInformationEntry<'dwarf, 'unit, Slice>) -> T>(dwarf: &'dwarf gimli::Dwarf<Slice>, unit: &'unit gimli::Unit<Slice>, svma: findshlibs::Svma, mut f: F) -> Option<T> {
    fn process_tree<'dwarf, 'unit: 'dwarf, T, F: FnMut(gimli::DebuggingInformationEntry<'dwarf, 'unit, Slice>) -> T>(dwarf: &gimli::Dwarf<Slice>, unit: &gimli::Unit<Slice>, node: gimli::EntriesTreeNode<'dwarf, 'unit, '_, Slice>, svma: findshlibs::Svma, f: &mut F) -> Option<T> {
        let entry = node.entry().clone();
        let mut children = node.children();
        while let Some(child) = children.next().unwrap() {
            // Recursively process a child.
            if let Some(val) = process_tree(dwarf, unit, child, svma, f) {
                return Some(val);
            }
        }

        let mut ranges = dwarf.die_ranges(unit, &entry).unwrap();
        while let Some(range) = ranges.next().unwrap() {
            if range.begin <= svma.0 as u64 && range.end > svma.0 as u64 {
                return Some(f(entry));
            }
        }

        None
    }

    let mut entries_tree = unit.entries_tree(None).unwrap();
    process_tree(&dwarf, &unit, entries_tree.root().unwrap(), svma, &mut f)
}
