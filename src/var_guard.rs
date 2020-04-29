use std::cell::RefCell;
use std::fmt::Debug;

thread_local! {
    static VAR_GUARDS: RefCell<Vec<(&'static str, *const dyn Debug)>> = RefCell::new(Vec::new());
}

pub struct VarGuard<T: Debug>(pub T);

impl<T: Debug> VarGuard<T> {
    pub unsafe fn init<'a>(&'a mut self, name: &'static str) {
        VAR_GUARDS.with(|var_guards| {
            let ptr = &self.0 as *const (dyn Debug + 'a);
            let ptr = std::mem::transmute::<_, *const (dyn Debug + 'static)>(ptr);
            var_guards.borrow_mut().push((name, ptr));
        });
    }
}

impl<T: Debug> Drop for VarGuard<T> {
    fn drop(&mut self) {
        VAR_GUARDS.with(|var_guard| var_guard.borrow_mut().pop()/*FIXME .unwrap()*/);
    }
}

pub(crate) fn print_all() {
    VAR_GUARDS.with(|var_guards| {
        for var_guard in var_guards.borrow().iter().rev() {
            println!("{}: {:?}", var_guard.0, unsafe { &*var_guard.1 });
        }
    })
}

#[macro_export]
macro_rules! var_guard {
    ($var:ident) => {
        let mut __pretty_backtrace_guard = $crate::var_guard::VarGuard($var);
        unsafe { __pretty_backtrace_guard.init(stringify!($var)); }
        let $var = &mut __pretty_backtrace_guard.0;
    }
}

use crate::dwarf::*;

pub(crate) fn print_values(context: &crate::Context, svma: findshlibs::Svma) {
    let mut val_guard_count = 0;

    use gimli::read::Reader;
    let unit = if let Some(unit) = find_unit_for_svma(&context.dwarf, svma).unwrap() {
        unit
    } else {
        return;
    };

    if let Some(entry) = find_die_for_svma(&context.dwarf, &unit, svma).unwrap() {
        let _: Option<()> = search_tree(&unit, Some(entry.offset()), |entry, indent| {
            if entry.tag() == gimli::DW_TAG_lexical_block {
                if !in_range(&context.dwarf, &unit, Some(&entry), svma)? {
                    return Ok(SearchAction::SkipChildren);
                }
            }

            if entry.tag() == gimli::DW_TAG_variable {
                let name = if let Some(name) = entry.attr(gimli::DW_AT_name).unwrap() {
                    name.string_value(&context.dwarf.debug_str).unwrap().to_string().unwrap().into_owned()
                } else {
                    "<unknown name>".to_string()
                };
                println!("{:indent$}name: {}", "", name, indent = indent);
                if name == "__pretty_backtrace_guard" {
                    val_guard_count += 1;
                }
            }

            Ok(SearchAction::VisitChildren)
        }).unwrap();
    };

    while val_guard_count > 0 {
        val_guard_count -= 1;

        VAR_GUARDS.with(|var_guards| {
            let var_guard = var_guards.borrow_mut().pop().unwrap();
            println!("{}: {:?}", var_guard.0, unsafe { &*var_guard.1 });
        })
    }
}
