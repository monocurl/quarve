use std::{fs, io};
use std::path::Path;

/// https://stackoverflow.com/questions/26958489/how-to-copy-a-folder-recursively-in-rust
pub fn copy_directory(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_directory(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}