#![allow(unsafe_code)]

extern crate regex;

use regex::Regex;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{
    fs::{self, File},
    io::Result,
    process::Command,
    str,
};

////
//// Statics and Constants
////
static mut CONFIG_FLAGS: Vec<&'static str> = Vec::new();
static mut COMPILE_FLAGS: Vec<&'static str> = Vec::new();
macro_rules! define_lazy_path {
    ($name:ident, $path:expr) => {
        static $name: LazyLock<PathBuf> = LazyLock::new(|| PathBuf::from($path));
    };
}
fn add_flag(flags: &mut Vec<&'static str>, flag: &'static str) {
    flags.push(flag);
}

// Always required
static MINIMUM_GIT_VERSION: [u8; 3] = [2, 40, 0]; // Still maintained
static MINIMUM_CMAKE_VERSION: [u8; 3] = [3, 31, 0]; // Features for C++20
const ABSEIL_SRC: &str = "https://github.com/abseil/abseil-cpp.git";

// Directories
define_lazy_path!(BUILD_DIR, "target/");
define_lazy_path!(SOURCE_DIR, "target/abseil-cpp/");
define_lazy_path!(ABSEIL_BUILD_DIR, "target/abseil-cpp/build/");
define_lazy_path!(OUTPUT_DIR, "external/");
define_lazy_path!(BIND_FILE, "external/bindings.cpp");
define_lazy_path!(INCLUDE_DIR, "external/include/");
define_lazy_path!(LIB_DIR, "external/lib/");

fn build_flags() {
    unsafe {
        // Build flags for CMake
        add_flag(&mut CONFIG_FLAGS, "-DABSL_USE_GOOGLETEST_HEAD=ON");
        add_flag(&mut CONFIG_FLAGS, "-DCMAKE_CXX_STANDARD_REQUIRED=ON");
        add_flag(&mut CONFIG_FLAGS, "-DCMAKE_CXX_STANDARD=20");
        #[cfg(debug_assertions)]
        add_flag(&mut CONFIG_FLAGS, "-DCMAKE_BUILD_TYPE=Debug");
        #[cfg(not(debug_assertions))]
        add_flag(&mut CONFIG_FLAGS, "-DCMAKE_BUILD_TYPE=Release");
        #[cfg(all(target_os = "windows", target_env = "msvc"))]
        add_flag(&mut CONFIG_FLAGS, "-DABSL_MSVC_STATIC_RUNTIME=ON");

        // Windows Compile-Time flags
        #[cfg(all(target_os = "windows", target_env = "msvc"))]
        add_flag(&mut COMPILE_FLAGS, "--build");
        add_flag(&mut COMPILE_FLAGS, ".");
        add_flag(&mut COMPILE_FLAGS, "--");
        add_flag(&mut COMPILE_FLAGS, "/p:Platform=x64");
        #[cfg(all(target_os = "windows", target_env = "msvc", debug_assertions))]
        add_flag(&mut COMPILE_FLAGS, "/p:Configuration=Debug");
        #[cfg(all(target_os = "windows", target_env = "msvc", not(debug_assertions)))]
        add_flag(&mut COMPILE_FLAGS, "/p:Configuration=Release");
    }
}

////
//// Primary Build Loop
////

/// The real main function.
fn __build() {
    println!("Performing library build...");
    build_flags();
    println!(" ");

    // Pre-build checks
    println!("---");
    println!("Confirming build environment...");
    basics_check();
    println!(" ");

    // Create build directory
    println!("---");
    println!("Creating build directory...");
    create_path(&BUILD_DIR).expect("Unable to create build directory");
    println!(" ");

    // Git clone
    println!("---");
    println!("Obtaining abseil-cpp source code...");
    run_command("git", &["clone", ABSEIL_SRC], &BUILD_DIR);
    println!(" ");

    // Create abseil build directory
    println!("---");
    println!("Creating Abseil build directory...");
    create_path(&ABSEIL_BUILD_DIR).expect("Unable to create Abseil build directory");
    println!(" ");

    // Cmake config
    println!("---");
    println!("Running CMake config...");
    unsafe {
        add_flag(&mut CONFIG_FLAGS, "..");
        run_command("cmake", &CONFIG_FLAGS, &ABSEIL_BUILD_DIR);
    }
    println!(" ");

    // Cmake build
    println!("---");
    println!("Running CMake build...");
    unsafe {
        run_command("cmake", &CONFIG_FLAGS, &ABSEIL_BUILD_DIR);
    }

    // Gather libs
    println!("---");
    println!("Gathering built libraries...");
    gather_libs();
    println!(" ");

    // Gather includes
    println!("---");
    println!("Gathering include files...");
    gather_includes();
    println!(" ");

    // Generate bindings
    println!("---");
    println!("Generating bindings...");
    generate_bindings().expect("Failed to generate bindings");
    println!(" ");

    //println!("6. Moving the built DLL to the project root");
    //if cfg!(debug_assertions) {
    //    let _source = ABSEIL_BUILD_DIR.join("bin/Release/abseil_dll.dll");
    //    let _destination = OUTPUT_DIR.join("abseil.dll");
    //    // fs::rename(&_source, &_destination).expect("Failed to move DLL file");
    //} else {
    //    let _source = ABSEIL_BUILD_DIR.join("bin/Debug/abseil_dll.dll");
    //    let _destination = OUTPUT_DIR.join("abseil_d.dll");
    //    // fs::rename(&_source, &_destination).expect("Failed to move DLL file");

    //    let _source = ABSEIL_BUILD_DIR.join("bin/Debug/abseil_dll.pdb");
    //    let _destination = OUTPUT_DIR.join("abseil_d.pdb");
    //    // fs::rename(&_source, &_destination).expect("Failed to move PDB file");
    //}

    println!("Build script completed successfully.");
}

