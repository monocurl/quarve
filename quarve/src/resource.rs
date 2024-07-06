use std::ops::Deref;
use std::path::{Path, PathBuf};

#[cfg(not(debug_assertions))]
use crate::native;

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

    pub fn named(rel_path: &str) -> Resource {
        Resource(resource_root().join(rel_path))
    }

    pub fn path(&self) -> &Path {
        self.0.deref()
    }
}

impl From<&Path> for Resource {
    fn from(value: &Path) -> Self {
        Self::new(value)
    }
}
