#![allow(unsafe_code)]

extern crate regex;
use regex::Regex;
use std::{
    fs::{self, File},
    io::{BufWriter, Result, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{LazyLock, Mutex},
};

static FUNC_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*(template\s*<[^;:{]+>\s*)?([\w:\*&<>\s]+)\s+(\w+)\s*\(")
        .expect("Failed to compile regex")
});

// Extracts function details: (template, return type, name)
pub fn extract_function_details(src: &str) -> Vec<(String, String, String)> {
    let mut results = Vec::new();
    for cap in FUNC_REGEX.captures_iter(src) {
        results.push((
            cap.get(1).map_or("", |m| m.as_str()).trim().to_owned(),
            cap.get(2).unwrap().as_str().trim().to_owned(),
            cap.get(3).unwrap().as_str().trim().to_owned(),
        ));
    }
    results
}

static CONFIG_FLAGS: LazyLock<Mutex<Vec<&'static str>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static COMPILE_FLAGS: LazyLock<Mutex<Vec<&'static str>>> = LazyLock::new(|| Mutex::new(Vec::new()));

macro_rules! define_lazy_path {
    ($name:ident, $path:expr) => {
        static $name: LazyLock<PathBuf> = LazyLock::new(|| PathBuf::from($path));
    };
}
macro_rules! add_flag {
    ($flags:expr, $flag:expr) => {{
        $flags.lock().unwrap().push($flag);
    }};
}

static MINIMUM_GIT_VERSION: [u8; 3] = [2, 40, 0];
static MINIMUM_CMAKE_VERSION: [u8; 3] = [3, 31, 0];
const ABSEIL_SRC: &str = "https://github.com/abseil/abseil-cpp.git";

define_lazy_path!(BUILD_DIR, "target/");
define_lazy_path!(ABSEIL_BUILD_DIR, "target/abseil-cpp/build/");
define_lazy_path!(SOURCE_DIR, "target/abseil-cpp/absl/");
define_lazy_path!(BIND_FILE, "external/bindings.cpp");
define_lazy_path!(INCLUDE_DIR, "external/include/");
define_lazy_path!(LIB_DIR, "external/lib/");

// Sets build flags.
fn build_flags() {
    add_flag!(CONFIG_FLAGS, "-DABSL_USE_GOOGLETEST_HEAD=ON");
    add_flag!(CONFIG_FLAGS, "-DCMAKE_CXX_STANDARD_REQUIRED=ON");
    add_flag!(CONFIG_FLAGS, "-DCMAKE_CXX_STANDARD=20");
    #[cfg(debug_assertions)]
    add_flag!(CONFIG_FLAGS, "-DCMAKE_BUILD_TYPE=Debug");
    #[cfg(not(debug_assertions))]
    add_flag!(CONFIG_FLAGS, "-DCMAKE_BUILD_TYPE=Release");
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    {
        add_flag!(CONFIG_FLAGS, "-DABSL_MSVC_STATIC_RUNTIME=ON");
        add_flag!(COMPILE_FLAGS, "--build");
        add_flag!(COMPILE_FLAGS, ".");
        add_flag!(COMPILE_FLAGS, "--");
        add_flag!(COMPILE_FLAGS, "/p:Platform=x64");
        #[cfg(debug_assertions)]
        add_flag!(COMPILE_FLAGS, "/p:Configuration=Debug");
        #[cfg(not(debug_assertions))]
        add_flag!(COMPILE_FLAGS, "/p:Configuration=Release");
    }
}

// Creates a directory if it doesn't exist.
// Logs any error and continues.
fn create_path(path: &Path) {
    if !path.exists() {
        if let Err(err) = fs::create_dir_all(path) {
            eprintln!("Failed to create path {:?}: {}", path, err);
        }
    }
}

// Recursively copies header files; logs errors and continues.
fn visit_dirs(src_dir: &Path, dest_dir: &Path, base: &Path) {
    let entries = fs::read_dir(src_dir).unwrap_or_else(|err| {
        eprintln!("Failed to read directory {:?}: {}", src_dir, err);
        // Return an empty iterator on error.
        fs::read_dir("/dev/null").unwrap()
    });
    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_dir() {
                    visit_dirs(&path, dest_dir, base);
                } else if path.extension().and_then(|s| s.to_str()) == Some("h") {
                    let dest_file_path = dest_dir.join(path.strip_prefix(base).unwrap());
                    if let Some(parent) = dest_file_path.parent() {
                        if let Err(err) = fs::create_dir_all(parent) {
                            eprintln!("Failed to create directory {:?}: {}", parent, err);
                            continue;
                        }
                    }
                    if let Err(err) = fs::copy(&path, &dest_file_path) {
                        eprintln!(
                            "Failed to copy file {:?} to {:?}: {}",
                            path, dest_file_path, err
                        );
                    }
                }
            }
            Err(err) => eprintln!("Failed to process directory entry: {}", err),
        }
    }
}

