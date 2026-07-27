use std::path::PathBuf;

/// Opens a file dialog.
#[cfg(target_arch = "wasm32")]
pub async fn open_file_dialog() -> Result<(PathBuf, String), String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Open File")
        .pick_file()
        .await
        .ok_or_else(|| "No file selected".to_string())?;

    let name = file.file_name();
    let bytes = file.read().await;
    let content = String::from_utf8(bytes)
        .map_err(|_| "Selected file is not valid UTF-8".to_string())?;

    Ok((PathBuf::from(name), content))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn open_file_dialog() -> Result<(PathBuf, String), String> {
    let file =
        rfd::AsyncFileDialog::new().set_title("Open File").pick_file().await;

    if let Some(file) = file {
        let path = file.path().to_path_buf();
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Unable to read file: {}", e))?;
        Ok((path, content))
    } else {
        Err("No file selected".to_string())
    }
}

/// Saves content to a file.
#[cfg(target_arch = "wasm32")]
pub async fn save_file(
    path: PathBuf,
    content: String,
) -> Result<PathBuf, String> {
    let filename =
        path.file_name().and_then(|n| n.to_str()).unwrap_or("demo.lua");

    let file = rfd::AsyncFileDialog::new()
        .set_title("Save")
        .set_file_name(filename)
        .save_file()
        .await
        .ok_or_else(|| "Save cancelled".to_string())?;

    file.write(content.as_bytes())
        .await
        .map_err(|e| format!("Unable to write file: {:?}", e))?;

    Ok(PathBuf::from(file.file_name()))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn save_file(
    path: PathBuf,
    content: String,
) -> Result<PathBuf, String> {
    std::fs::write(&path, content)
        .map_err(|e| format!("Unable to write file: {}", e))?;
    Ok(path)
}

/// Opens a save-as dialog.
#[cfg(target_arch = "wasm32")]
pub async fn save_file_as_dialog(content: String) -> Result<PathBuf, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Save As")
        .save_file()
        .await
        .ok_or_else(|| "Save cancelled".to_string())?;

    file.write(content.as_bytes())
        .await
        .map_err(|e| format!("Unable to write file: {:?}", e))?;

    Ok(PathBuf::from(file.file_name()))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn save_file_as_dialog(content: String) -> Result<PathBuf, String> {
    let file =
        rfd::AsyncFileDialog::new().set_title("Save As").save_file().await;

    if let Some(file) = file {
        let path = file.path().to_path_buf();
        std::fs::write(&path, content)
            .map_err(|e| format!("Unable to write file: {}", e))?;
        Ok(path)
    } else {
        Err("Save cancelled".to_string())
    }
}

/// Reads a file from the given path.
#[cfg(not(target_arch = "wasm32"))]
pub async fn read_file(path: PathBuf) -> Result<(PathBuf, String), String> {
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Unable to read file: {}", e))?;
    Ok((path, content))
}

#[cfg(not(target_arch = "wasm32"))]
fn reveal_file_with<E, F>(
    path: &std::path::Path,
    reveal: F,
) -> Result<(), String>
where
    E: std::fmt::Display,
    F: FnOnce(&std::path::Path) -> Result<(), E>,
{
    reveal(path).map_err(|error| {
        format!("Unable to reveal {}: {error}", path.display())
    })
}

/// Reveals a file in the platform file manager.
#[cfg(not(target_arch = "wasm32"))]
pub async fn reveal_in_file_manager(path: PathBuf) -> Result<PathBuf, String> {
    reveal_file_with(&path, |candidate| opener::reveal(candidate))?;
    Ok(path)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn test_reveal_adapter_forwards_path() {
        let path = PathBuf::from("/tmp/iced-code-editor/reveal.lua");
        let received = RefCell::new(None);

        let result = reveal_file_with(&path, |candidate: &std::path::Path| {
            *received.borrow_mut() = Some(candidate.to_path_buf());
            Ok::<_, std::io::Error>(())
        });

        assert_eq!(result, Ok(()));
        assert_eq!(received.into_inner(), Some(path));
    }

    #[test]
    fn test_reveal_adapter_reports_errors() {
        let path = PathBuf::from("/tmp/iced-code-editor/missing.lua");

        let result = reveal_file_with(&path, |_| {
            Err::<(), _>(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "test failure",
            ))
        });

        assert_eq!(
            result,
            Err(
                "Unable to reveal /tmp/iced-code-editor/missing.lua: test failure"
                    .to_string()
            )
        );
    }
}
