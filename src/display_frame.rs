use std::path::PathBuf;

use gimli::Endianity;

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
                    if line_num as u64 == line && location.column.is_some() {
                        eprintln!("         | {:width$}\x1b[91m^\x1b[0m", " ", width=location.column.unwrap() as usize);
                    }
                }
            });
        }
    }
}

type Slice = gimli::EndianRcSlice<gimli::RunTimeEndian>;

fn print_values(context: &crate::Context, frame: &Frame) {
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
                    print_local(dwarf, unit, frame, indent, entry);
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

fn print_local(
    dwarf: &gimli::Dwarf<Slice>,
    unit: &gimli::Unit<Slice>,
    frame: &Frame,
    indent: usize,
    entry: &gimli::DebuggingInformationEntry<Slice>,
) {
    let local_name = entry_name(dwarf, entry);
    println!("{:indent$}name: {}", "", local_name, indent = indent);

    let mut attrs = entry.attrs();
    while let Some(attr) = attrs.next().unwrap() {
        //println!("{:indent$}attr {:?} = ???", "", attr.name().static_string(), indent = indent);
        //println!("Attribute value = {:?}", attr.value());
    }

    let ty_entry = if let Some(ty_entry) = entry_type_entry(unit, entry) {
        ty_entry
    } else {
        println!("warning: missing type for local {}", local_name);
        return;
    };

    let byte_size = ty_entry.attr(gimli::DW_AT_byte_size).unwrap().map(|val| val.udata_value().unwrap() as usize).unwrap_or_else(|| {
        if ty_entry.tag() == gimli::DW_TAG_pointer_type {
            std::mem::size_of::<usize>()
        } else {
            panic!("type die with tag {:?} doesn't have DW_AT_byte_size", ty_entry.tag().static_string());
        }
    });

    let exprloc = if let Some(exprloc) = entry.attr(gimli::DW_AT_location).unwrap() {
        match exprloc.value() {
            gimli::AttributeValue::Block(data) => gimli::Expression(data),
            gimli::AttributeValue::Exprloc(exprloc) => exprloc,
            gimli::AttributeValue::LocationListsRef(_) => {
                println!("warning: location lists are not yet supported");
                return;
            },
            _ => panic!("{:?}", exprloc.value()),
        }
    } else {
        println!("{} = <unknown>", local_name);
        return;
    };

    match binary_data_for_expression(frame, unit, exprloc, byte_size, indent) {
        Ok(ref bytes) => { // use ref here to prevent accidential mutation
            let ty_name = entry_name(dwarf, &ty_entry);
            let val = pretty_print_value(dwarf, unit, &ty_entry, bytes, indent)
                .unwrap_or_else(|| "<unknown>".to_string());
            println!("{:<88}raw: {:?}", format!("{}: {} = {}", local_name, ty_name, val), bytes);
        }
        Err(res) => {
            println!("{:indent$}eval for {}: {:?}", "", local_name, res, indent = indent);
            return;
        }
    }

    println!();
}

fn pretty_print_value(dwarf: &gimli::Dwarf<Slice>, unit: &gimli::Unit<Slice>, ty_entry: &gimli::DebuggingInformationEntry<Slice>, bytes: &[u8], indent: usize) -> Option<String> {
    match ty_entry.tag() {
        gimli::DW_TAG_base_type => {
            let encoding = match ty_entry.attr(gimli::DW_AT_encoding).unwrap().unwrap().value() {
                gimli::AttributeValue::Encoding(encoding) => encoding,
                val => panic!("{:?}", val),
            };

            Some(match encoding {
                gimli::DW_ATE_signed => format!("{}", read_to_i64(bytes)),
                gimli::DW_ATE_unsigned => format!("{}", read_to_u64(bytes)),
                _ => {
                    println!("warning: unknown base type encoding {:?}", encoding.static_string());
                    return None;
                }
            })
        }
        gimli::DW_TAG_pointer_type => {
            Some(format!("{:0ptrsize$p}", read_to_u64(bytes) as *const u8, ptrsize = bytes.len()))
        }
        _ => {
            fn process_tree<'dwarf, 'unit: 'dwarf>(dwarf: &gimli::Dwarf<Slice>, unit: &gimli::Unit<Slice>, node: gimli::EntriesTreeNode<'dwarf, 'unit, '_, Slice>, indent: usize) {
                println!("{:indent$}tag: {}", "", node.entry().tag().static_string().unwrap(), indent = indent);
                println!("{:indent$}name: {}", "", entry_name(dwarf, node.entry()), indent = indent + 4);

                let mut attrs = node.entry().attrs();
                while let Some(attr) = attrs.next().unwrap() {
                    println!("{:indent$}attr {:?} = {:?}", "", attr.name().static_string(), attr.value(), indent = indent + 4);
                }

                let mut children = node.children();
                while let Some(child) = children.next().unwrap() {
                    // Recursively process a child.
                    process_tree(dwarf, unit, child, indent + 2);
                }
            }

            let mut entries_tree = unit.entries_tree(Some(ty_entry.offset())).unwrap();
            process_tree(&dwarf, &unit, entries_tree.root().unwrap(), indent + 4);

            None
        }
    }
}

