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
