//! The native file choosers, as asynchronous tasks so the window never blocks.

use std::path::PathBuf;

/// The extension of a session file.
pub const EXTENSION: &str = "lasession";

/// Asks for a session file to open. `None` if the user gave up.
pub async fn pick_open(title: String, filter_name: String) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(title)
        .add_filter(filter_name, &[EXTENSION])
        .pick_file()
        .await
        .map(|handle| handle.path().to_path_buf())
}

/// Asks for the name of a session file to save to. `None` if the user gave up. The extension is
/// added when the user left it out.
pub async fn pick_save(title: String, filter_name: String, file_name: String) -> Option<PathBuf> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title(title)
        .set_file_name(file_name)
        .add_filter(filter_name, &[EXTENSION])
        .save_file()
        .await?;
    Some(with_extension(handle.path().to_path_buf()))
}

/// `path` with the session extension, unless it has one already.
fn with_extension(mut path: PathBuf) -> PathBuf {
    if path.extension().is_none() {
        path.set_extension(EXTENSION);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_is_added_only_when_missing() {
        assert_eq!(
            with_extension(PathBuf::from("a")),
            PathBuf::from("a.lasession")
        );
        assert_eq!(
            with_extension(PathBuf::from("a.lasession")),
            PathBuf::from("a.lasession")
        );
        assert_eq!(
            with_extension(PathBuf::from("a.data")),
            PathBuf::from("a.data")
        );
    }
}
