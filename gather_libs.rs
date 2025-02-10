use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() -> io::Result<()> {
    // First operation: copy *.lib and *.pdb files from build/abseil-cpp/build/absl that reside in a Debug folder.
    let source_root_build = fs::canonicalize("./target/abseil-cpp/build/absl")?;
    let destination = fs::canonicalize("./external")?;
    copy_files_with_filter(
        &source_root_build,
        &destination,
        |path| {
            let file = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
            // Only process .lib and .pdb files ...
            if !(file.ends_with(".lib") || file.ends_with(".pdb")) {
                return false;
            }
            // ... that are in a directory named "Debug"
            path.components().any(|c| c.as_os_str() == "Debug")
        },
        /* remove_debug */ true,
        &source_root_build,
    )?;

    // Second operation: copy all .h header files from target/abseil-cpp/absl preserving their folder structure.
    let source_root_includes = fs::canonicalize("./target/abseil-cpp/absl")?;
    copy_files_with_filter(
        &source_root_includes,
        &destination,
        |path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("h"))
                .unwrap_or(false)
        },
        /* remove_debug */ false,
        &source_root_includes,
    )?;

    Ok(())
}

/// Recursively traverses `current_dir` looking for files satisfying `predicate`.
/// Files are copied to `dest_base` preserving the relative path from `source_base`.
/// If `remove_debug` is true, any path segment equal to "Debug" is filtered out.
fn copy_files_with_filter<F>(
    current_dir: &Path,
    dest_base: &Path,
    predicate: F,
    remove_debug: bool,
    source_base: &Path,
) -> io::Result<()>
where
    F: Fn(&Path) -> bool,
{
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            copy_files_with_filter(&path, dest_base, &predicate, remove_debug, source_base)?;
        } else if predicate(&path) {
            // Create a relative path from source_base
            let rel_path = path
                .strip_prefix(source_base)
                .unwrap_or(&path)
                .to_path_buf();

            // Optionally remove any "Debug" folder from the relative path.
            let filtered_path = if remove_debug {
                rel_path
                    .components()
                    .filter(|comp| comp.as_os_str() != "Debug")
                    .collect::<PathBuf>()
            } else {
                rel_path
            };

            let dest_path = dest_base.join(&filtered_path);
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &dest_path)?;
        }
    }
    Ok(())
}