////
//// Generic Helpers
////

/// Creates the directory (including any necessary parent directories)
pub fn create_path(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

/// Executes a command with the provided arguments and returns its standard output as a UTF-8 string.
pub fn run_command(command: &str, args: &[&str], path: &std::path::Path) -> String {
    println!(
        "Running command: {} {:?} in directory: {:?}",
        command, args, path
    );
    let output = Command::new(command)
        .args(args)
        .current_dir(path)
        .output()
        .unwrap();

    if !output.status.success() {
        panic!(
            "Command was not successful. Output: {}",
            String::from_utf8(output.stderr).unwrap()
        );
    }
    return String::from_utf8(output.stdout).unwrap();
}

////
//// Gathering helpers
////

/// Gathers all the built libs, pdbs, and exps.
fn gather_libs() {
    let source = &*ABSEIL_BUILD_DIR;
    let destination = &*LIB_DIR;

    // Create the destination directory if it does not exist.
    create_path(&*LIB_DIR).expect("Unable to create output directory");

    // Recursively walk the directory tree.
    fn visit_dirs(dir: &Path, destination: &Path) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, destination)?;
            } else if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext == "lib" || ext == "pdb" || ext == "exp" {
                    let file_name = path.file_name().expect("File must have a name");
                    let dest_path = destination.join(file_name);
                    fs::copy(&path, &dest_path)
                        .unwrap_or_else(|_| panic!("Failed copying {:?} to {:?}", path, dest_path));
                }
            }
        }
        Ok(())
    }
    visit_dirs(source, &destination).expect("Failed to gather libs");
}

/// Gather inclusion tree
fn gather_includes() {
    let source = &*ABSEIL_BUILD_DIR.join("absl");
    let destination = &*INCLUDE_DIR;

    // Create the destination root directory if it does not exist.
    create_path(&*INCLUDE_DIR).expect("Unable to create include directory");

    // Recursively walk the directory tree starting at `source`,
    // computing the relative path for each header file, then copy it to `destination`.
    fn visit_dirs(src_dir: &Path, dest_dir: &Path, base: &Path) -> Result<()> {
        for entry in fs::read_dir(src_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, dest_dir, base)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("h") {
                // Compute the relative path from the base (ABSEIL_BUILD_DIR/absl)
                let relative_path = path
                    .strip_prefix(base)
                    .expect("Failed to compute relative path");
                let dest_file_path = dest_dir.join(relative_path);
                // Ensure the parent directories exist
                if let Some(parent) = dest_file_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&path, &dest_file_path).unwrap_or_else(|_| {
                    panic!("Failed copying {:?} to {:?}", path, dest_file_path)
                });
            }
        }
        Ok(())
    }
    visit_dirs(&source, &destination, &source).expect("Failed to gather include files");
}

////
//// Bindings helpers
////

fn generate_bindings() -> Result<()> {
    // Set the directory that contains your header files.
    let headers_dir = &*INCLUDE_DIR;
    // Output file that will contain the generated wrappers.
    let bindings_path = &*BIND_FILE;
    let bindings_file = File::create(&bindings_path)?;
    let mut writer = BufWriter::new(bindings_file);

    // Write the header for the generated file.
    writeln!(writer, "// language: C++")?;
    writeln!(
        writer,
        "// This file is auto-generated. It includes all header files from the external folder"
    )?;
    writeln!(writer)?;

    // Begin extern "C" block.
    writeln!(writer, "#ifdef __cplusplus")?;
    writeln!(writer, "extern \"C\" {{")?;
    writeln!(writer, "#endif")?;
    writeln!(writer)?;

    // Recursively scan for header files and include them.
    generate_bind_includes(headers_dir, headers_dir, &mut writer)?;

    writeln!(writer)?;
    // Now generate wrappers for functions found in the header files.
    generate_bind_wrappers(headers_dir, &mut writer)?;

    writeln!(writer)?;
    writeln!(writer, "#ifdef __cplusplus")?;
    writeln!(writer, "}}")?;
    writeln!(writer, "#endif")?;
    writeln!(writer)?;

    println!("Generated wrapper file at: {:?}", bindings_path);
    Ok(())
}

