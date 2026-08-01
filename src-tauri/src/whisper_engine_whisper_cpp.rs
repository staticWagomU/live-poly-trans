pub fn whisper_cpp_library_file_name(target_os: &str) -> &'static str {
    match target_os {
        "macos" => "liblpt_whisper_backend.dylib",
        "windows" => "lpt_whisper_backend.dll",
        _ => "liblpt_whisper_backend.so",
    }
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
}
