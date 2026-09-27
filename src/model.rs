use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct ModelAssets {
    pub manifest: PathBuf,
    pub moc: Vec<u8>,
    pub textures: Vec<PathBuf>,
    pub physics: Option<Value>,
    pub vtube: Option<Value>,
}

impl ModelAssets {
    pub fn open(path: &Path) -> Result<Self> {
        let manifest = path
            .canonicalize()
            .with_context(|| format!("Model manifest not found: {}", path.display()))?;
        ensure!(
            manifest
                .file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(".model3.json")),
            "Select a .model3.json file"
        );
        let root = manifest.parent().context("Model manifest has no parent")?;
        let metadata = fs::metadata(&manifest)?;
        ensure!(
            metadata.len() <= 20 * 1024 * 1024,
            "Model manifest exceeds 20 MiB"
        );
        let doc: Value =
            serde_json::from_slice(&fs::read(&manifest)?).context("Invalid model manifest JSON")?;
        let references = &doc["FileReferences"];
        let moc = references["Moc"]
            .as_str()
            .context("Model manifest lacks Moc")?;
        let texture_names = references["Textures"]
            .as_array()
            .context("Model manifest lacks Textures")?;
        ensure!(
            (1..=32).contains(&texture_names.len()),
            "Expected 1–32 texture atlases"
        );
        let moc_path = confined(root, moc)?;
        ensure!(
            fs::metadata(&moc_path)?.len() <= 1280 * 1024 * 1024,
            "Model data exceeds 1280 MiB"
        );
        let textures = texture_names
            .iter()
            .map(|item| {
                confined(
                    root,
                    item.as_str().context("Texture path must be a string")?,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let physics = if let Some(path) = references["Physics"].as_str() {
            let path = confined(root, path)?;
            ensure!(
                fs::metadata(&path)?.len() <= 20 * 1024 * 1024,
                "Physics file exceeds 20 MiB"
            );
            Some(serde_json::from_slice(&fs::read(path)?).context("Invalid physics JSON")?)
        } else {
            None
        };
        let vtube = manifest
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|name| name.strip_suffix(".model3.json"))
            .map(|stem| root.join(format!("{stem}.vtube.json")))
            .filter(|path| path.is_file())
            .filter(|path| fs::metadata(path).is_ok_and(|meta| meta.len() <= 20 * 1024 * 1024))
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        Ok(Self {
            manifest,
            moc: fs::read(moc_path)?,
            textures,
            physics,
            vtube,
        })
    }
}

fn confined(root: &Path, relative: &str) -> Result<PathBuf> {
    ensure!(
        !relative.is_empty() && relative.len() <= 4096,
        "Invalid model asset path"
    );
    let joined = root.join(relative);
    let canonical = joined
        .canonicalize()
        .with_context(|| format!("Missing model asset: {}", joined.display()))?;
    ensure!(
        canonical.starts_with(root),
        "Model asset escapes its folder"
    );
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_non_manifest() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert!(
            ModelAssets::open(&path)
                .unwrap_err()
                .to_string()
                .contains("model3.json")
        );
    }
}
