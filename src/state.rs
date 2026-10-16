use crate::models::File;
#[derive(Default)]
pub struct BrowserState {
    pub files: Vec<File>,
    pub selected: std::collections::BTreeSet<String>,
    pub path: Vec<(String, String)>,
    pub generation: u64,
}
impl BrowserState {
    pub fn begin_navigation(&mut self) -> u64 {
        self.generation += 1;
        self.selected.clear();
        self.generation
    }
    pub fn apply(&mut self, generation: u64, files: Vec<File>) -> bool {
        if generation != self.generation {
            return false;
        }
        self.files = files;
        true
    }
    pub fn visible(&self, search: &str, descending: bool) -> Vec<File> {
        let search = search.to_lowercase();
        let mut out: Vec<_> = self
            .files
            .iter()
            .filter(|f| f.name.to_lowercase().contains(&search))
            .cloned()
            .collect();
        out.sort_by(|a, b| {
            b.is_folder().cmp(&a.is_folder()).then_with(|| {
                let order = a.name.to_lowercase().cmp(&b.name.to_lowercase());
                if descending { order.reverse() } else { order }
            })
        });
        out
    }
}

/// IME selections are UTF-16 offsets within the replacement text, not the old document.
/// Convert them to UTF-8 byte offsets before adding the insertion point.
pub fn composition_selection(
    text: &str,
    selection: std::ops::Range<usize>,
    insertion: usize,
) -> std::ops::Range<usize> {
    let byte_offset = |offset: usize| {
        let mut units = 0;
        let mut bytes = 0;
        for character in text.chars() {
            if units >= offset {
                break;
            }
            units += character.len_utf16();
            bytes += character.len_utf8();
        }
        bytes
    };
    let start = byte_offset(selection.start);
    let end = byte_offset(selection.end);
    insertion + start.min(end)..insertion + start.max(end)
}
