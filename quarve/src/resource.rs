use std::ffi::{CStr, CString};
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

#[derive(PartialEq, Eq, Clone)]
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

    // https://stackoverflow.com/a/59224987
    pub(crate) fn cstring(&self) -> CString {
        let mut buf = Vec::new();

        #[cfg(unix)] {
            use std::os::unix::ffi::OsStrExt;
            buf.extend(self.path().as_os_str().as_bytes());
        }

        #[cfg(windows)] {
            use std::os::windows::ffi::OsStrExt;
            buf.extend(self.path().as_os_str()
                .encode_wide()
                .map(|b| {
                    let b = b.to_ne_bytes();
                    b.get(0).map(|s| *s).into_iter().chain(b.get(1).map(|s| *s))
                })
                .flatten());
        }

        CString::new(buf).unwrap()
    }
}

impl From<&Path> for Resource {
    fn from(value: &Path) -> Self {
        Self::new(value)
    }
}
