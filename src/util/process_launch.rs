use std::path::Path;
use std::process::Command;

pub fn launch_target_exe(path: &str) -> Result<u32, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Enter an EXE path.".to_owned());
    }

    let exe_path = Path::new(trimmed);
    if !exe_path.exists() {
        return Err(format!("File does not exist: {trimmed}"));
    }

    if !exe_path.is_file() {
        return Err(format!("Path is not a file: {trimmed}"));
    }

    let is_exe = exe_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
    if !is_exe {
        return Err("Target must be an .exe file.".to_owned());
    }

    let child = Command::new(exe_path)
        .spawn()
        .map_err(|e| format!("Failed to start process: {e}"))?;

    Ok(child.id())
}
