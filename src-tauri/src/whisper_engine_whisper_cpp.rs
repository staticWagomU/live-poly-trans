use std::path::{Path, PathBuf};

pub fn whisper_cpp_library_file_name(target_os: &str) -> &'static str {
    match target_os {
        "macos" => "liblpt_whisper_backend.dylib",
        "windows" => "lpt_whisper_backend.dll",
        _ => "liblpt_whisper_backend.so",
    }
}

pub fn whisper_cpp_library_candidates(
    executable_dir: &Path,
    manifest_dir: &Path,
    target_os: &str,
) -> Vec<PathBuf> {
    let file_name = whisper_cpp_library_file_name(target_os);
    vec![
        executable_dir.join(file_name),
        manifest_dir.join("binaries").join(file_name),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_platform_specific_whisper_cpp_shim_library_names() {
        assert_eq!(whisper_cpp_library_file_name("macos"), "liblpt_whisper_backend.dylib");
        assert_eq!(whisper_cpp_library_file_name("windows"), "lpt_whisper_backend.dll");
        assert_eq!(whisper_cpp_library_file_name("linux"), "liblpt_whisper_backend.so");
    }

    #[test]
    fn library_candidates_prefer_the_executable_directory_then_dev_binaries() {
        assert_eq!(
            whisper_cpp_library_candidates(
                Path::new("/app/Contents/MacOS"),
                Path::new("/repo/src-tauri"),
                "macos",
            ),
            vec![
                PathBuf::from("/app/Contents/MacOS/liblpt_whisper_backend.dylib"),
                PathBuf::from("/repo/src-tauri/binaries/liblpt_whisper_backend.dylib"),
            ]
        );
    }
}
