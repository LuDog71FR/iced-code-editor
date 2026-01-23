use std::path::PathBuf;

/// Opens a file dialog.
pub async fn open_file_dialog() -> Result<(PathBuf, String), String> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("Lua Files", &["lua"])
        .add_filter("All Files", &["*"])
        .set_title("Open Lua File")
        .pick_file()
        .await;

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
pub async fn save_file(
    path: PathBuf,
    content: String,
) -> Result<PathBuf, String> {
    std::fs::write(&path, content)
        .map_err(|e| format!("Unable to write file: {}", e))?;
    Ok(path)
}

/// Opens a save-as dialog.
pub async fn save_file_as_dialog(content: String) -> Result<PathBuf, String> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("Lua Files", &["lua"])
        .set_title("Save As")
        .save_file()
        .await;

    if let Some(file) = file {
        let path = file.path().to_path_buf();
        std::fs::write(&path, content)
            .map_err(|e| format!("Unable to write file: {}", e))?;
        Ok(path)
    } else {
        Err("Save cancelled".to_string())
    }
}
