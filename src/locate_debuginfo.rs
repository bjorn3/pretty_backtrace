use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use object::Object;

pub struct Context {
    pub addr2line: addr2line::Context<crate::dwarf::Slice>,
    pub dwarf_context: crate::dwarf::DwarfContext,
}

pub fn get_context() -> Context {
    let bin_file_name = std::env::current_exe().expect("current bin");
    get_context_for_file(&bin_file_name)
}

pub fn get_context_for_file(file_name: &Path) -> Context {
    let debug_file = if cfg!(target_os="macos") {
        // Locate .dSYM dwarf debuginfo
        let dsym_dir = fs::read_dir(file_name.parent().expect("parent"))
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.extension() == Some(std::ffi::OsStr::new("dSYM")))
            .next()
            .unwrap();
        load_dsym(dsym_dir)
    } else {
        fs::read(file_name).unwrap()
    };

    let debug_file = object::File::parse(&debug_file).expect("parse file");
    let addr2line = addr2line::Context::new(&debug_file).expect("create context");

    let endian = if debug_file.is_little_endian() {
        gimli::RunTimeEndian::Little
    } else {
        gimli::RunTimeEndian::Big
    };

    let dwarf = gimli::read::Dwarf::load(
        |sect_id| {
            let data = debug_file.section_data_by_name(sect_id.name()).unwrap_or(Cow::Borrowed(&[]));
            Ok(gimli::EndianRcSlice::new(Rc::from(&*data), endian))
        },
        |_| Ok::<_, ()>(gimli::EndianRcSlice::new(Rc::from([]), endian)),
    ).unwrap();

    Context {
        addr2line,
        dwarf_context: crate::dwarf::DwarfContext::new(dwarf),
    }
}

fn load_dsym(dsym_dir: PathBuf) -> Vec<u8> {
    let mut dir_iter = fs::read_dir(dsym_dir.join("Contents/Resources/DWARF"))
        .unwrap();

    let debug_file_name = dir_iter
        .next()
        .unwrap()
        .unwrap()
        .path();

    assert!(dir_iter.next().is_none());

    fs::read(debug_file_name).unwrap()
}
