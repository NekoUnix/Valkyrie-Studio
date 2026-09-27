//! Per-user runtime files are never addressed through the build machine's
//! Cargo source path. Set VALKYRIE_HOME to share a portable profile deliberately.
use anyhow::{Result, ensure};
use std::{env, path::PathBuf};

pub fn data_dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os("VALKYRIE_HOME") {
        let path = PathBuf::from(path);
        ensure!(path.is_absolute(), "VALKYRIE_HOME must be an absolute path");
        return Ok(path);
    }
    if cfg!(debug_assertions) {
        return Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    }
    #[cfg(target_os = "windows")]
    {
        return Ok(PathBuf::from(
            env::var_os("APPDATA").ok_or_else(|| anyhow::anyhow!("APPDATA is missing"))?,
        )
        .join("ValkyrieStudio"));
    }
    #[cfg(target_os = "macos")]
    {
        return Ok(PathBuf::from(
            env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is missing"))?,
        )
        .join("Library/Application Support/ValkyrieStudio"));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(xdg) = env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(xdg).join("valkyrie-studio"));
        }
        return Ok(PathBuf::from(
            env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("HOME is missing"))?,
        )
        .join(".local/share/valkyrie-studio"));
    }
    #[allow(unreachable_code)]
    Err(anyhow::anyhow!("Unsupported operating system"))
}