/// Recursively traverses `base_dir` and writes an #include for each header.
fn generate_bind_includes(
    base_dir: &Path,
    current_dir: &Path,
    writer: &mut BufWriter<File>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            generate_bind_includes(base_dir, &path, writer)?;
        } else if let Some(ext) = path.extension() {
            if ext.to_string_lossy().eq_ignore_ascii_case("h") {
                // Build the include path relative to base_dir.
                let relative_path = path.strip_prefix(base_dir).unwrap();
                // Use forward slashes in C++ include paths.
                let include_path = relative_path.to_string_lossy().replace("\\", "/");
                writeln!(writer, "#include \"{}\"", include_path)?;
            }
        }
    }
    Ok(())
}

/// Searches for function declarations in header files and generates simple wrapper stubs.
/// NOTE: This is a very simplistic approach that uses a regex to match C-style
/// function declarations. Real-world headers may require a proper parser.
fn generate_bind_wrappers(headers_dir: &Path, writer: &mut BufWriter<File>) -> std::io::Result<()> {
    // This regex is a naive attempt to capture a return type and a function name.
    // It will match lines like:
    //   int my_function(...);
    // and capture "int" and "my_function".
    // Adjust the regex to suit your headers.
    let func_regex =
        Regex::new(r"(?m)^\s*(template\s*<[^;:{]+>\s*)?([\w:\*&<>\s]+)\s+(\w+)\s*\(").unwrap();

    // Recursively find all .h files
    for entry in fs::read_dir(headers_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            generate_bind_wrappers(&path, writer)?;
        } else if let Some(ext) = path.extension() {
            if ext.to_string_lossy().eq_ignore_ascii_case("h") {
                let mut file = File::open(&path)?;
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;

                for cap in func_regex.captures_iter(&contents) {
                    let cap = cap; // Directly use the captured match.
                    let return_type = cap.get(1).unwrap().as_str().trim();
                    let func_name = cap.get(2).unwrap().as_str().trim();
                    // Skip if the function name is a macro or already wrapped.
                    if func_name.starts_with("LOW_LEVEL_ALLOC") {
                        continue;
                    }
                    // Write a simple wrapper.
                    // (Here we assume a void wrapper with no parameters for demonstration.
                    // In practice you would want to capture the full signature.)
                    writeln!(
                        writer,
                        "  // Wrapper for function declared in {:?}",
                        path.file_name().unwrap()
                    )?;
                    writeln!(
                        writer,
                        "  {ret} {name}_wrapper() {{ return {name}(); }}",
                        ret = return_type,
                        name = func_name
                    )?;
                    writeln!(writer)?;
                }
            }
        }
    }
    Ok(())
}

////
//// Versioning Helpers
////

/// Checks if the given version string meets or exceeds the required version.
fn is_version_valid(version_token: &str, req: [u8; 3]) -> bool {
    if let Some((major, minor, patch)) = extract_version(version_token) {
        (major, minor, patch) >= (req[0] as u32, req[1] as u32, req[2] as u32)
    } else {
        false
    }
}

/// Extracts the version from a string in the format "X.Y.Z".
fn extract_version(version: &str) -> Option<(u32, u32, u32)> {
    let re = Regex::new(r"(\d+)\.(\d+)\.(\d+)").unwrap();
    let caps = re.captures(version)?;
    Some((
        caps.get(1)?.as_str().parse().ok()?,
        caps.get(2)?.as_str().parse().ok()?,
        caps.get(3)?.as_str().parse().ok()?,
    ))
}

/// Checks the git version and panics if it is not valid.
fn check_git_version(version_str: &str) -> bool {
    if is_version_valid(version_str, MINIMUM_GIT_VERSION) {
        println!("git found. Proceeding.");
        return true;
    }
    panic!("Please install, update, or repair git.");
}

/// Checks the cmake version and panics if it is not valid.
fn check_cmake_version(version_str: &str) -> bool {
    if is_version_valid(version_str, MINIMUM_CMAKE_VERSION) {
        println!("CMake found. Proceeding.");
        return true;
    }
    panic!("Please install, update, or repair CMake.");
}

////
//// Target Specifics
////

#[cfg(all(target_os = "windows", target_env = "msvc"))]
/// Checks the MSVC version and panics if it is not valid.
fn check_msvc_version(capabilities: &str) -> bool {
    let re = Regex::new(r"Visual Studio (16 2019|17 2022)").unwrap();
    if let Some(m) = re.find(capabilities) {
        println!("{} found. Proceeding.", m.as_str());
        return true;
    }
    panic!("CMake reports no suitable MSVC version. Needs 2019+.");
}

#[cfg(all(target_os = "windows", target_env = "msvc"))]
/// Checks the windows build requirements, everything panics if not valid.
fn basics_check() {
    let git_version = run_command("git", &["--version"], &BUILD_DIR);
    check_git_version(&git_version);
    let cmake_version = run_command("CMake", &["--version"], &BUILD_DIR);
    check_cmake_version(&cmake_version);
    let caps = run_command("CMake", &["-E", "capabilities"], &BUILD_DIR);
    check_msvc_version(&caps);
}

#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
/// Not yet implemented for other platforms.
fn basics_check() {
    panic!("Architecture not yet supported.");
}

////
//// Main Function, calls builder.
////

fn main() {
    unsafe {
        __build();
    }
}
