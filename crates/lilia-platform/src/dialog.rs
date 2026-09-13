use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileDialogRequest {
    pub title: Option<String>,
    pub initial_directory: Option<PathBuf>,
    pub filters: Vec<FileFilter>,
    pub select_directories: bool,
    pub multiple: bool,
}

/// Blocks the calling thread on the OS picker. An empty result means the user
/// cancelled, which is not an error.
///
/// Never call this on an event-loop thread: while the picker is up the window
/// system re-enters event dispatch, which panics winit's re-entrancy guard or
/// wedges the loop. Run it on a dedicated thread and deliver the result as a
/// message.
pub fn pick(request: FileDialogRequest) -> Vec<PathBuf> {
    let select_directories = request.select_directories;
    let multiple = request.multiple;
    let dialog = build(request);
    match (select_directories, multiple) {
        (true, true) => dialog.pick_folders().unwrap_or_default(),
        (true, false) => dialog.pick_folder().into_iter().collect(),
        (false, true) => dialog.pick_files().unwrap_or_default(),
        (false, false) => dialog.pick_file().into_iter().collect(),
    }
}

pub fn save(request: FileDialogRequest, suggested_filename: &str) -> Vec<PathBuf> {
    build(request)
        .set_file_name(suggested_filename)
        .save_file()
        .into_iter()
        .collect()
}

fn build(request: FileDialogRequest) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new();
    if let Some(title) = request.title {
        dialog = dialog.set_title(title);
    }
    if let Some(initial_directory) = request.initial_directory {
        dialog = dialog.set_directory(initial_directory);
    }
    for filter in request.filters {
        let extensions = filter
            .extensions
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        dialog = dialog.add_filter(filter.name, &extensions);
    }
    dialog
}
