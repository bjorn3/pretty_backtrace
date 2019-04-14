use std::fs;
use std::path::{Path, PathBuf};

use addr2line::Context;
use object::Object;

macro_rules! read_and_parse {
    (let $var:ident = $file_name:expr) => {
        let file = fs::read($file_name).unwrap();
        let $var = object::File::parse(&file).expect("Couldn't parse binary");
    };
}

pub fn get_context() -> Context {
    let bin_file_name = std::env::current_exe().expect("current bin");
    get_context_for_file(&bin_file_name)
}

pub fn get_context_for_file(file_name: &Path) -> Context {
    read_and_parse!(let obj = file_name);
    if obj.has_debug_symbols() {
        return addr2line::Context::new(&obj).expect("create context");
    }

    if let Some(context) = macos_fastpath(file_name, &obj) {
        return context;
    }

    let debug_file_path = spin_loop_locate_debug_symbols(file_name, &obj).unwrap();

    read_and_parse!(let debug_file = debug_file_path);
    addr2line::Context::new(&debug_file).expect("create context")
}

/// On macOS it can take some time for spotlight to index the dSYM file. When built by cargo, we can
/// likely find the dSYM file in target/<profile>/deps or target/<profile>/examples. This function
/// will try to find it there.
///
/// # Arguments
///
/// * Path to the object file which needs its debuginfo.
/// * Parsed version of the object file.
fn macos_fastpath(path: &Path, obj: &object::File) -> Option<Context> {
    // Step 1. Return if not on macOS, because debuginfo is stored in the object file itself on OSes other than macOS.
    if cfg!(not(target_os = "macos")) {
        return None;
    }

    // Step 2. Get the path to the target dir of the current build channel.
    let mut target_channel_dir = path;
    loop {
        let parent = target_channel_dir.parent()?;
        target_channel_dir = parent;

        if target_channel_dir.parent().and_then(|parent| parent.file_name()) == Some(std::ffi::OsStr::new("target")) {
            break; // target_dir = ???/target/<channel>
        }
    }

    // Step 3. Check every entry in <target_channel_dir>/deps and <target_channel_dir>/examples
    for dir in fs::read_dir(target_channel_dir.join("deps")).unwrap().chain(fs::read_dir(target_channel_dir.join("examples")).unwrap()) {
        let dir = dir.unwrap().path();

        // Step 4. If not a dSYM dir, try next entry.
        if dir.extension() != Some(std::ffi::OsStr::new("dSYM")) {
            continue;
        }

        // Step 5. Get path to inner object file.
        let mut dir_iter = fs::read_dir(dir.join("Contents/Resources/DWARF"))
            .unwrap();

        let debug_file_name = dir_iter
            .next()
            .unwrap()
            .unwrap()
            .path();

        assert!(dir_iter.next().is_none());

        // Step 6. Parse inner object file.
        read_and_parse!(let dsym = debug_file_name);

        // Step 7. Make sure the dSYM file matches the object file to find debuginfo for.
        if obj.mach_uuid() == dsym.mach_uuid() {
            return Some(addr2line::Context::new(&dsym).expect("create context"));
        }
    }

    None
}

fn spin_loop_locate_debug_symbols(file_name: &Path, obj: &object::File) -> Option<PathBuf> {
    let mut i = 0;
    loop {
        match moria::locate_debug_symbols(&obj, file_name) {
            Ok(res) => {
                if i != 0 {
                    eprintln!();
                }
                return Some(res);
            }
            Err(err) => {
                if i == 0 {
                    if err.to_string() != "dSYM not found" {
                        eprintln!("{}", err);
                        if i != 0 {
                            eprintln!();
                        }
                        Err::<(), _>(err).unwrap();
                    }
                    eprint!("Searching for dSYM file");
                } else if i == 60 {
                    eprintln!(" Couldn't find dSYM file.");
                    return None;
                } else {
                    eprint!(".");
                }
                i += 1;
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}
