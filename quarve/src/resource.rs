use std::ffi::CString;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use crate::core::APP;
use crate::native;

pub fn resource_root() -> PathBuf {
    // if debug, just the project root
    #[cfg(not(quarve_managed_run))]
    {
        std::env::current_dir().unwrap()
            .join("res")
    }

    // if production, os dependent
    #[cfg(quarve_managed_run)]
    {
        native::path::production_resource_root()
    }
}

pub fn local_storage() -> PathBuf {
    APP.with(|app| {
        native::path::local_storage(
            app.get()
                .expect("quarve::launch should have been called before this method")
                .name()
        )
    })
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Resource(pub PathBuf);

impl Resource {
    pub fn font(rel_path: impl AsRef<Path>) -> Resource {
        let path = resource_root().join("font/").join(rel_path);
        Resource(path)
    }

    pub fn named(rel_path: impl AsRef<Path>) -> Resource {
        let path = resource_root().join(rel_path);
        Resource(path)
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
        Self::named(value)
    }
}
