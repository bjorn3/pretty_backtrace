use std::fs;
use std::path::{Path, PathBuf};

pub struct Context {
    pub addr2line: addr2line::Context<crate::dwarf::Slice>,
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

    Context {
        addr2line,
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
