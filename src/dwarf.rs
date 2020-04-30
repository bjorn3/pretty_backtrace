use std::cell::RefCell;
use std::rc::Rc;

use gimli::{DebuggingInformationEntry, Dwarf, Range, Unit};

pub type Slice = gimli::EndianRcSlice<gimli::RunTimeEndian>;

pub struct UnitMap {
    cached: RefCell<Vec<(Range, gimli::Result<Rc<Unit<Slice>>>)>>,
    iter: RefCell<gimli::CompilationUnitHeadersIter<Slice>>,
}

impl UnitMap {
    pub fn new(iter: gimli::CompilationUnitHeadersIter<Slice>) -> Self {
        UnitMap {
            cached: RefCell::new(Vec::new()),
            iter: RefCell::new(iter),
        }
    }

    pub fn find(
        &self,
        dwarf: &Dwarf<Slice>,
        svma: findshlibs::Svma,
    ) -> gimli::Result<Option<Rc<Unit<Slice>>>> {
        for (range, item) in self.cached.borrow().iter() {
            if range.begin <= svma.0 as u64 && range.end > svma.0 as u64 {
                return Ok(Some(item.clone()?));
            }
        }

        while let Some(unit) = self.iter.borrow_mut().next()? {
            let unit = Unit::new(&dwarf, unit)?;
            let unit = Rc::new(unit);
            let mut ranges = dwarf.unit_ranges(&unit)?;

            let mut found = false;
            while let Some(range) = ranges.next()? {
                if range.begin <= svma.0 as u64 && range.end > svma.0 as u64 {
                    found = true;
                }
                self.cached.borrow_mut().push((range, Ok(unit.clone())));
            }

            if found {
                return Ok(Some(unit));
            }
        }

        Ok(None)
    }
}

pub struct DwarfContext {
    pub dwarf: Dwarf<Slice>,
    pub units: UnitMap,
}

impl DwarfContext {
    pub fn new(dwarf: Dwarf<Slice>) -> Self {
        let units = UnitMap::new(dwarf.units());
        DwarfContext {
            dwarf,
            units,
        }
    }
}

pub fn in_range(dwarf: &Dwarf<Slice>, unit: &Unit<Slice>, entry: Option<&DebuggingInformationEntry<Slice>>, svma: findshlibs::Svma) -> gimli::Result<bool> {
    if let Some(entry) = entry {
        let mut ranges = dwarf.die_ranges(unit, entry)?;
        while let Some(range) = ranges.next()? {
            if range.begin <= svma.0 as u64 && range.end > svma.0 as u64 {
                return Ok(true);
            }
        }
    } else {
        let mut ranges = dwarf.unit_ranges(unit)?;
        while let Some(range) = ranges.next()? {
            if range.begin <= svma.0 as u64 && range.end > svma.0 as u64 {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

pub fn find_unit_for_svma(context: &DwarfContext, svma: findshlibs::Svma) -> gimli::Result<Option<Rc<Unit<Slice>>>> {
    Ok(context.units.find(&context.dwarf, svma)?)
}

pub fn find_die_for_svma<'dwarf, 'unit: 'dwarf>(
    dwarf: &'dwarf Dwarf<Slice>,
    unit: &'unit Unit<Slice>,
    svma: findshlibs::Svma,
) -> gimli::Result<Option<DebuggingInformationEntry<'dwarf, 'unit, Slice>>> {
    search_tree(unit, None, |entry, _indent| {
        if in_range(dwarf, unit, Some(&entry), svma)? {
            if entry.tag() == gimli::DW_TAG_subprogram {
                Ok(SearchAction::Found(entry))
            } else {
                Ok(SearchAction::VisitChildren)
            }
        } else {
            Ok(SearchAction::VisitChildren)
        }
    })
}

pub enum SearchAction<T> {
    Found(T),
    VisitChildren,
    SkipChildren,
}

pub fn search_tree<'dwarf, 'unit: 'dwarf, T>(
    unit: &'unit Unit<Slice>,
    offset: Option<gimli::UnitOffset>,
    mut f: impl FnMut(DebuggingInformationEntry<'dwarf, 'unit, Slice>, usize) -> gimli::Result<SearchAction<T>>,
) -> gimli::Result<Option<T>> {
    fn process_tree<'dwarf, 'unit: 'dwarf, T>(
        unit: &Unit<Slice>,
        node: gimli::EntriesTreeNode<'dwarf, 'unit, '_, Slice>,
        indent: usize,
        f: &mut impl FnMut(DebuggingInformationEntry<'dwarf, 'unit, Slice>, usize) -> gimli::Result<SearchAction<T>>,
    ) -> gimli::Result<Option<T>> {
        let entry = node.entry().clone();

        match f(entry, indent)? {
            SearchAction::Found(val) => Ok(Some(val)),
            SearchAction::VisitChildren => {
                let mut children = node.children();
                while let Some(child) = children.next()? {
                    // Recursively process a child.
                    if let Some(val) = process_tree(unit, child, indent + 1, f)? {
                        return Ok(Some(val));
                    }
                }
                Ok(None)
            }
            SearchAction::SkipChildren => Ok(None),
        }
    }

    let mut entries_tree = unit.entries_tree(offset)?;
    process_tree(unit, entries_tree.root()?, 0, &mut f)
}