fn read_to_u64(bytes: &[u8]) -> u64 {
    match bytes.len() {
        1 => bytes[0] as u64,
        2 => gimli::NativeEndian::default().read_u16(bytes) as u64,
        4 => gimli::NativeEndian::default().read_u32(bytes) as u64,
        8 => gimli::NativeEndian::default().read_u64(bytes) as u64,
        _ => {
            panic!("{:?}", bytes);
        }
    }
}

fn read_to_i64(bytes: &[u8]) -> i64 {
    match bytes.len() {
        1 => bytes[0] as i8 as i64,
        2 => gimli::NativeEndian::default().read_i16(bytes) as i64,
        4 => gimli::NativeEndian::default().read_i32(bytes) as i64,
        8 => gimli::NativeEndian::default().read_i64(bytes) as i64,
        _ => {
            panic!();
        }
    }
}

fn entry_name(dwarf: &gimli::Dwarf<Slice>, entry: &gimli::DebuggingInformationEntry<Slice>) -> String {
    use gimli::read::Reader;

    if let Some(name) = entry.attr(gimli::DW_AT_name).unwrap() {
        name.string_value(&dwarf.debug_str).unwrap().to_string().unwrap().into_owned()
    } else {
        "<unknown name>".to_string()
    }
}

fn entry_type_entry<'dwarf, 'unit: 'dwarf>(unit: &'unit gimli::Unit<Slice>, entry: &gimli::DebuggingInformationEntry<Slice>) -> Option<gimli::DebuggingInformationEntry<'dwarf, 'unit, Slice>> {
    if let Some(ty) = entry.attr(gimli::DW_AT_type).unwrap() {
        let ty_offset = match ty.value() {
            gimli::AttributeValue::UnitRef(unit_offset) => unit_offset,
            _ => panic!("{:?}", ty.value()),
        };

        let mut entries = unit.entries_at_offset(ty_offset).expect("entry");
        entries.next_entry().unwrap().unwrap();
        Some(entries.current().expect("current").clone())
    } else {
        None
    }
}

fn evaluate_expression(
    frame: &Frame,
    mut eval: gimli::Evaluation<Slice>,
) -> Result<Vec<gimli::Piece<Slice>>, gimli::EvaluationResult<Slice>> {
    let mut res = eval.evaluate().unwrap();
    loop {
        match res {
            gimli::EvaluationResult::Complete => {
                return Ok(eval.result());
            }
            // FIXME use DW_AT_frame_base for register
            gimli::EvaluationResult::RequiresFrameBase => {
                res = eval.resume_with_frame_base(frame.regs[gimli::X86_64::RSP.0].unwrap()).unwrap();
            }
            gimli::EvaluationResult::RequiresMemory {
                address,
                size,
                space: None,
                base_type: _,
            } => {
                let value = unsafe { std::slice::from_raw_parts(address as *const u8, size as usize) }.to_vec();
                res = eval.resume_with_memory(gimli::Value::Generic(read_to_u64(&value))).unwrap();
            }
            err => {
                return Err(err)
            }
        }
    }
}

fn binary_data_for_expression(
    frame: &Frame,
    unit: &gimli::Unit<Slice>,
    exprloc: gimli::Expression<Slice>,
    byte_size: usize,
    indent: usize,
) -> Result<Vec<u8>, gimli::EvaluationResult<Slice>> {
    let eval = exprloc.clone().evaluation(unit.encoding());
    evaluate_expression(frame, eval).map(|result| {
        assert!(result.len() == 1, "eval result: {:?}", result);
        let piece = result.into_iter().next().unwrap();
        use gimli::read::Location::*;

        assert!(piece.size_in_bits.is_none(), "{:?}", piece);
        assert!(piece.bit_offset.is_none(), "{:?}", piece);

        match piece.location {
            Empty => {
                println!("{:indent$}piece: empty", "", indent = indent);
                Vec::new()
            }
            Register { register } => {
                panic!("{:indent$}piece: register={:?}", "", register, indent = indent);
            }
            Address { address } => {
                println!("{:indent$}piece: address={:016p}", "", address as *const u8, indent = indent);
                unsafe { std::slice::from_raw_parts(address as *const u8, byte_size as usize) }.to_vec()
            }
            Value { value } => {
                panic!("{:indent$}piece: value={:?}", "", value, indent = indent);
            }
            Bytes { value } => {
                panic!("{:indent$}piece: bytes={:?}", "", value, indent = indent);
            }
            ImplicitPointer { value, byte_offset } => {
                panic!("{:indent$}piece: implicitptr={:?}+{:?}", "", value, byte_offset, indent = indent);
            }
        }
    })
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
