fn main() {
    // Lets debug builds find `binaries/examops-parser-<target-triple>` on any platform.
    println!(
        "cargo:rustc-env=TARGET_TRIPLE={}",
        std::env::var("TARGET").expect("cargo sets TARGET")
    );
    tauri_build::build()
}
