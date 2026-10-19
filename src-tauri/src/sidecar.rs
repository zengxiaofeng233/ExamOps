//! Locating and launching the bundled Python document processor.
//!
//! Release builds ship the PyInstaller output next to the main executable
//! (`examops-parser.exe` + `_internal/`, see `tauri.conf.json`). Debug builds
//! use `src-tauri/binaries/examops-parser-<target-triple>` produced by
//! `npm run sidecar:build`; `EXAMOPS_PARSER` can point debug builds elsewhere.

use std::path::PathBuf;

pub fn parser_executable() -> Result<PathBuf, String> {
    let suffix = std::env::consts::EXE_SUFFIX;
    if cfg!(debug_assertions) {
        if let Some(custom) = std::env::var_os("EXAMOPS_PARSER") {
            return Ok(PathBuf::from(custom));
        }
        return Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(format!("examops-parser-{}{suffix}", env!("TARGET_TRIPLE"))));
    }
    Ok(std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name(format!("examops-parser{suffix}")))
}

/// Builds a command for the parser that never opens a console window on Windows
/// and is killed if the owning future is dropped (e.g. the app exits).
pub fn command() -> Result<tokio::process::Command, String> {
    let executable = parser_executable()?;
    let mut command = tokio::process::Command::new(executable);
    command.kill_on_drop(true);
    command.arg("--parent-pid").arg(std::process::id().to_string());
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    Ok(command)
}
