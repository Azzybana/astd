use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{
    fs,
    io::{Error, ErrorKind, Result},
    process::Command,
    str,
};

////
//// Statics and Constants
////

static MINIMUM_GIT_VERSION: [u8; 3] = [2, 40, 0]; // Still maintained
static MINIMUM_CMAKE_VERSION: [u8; 3] = [3, 31, 0]; // Features for C++20

const ABSEIL_SRC: &str = "https://github.com/abseil/abseil-cpp.git";

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

const BUILD_FLAGS: [&str; 3] = [
    // "-DABSL_BUILD_MONOLITHIC_SHARED_LIBS=ON", // For DLL, going to use static and wrap
    // "-DBUILD_SHARED_LIBS=ON",                // For DLL, going to use static and wrap
    "-DABSL_USE_GOOGLETEST_HEAD=ON",
    "-DCMAKE_CXX_STANDARD_REQUIRED=ON",
    "-DCMAKE_CXX_STANDARD=20",
];
#[cfg(debug_assertions)]
static BUILD_TYPE_FLAGS: &str = "-DCMAKE_BUILD_TYPE=Debug";
#[cfg(not(debug_assertions))]
static BUILD_TYPE_FLAGS: &str = "-DCMAKE_BUILD_TYPE=Release";

#[allow(dead_code)]
#[cfg(all(target_os = "windows", target_env = "msvc"))]
static MINIMUM_MSVC_VERSION: [u8; 2] = [19, 22]; // CMake supported C++20
const COMPILE_FLAGS: [&str; 4] = ["--build", ".", "--", "/p:Platform=x64"];
#[cfg(all(target_os = "windows", target_env = "msvc", debug_assertions))]
static COMPILE_TYPE_FLAGS: &str = "/p:Configuration=Debug";
#[cfg(all(target_os = "windows", target_env = "msvc", not(debug_assertions)))]
static COMPILE_TYPE_FLAGS: &str = "/p:Configuration=Release";

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
    create_build_dir(&BUILD_DIR).expect("Unable to create build directory");
    println!(" ");

    println!("---");
    println!("Obtaining abseil-cpp source code...");
    run_command("git", &["clone", ABSEIL_SRC], &BUILD_DIR);
    println!(" ");

    println!("---");
    println!("Creating Abseil build directory...");
    create_build_dir(&ABSEIL_BUILD_DIR).expect("Unable to create Abseil build directory");
    println!(" ");

    println!("---");
    println!("Running CMake config...");
    let cmake_config_args = BUILD_FLAGS
        .iter()
        .chain([BUILD_TYPE_FLAGS, ".."].iter())
        .copied()
        .collect::<Vec<&str>>();
    run_command("cmake", &cmake_config_args, &ABSEIL_BUILD_DIR);
    println!(" ");

    println!("---");
    println!("Running CMake build...");
    let cmake_build_args = COMPILE_FLAGS
        .iter()
        .chain([COMPILE_TYPE_FLAGS].iter())
        .copied()
        .collect::<Vec<&str>>();
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

fn create_build_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path).expect("Unable to create build directory");
    }
    Ok(())
}

/// Runs the specified command with arguments and returns its stdout as a UTF-8 String.
fn gather_output(command: &str, args: &[&str]) -> Result<String> {
    println!("Running: {} {:?}", command, args);
    let output = Command::new(command).args(args).output()?;
    String::from_utf8(output.stdout)
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid output!"))
}

fn is_version_valid(version_token: &str, req: [u8; 3]) -> bool {
    let numbers: Vec<u32> = version_token
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    let major = *numbers.get(0).unwrap_or(&0);
    let minor = *numbers.get(1).unwrap_or(&0);
    let patch = *numbers.get(2).unwrap_or(&0);
    (major, minor, patch) >= (req[0] as u32, req[1] as u32, req[2] as u32)
}

fn is_valid_git_version(version_str: &str) -> bool {
    let tokens: Vec<&str> = version_str.split_whitespace().collect();
    if let Some(&version_token) = tokens.get(2) {
        is_version_valid(version_token, MINIMUM_GIT_VERSION)
    } else {
        false
    }
}

fn is_valid_cmake_version(version_str: &str) -> bool {
    let tokens: Vec<&str> = version_str.split_whitespace().collect();
    if let Some(&version_token) = tokens.get(2) {
        is_version_valid(version_token, MINIMUM_CMAKE_VERSION)
    } else {
        false
    }
}

/// Executes a command in a given directory and panics if it fails.
fn run_command(command: &str, args: &[&str], dir: &Path) {
    let status = Command::new(command)
        .args(args)
        .current_dir(dir)
        .status()
        .expect(&format!("Failed to run command: {} {:?}", command, args));
    if !status.success() {
        panic!("Command {:?} failed with status: {:?}", command, status);
    }
}

////
//// Target Specifics
////

#[cfg(all(target_os = "windows", target_env = "msvc"))]
fn basics_check() {
    let git_version = gather_output("git", &["--version"]).expect("Failed to get git version");
    println!("Git output: {}", git_version);
    if !is_valid_git_version(&git_version) {
        println!("Please install or update git to the latest version.");
        std::process::exit(1);
    } else {
        println!("Git found. Proceeding.");
    }

    let cmake_version =
        gather_output("CMake", &["--version"]).expect("Failed to get CMake version");
    println!("CMake output: {}", cmake_version);
    if !is_valid_cmake_version(&cmake_version) {
        println!("Please install or update CMake to the latest version.");
        std::process::exit(1);
    } else {
        println!("CMake found. Proceeding.");
    }

    let caps =
        gather_output("CMake", &["-E", "capabilities"]).expect("Failed to run CMake capabilities");
    if caps.contains("Visual Studio 16 2019") {
        println!("Found Visual Studio 16 2019");
    } else if caps.contains("Visual Studio 17 2022") {
        println!("Found Visual Studio 17 2022");
    } else {
        println!(
            "No suitable Visual Studio version found.\nRun 'CMake -E capabilities' to see what CMake sees."
        );
        panic!("CMake reports no suitable MSVC version. Needs 2019+.");
    }
}

#[cfg(not(all(target_os = "windows", target_env = "msvc")))]
fn basics_check() {
    println!("Architecture not yet supported.");
    std::process::exit(1);
}

fn main() {
    __build()
}
