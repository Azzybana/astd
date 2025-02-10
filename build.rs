use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{fs, io::Result, process::Command, str};

////
//// Statics and Constants
////

/// Always required
static MINIMUM_GIT_VERSION: [u8; 3] = [2, 40, 0]; // Still maintained
static MINIMUM_CMAKE_VERSION: [u8; 3] = [3, 31, 0]; // Features for C++20
const ABSEIL_SRC: &str = "https://github.com/abseil/abseil-cpp.git";

/// Directories
static BUILD_DIR: LazyLock<PathBuf> = LazyLock::new(|| PathBuf::from("target/"));
static SOURCE_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let mut path = BUILD_DIR.clone();
    path.push("abseil-cpp/");
    path
});
static ABSEIL_BUILD_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let mut path = SOURCE_DIR.clone();
    path.push("build/");
    path
});
static _OUTPUT_DIR: LazyLock<PathBuf> = LazyLock::new(|| PathBuf::from("external/"));

/// Build flags for CMake
const CONFIG_FLAGS: [&str; 4] = [
    "-DABSL_USE_GOOGLETEST_HEAD=ON",
    "-DCMAKE_CXX_STANDARD_REQUIRED=ON",
    "-DCMAKE_CXX_STANDARD=20",
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    "-DABSL_MSVC_STATIC_RUNTIME=ON",
];
#[cfg(debug_assertions)]
static CONFIG_TYPE_FLAGS: &str = "-DCMAKE_BUILD_TYPE=Debug";
#[cfg(not(debug_assertions))]
static BUILD_TYPE_FLAGS: &str = "-DCMAKE_BUILD_TYPE=Release";

/// Windows Compile-Time flags
#[allow(dead_code)]
#[cfg(all(target_os = "windows", target_env = "msvc"))]
static MINIMUM_MSVC_VERSION: [u8; 2] = [19, 22]; // CMake supported C++20
const COMPILE_FLAGS: [&str; 4] = ["--build", ".", "--", "/p:Platform=x64"];
#[cfg(all(target_os = "windows", target_env = "msvc", debug_assertions))]
static COMPILE_TYPE_FLAGS: &str = "/p:Configuration=Debug";
#[cfg(all(target_os = "windows", target_env = "msvc", not(debug_assertions)))]
static COMPILE_TYPE_FLAGS: &str = "/p:Configuration=Release";

////
//// Macros
////

/// Macro that combines two sets of flags into a Vec<&str> by iterating, chaining, and copying.
macro_rules! combine_flags {
    ($primary:expr, $secondary:expr) => {{
        $primary
            .iter()
            .chain($secondary.iter())
            .copied()
            .collect::<Vec<&str>>()
    }};
}

////
//// Primary Build Loop
////

/// The real main function.
fn __build() {
    println!("Performing library build...");
    println!(" ");

    println!("---");
    println!("Confirming build environment...");
    basics_check();
    println!(" ");

    println!("---");
    println!("Creating build directory...");
    create_path(&BUILD_DIR).expect("Unable to create build directory");
    println!(" ");

    println!("---");
    println!("Obtaining abseil-cpp source code...");
    run_command("git", &["clone", ABSEIL_SRC], &BUILD_DIR);
    println!(" ");

    println!("---");
    println!("Creating Abseil build directory...");
    create_path(&ABSEIL_BUILD_DIR).expect("Unable to create Abseil build directory");
    println!(" ");

    println!("---");
    println!("Running CMake config...");
    let cmake_config_args = combine_flags!(CONFIG_FLAGS, [CONFIG_TYPE_FLAGS, ".."]);
    run_command("cmake", &cmake_config_args, &ABSEIL_BUILD_DIR);
    println!(" ");

    println!("---");
    println!("Running CMake build...");
    let cmake_build_args = combine_flags!(COMPILE_FLAGS, [COMPILE_TYPE_FLAGS]);
    run_command("cmake", &cmake_build_args, &ABSEIL_BUILD_DIR);

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
//// Helpers
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
    __build()
}
