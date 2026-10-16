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
