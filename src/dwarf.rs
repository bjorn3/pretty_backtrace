use gimli::{DebuggingInformationEntry, Dwarf, Unit};

pub type Slice = gimli::EndianRcSlice<gimli::RunTimeEndian>;

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

pub enum SearchAction<T> {
    #[allow(dead_code)] // FIXME
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
