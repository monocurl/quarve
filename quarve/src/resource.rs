use std::ops::Deref;
use std::path::{Path, PathBuf};

fn resource_root() -> PathBuf {
    // if debug, just the project root
    #[cfg(debug_assertions)]
    {
        std::env::current_dir().unwrap()
            .join("res")
    }

    // if production, os dependent
    #[cfg(not(debug_assertions))]
    {
        native::path::production_resource_root()
    }
}

#[derive(PartialEq, Eq)]
pub struct Resource(PathBuf);

impl Resource {
    pub fn new(rel_path: &Path) -> Resource {
        Resource(resource_root().join(rel_path))
    }

    pub fn path(&self) -> &Path {
        self.0.deref()
    }
}

