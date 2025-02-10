use std::{
    fs,
    io::{Error, ErrorKind, Result},
    path::Path,
    process::Command,
    str,
};

/// Statics and Constants
static MINIMUM_GIT_VERSION: [u8; 3] = [2, 40, 0]; // Still maintained
static MINIMUM_CMAKE_VERSION: [u8; 3] = [3, 31, 0]; // Features for C++20

static BUILD_DIR: &str = "build/";
static OUTPUT_DIR: &str = "external/";
static ABSEIL_SRC: &str = "https://github.com/abseil/abseil-cpp.git";

#[allow(dead_code)]
#[cfg(all(target_os = "windows", target_env = "msvc"))]
static MINIMUM_MSVC_VERSION: [u8; 2] = [19, 22]; // CMake supported C++20

/// Runs the specified command with arguments and returns its stdout as a UTF-8 String.
///
/// # Arguments
///
/// * `command` - The command to run (e.g., "git").
/// * `args` - A slice of arguments to pass to the command.
///
/// # Returns
///
/// * `Ok(String)` containing the command output if successful.
/// * `Err(Error)` if the command fails or the output is not valid UTF-8.
fn gather_output(command: &str, args: &[&str]) -> Result<String> {
    println!("Running: {} {:?}", command, args);
    let output = Command::new(command).args(args).output()?;
    String::from_utf8(output.stdout)
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid output!"))
}

/// Compares a version string (in the form "X.Y[.Z]") against a minimum required version.
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
    // Expecting "git version 2.42.0"
    let tokens: Vec<&str> = version_str.split_whitespace().collect();
    if let Some(&version_token) = tokens.get(2) {
        is_version_valid(version_token, MINIMUM_GIT_VERSION)
    } else {
        false
    }
}

fn is_valid_cmake_version(version_str: &str) -> bool {
    // Expecting "cmake version 3.31.0"
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

/// Windows-specific checks: Git, CMake, and MSVC version via CMake capabilities.
#[cfg(all(target_os = "windows", target_env = "msvc"))]
fn basics_check() {
    // Git version check
    let git_version = gather_output("git", &["--version"]).expect("Failed to get git version");
    println!("Git output: {}", git_version);
    if !is_valid_git_version(&git_version) {
        println!("Please install or update git to the latest version.");
        std::process::exit(1);
    } else {
        println!("Git found. Proceeding.");
    }

    // CMake version check
    let cmake_version =
        gather_output("CMake", &["--version"]).expect("Failed to get CMake version");
    println!("CMake output: {}", cmake_version);
    if !is_valid_cmake_version(&cmake_version) {
        println!("Please install or update CMake to the latest version.");
        std::process::exit(1);
    } else {
        println!("CMake found. Proceeding.");
    }

    // Check for MSVC support via CMake capabilities
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
    println!("cargo:rerun-if-changed=build.rs");
    basics_check();

    // Create the build directory
    println!("1. Creating build directory");
    fs::create_dir_all(BUILD_DIR).expect("Unable to create build directory");

    // Clone the repository if needed
    println!("2. Cloning abseil-cpp repository if not present");
    let abseil_dir = Path::new(BUILD_DIR).join("abseil-cpp");
    if !abseil_dir.exists() {
        run_command("git", &["clone", ABSEIL_SRC], Path::new(BUILD_DIR));
    }

    // Create the CMake build directory inside abseil-cpp
    println!("3. Creating build directory in abseil-cpp");
    let cmake_build_dir = abseil_dir.join(BUILD_DIR);
    fs::create_dir_all(&cmake_build_dir).expect("Unable to create abseil build directory");

    // Prepare common CMake flags
    let common_flags = [
        // "-DABSL_BUILD_MONOLITHIC_SHARED_LIBS=ON",
        // "-DBUILD_SHARED_LIBS=ON",
        "-DABSL_USE_GOOGLETEST_HEAD=ON",
        "-DCMAKE_CXX_STANDARD_REQUIRED=ON",
        "-DCMAKE_CXX_STANDARD=20",
    ];
    let build_type_flag = if cfg!(debug_assertions) {
        "-DCMAKE_BUILD_TYPE=Debug"
    } else {
        "-DCMAKE_BUILD_TYPE=Release"
    };

    let cmake_config_args: Vec<&str> = common_flags
        .iter()
        .chain([build_type_flag, "../"].iter())
        .copied()
        .collect();

    // Configure the build using CMake
    println!("4. Configuring the build using cmake");
    run_command("cmake", &cmake_config_args, &cmake_build_dir);

    // Build the project using CMake (use appropriate flags for configuration)
    println!("5. Building the project using cmake");
    let cmake_build_args = if cfg!(debug_assertions) {
        vec![
            "--build",
            ".",
            "--",
            "/p:Configuration=Debug",
            "/p:Platform=x64",
        ]
    } else {
        vec![
            "--build",
            ".",
            "--",
            "/p:Configuration=Release",
            "/p:Platform=x64",
        ]
    };
    run_command("cmake", &cmake_build_args, &cmake_build_dir);

    // Move the built DLL to the project root
    println!("6. Moving the built DLL to the project root");
    if cfg!(debug_assertions) {
        let source = cmake_build_dir.join("bin/Release/abseil_dll.dll");
        let destination = Path::new(OUTPUT_DIR).join("abseil.dll");
    //    fs::rename(&source, &destination).expect("Failed to move DLL file");
    } else {
        let source = cmake_build_dir.join("bin/Debug/abseil_dll.dll");
        let destination = Path::new(OUTPUT_DIR).join("abseil_d.dll");
        //    fs::rename(&source, &destination).expect("Failed to move DLL file");
        let source = cmake_build_dir.join("bin/Debug/abseil_dll.pdb");
        let destination = Path::new(OUTPUT_DIR).join("abseil_d.pdb");
        //    fs::rename(&source, &destination).expect("Failed to move PDB file");
    }
    println!("Build script completed successfully.");
}