// Generates C++ bindings; a failure here is critical.
fn generate_bindings() -> Result<()> {
    let headers_dir = &*INCLUDE_DIR;
    let bindings_path = &*BIND_FILE;
    let mut writer = BufWriter::new(File::create(bindings_path)?);
    writeln!(writer, "// language: C++")?;
    writeln!(
        writer,
        "// Auto-generated: includes from the external folder"
    )?;
    writeln!(writer, "#ifdef __cplusplus")?;
    writeln!(writer, "extern \"C\" {{")?;
    writeln!(writer, "#endif")?;
    writeln!(writer)?;
    generate_bind_includes(headers_dir, headers_dir, &mut writer)?;
    writeln!(writer)?;
    generate_bind_wrappers(headers_dir, &mut writer)?;
    writeln!(writer)?;
    writeln!(writer, "#ifdef __cplusplus")?;
    writeln!(writer, "}}")?;
    writeln!(writer, "#endif")?;
    println!("Generated bindings at: {:?}", bindings_path);
    Ok(())
}

// Generates include directives; a failure here is critical.
fn generate_bind_includes(
    base_dir: &Path,
    current_dir: &Path,
    writer: &mut BufWriter<File>,
) -> Result<()> {
    for entry in fs::read_dir(current_dir)? {
        let path = entry?.path();
        if path.is_dir() {
            generate_bind_includes(base_dir, &path, writer)?;
        } else if path
            .extension()
            .and_then(|s| s.to_str())
            .map_or(false, |ext| ext.eq_ignore_ascii_case("h"))
        {
            let include_path = path
                .strip_prefix(base_dir)
                .unwrap()
                .to_string_lossy()
                .replace("\\", "/");
            writeln!(writer, "#include \"{}\"", include_path)?;
        }
    }
    Ok(())
}

// Placeholder for future wrapper generation.
fn generate_bind_wrappers(_headers_dir: &Path, writer: &mut BufWriter<File>) -> Result<()> {
    writeln!(writer, "// Wrappers go here")
}

// Runs a command and returns its stdout; logs error and returns an empty string on failure.
fn run_command(command: &str, args: &[&str], path: &Path) -> String {
    let output = Command::new(command).args(args).current_dir(path).output();
    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8(output.stdout).unwrap_or_default()
        }
        Ok(output) => {
            eprintln!(
                "Command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::new()
        }
        Err(err) => {
            eprintln!("Failed to run command {}: {}", command, err);
            String::new()
        }
    }
}

// Gathers header files by copying them from SOURCE_DIR to INCLUDE_DIR.
fn gather_includes() {
    let source = &*SOURCE_DIR;
    let destination = &*INCLUDE_DIR;
    if !source.exists() {
        eprintln!("Source {:?} missing, skipping.", source);
        return;
    }
    create_path(destination);
    visit_dirs(source, destination, source);
}

fn main() {
    build_flags();
    create_path(&BUILD_DIR);
    create_path(&ABSEIL_BUILD_DIR);
    gather_includes();
    if let Err(err) = generate_bindings() {
        eprintln!("Failed to generate bindings: {}", err);
    }
    println!("Build script completed successfully.");
}
