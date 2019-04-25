use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use object::Object;

pub struct Context {
    pub addr2line: addr2line::Context,
    pub dwarf: gimli::read::Dwarf<gimli::EndianRcSlice<gimli::RunTimeEndian>>,
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

    fn load_section<'data, 'file, O, S, Endian>(file: &'file O, endian: Endian) -> S
    where
        O: object::Object<'data, 'file>,
        S: gimli::Section<gimli::EndianRcSlice<Endian>>,
        Endian: gimli::Endianity,
    {
        let data = file.section_data_by_name(S::section_name()).unwrap_or(Cow::Borrowed(&[]));
        S::from(gimli::EndianRcSlice::new(Rc::from(&*data), endian))
    }

    let dwarf = gimli::read::Dwarf {
        debug_abbrev: load_section(&debug_file, endian),
        debug_addr: load_section(&debug_file, endian),
        debug_info: load_section(&debug_file, endian),
        debug_line: load_section(&debug_file, endian),
        debug_line_str: load_section(&debug_file, endian),
        debug_str: load_section(&debug_file, endian),
        debug_str_offsets: load_section(&debug_file, endian),
        debug_str_sup: gimli::EndianRcSlice::new(Rc::new([]), endian).into(),
        debug_types: load_section(&debug_file, endian),
        locations: gimli::LocationLists::new(
            load_section(&debug_file, endian),
            load_section(&debug_file, endian),
        ),
        ranges: gimli::RangeLists::new(
            load_section(&debug_file, endian),
            load_section(&debug_file, endian),
        ),
    };

    Context {
        addr2line,
        dwarf,
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
