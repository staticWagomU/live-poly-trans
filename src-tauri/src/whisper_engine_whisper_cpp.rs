use crate::whisper_engine_backend::{WhisperBackendError, WhisperTranscription};
use std::{
    ffi::{c_char, CStr},
    path::{Path, PathBuf},
};

pub const WHISPER_CPP_SHIM_SYMBOLS: &[&str] = &[
    "lpt_whisper_backend_create",
    "lpt_whisper_backend_free",
    "lpt_whisper_backend_transcribe",
    "lpt_whisper_backend_free_transcript",
    "lpt_whisper_backend_last_error",
];

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

pub struct WhisperCppBackend {
    _library: libloading::Library,
}

impl WhisperCppBackend {
    pub fn from_library(path: &Path) -> Result<Self, WhisperBackendError> {
        let library = unsafe { libloading::Library::new(path) }.map_err(|error| {
            WhisperBackendError {
                message: format!("failed to load whisper cpp shim {}: {error}", path.display()),
            }
        })?;

        Ok(Self { _library: library })
    }
}

#[repr(C)]
pub struct LptWhisperTranscript {
    pub text: *const c_char,
    pub language: *const c_char,
    pub confidence: f64,
    pub has_confidence: u8,
}

pub unsafe fn transcript_from_ffi(
    transcript: &LptWhisperTranscript,
) -> Result<WhisperTranscription, WhisperBackendError> {
    if transcript.text.is_null() || transcript.language.is_null() {
        return Err(WhisperBackendError {
            message: "whisper cpp shim returned a null transcript string".to_string(),
        });
    }

    Ok(WhisperTranscription {
        text: unsafe { CStr::from_ptr(transcript.text) }
            .to_string_lossy()
            .into_owned(),
        language: unsafe { CStr::from_ptr(transcript.language) }
            .to_string_lossy()
            .into_owned(),
        confidence: (transcript.has_confidence != 0).then_some(transcript.confidence),
    })
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

    #[test]
    fn loading_a_missing_shim_reports_the_path() {
        let error =
            match WhisperCppBackend::from_library(Path::new("/missing/liblpt_whisper_backend.dylib"))
            {
                Ok(_) => panic!("missing shim unexpectedly loaded"),
                Err(error) => error,
            };

        assert!(error.message.contains("/missing/liblpt_whisper_backend.dylib"));
    }

    #[test]
    fn shim_symbol_names_match_the_c_abi_contract() {
        assert_eq!(
            WHISPER_CPP_SHIM_SYMBOLS,
            &[
                "lpt_whisper_backend_create",
                "lpt_whisper_backend_free",
                "lpt_whisper_backend_transcribe",
                "lpt_whisper_backend_free_transcript",
                "lpt_whisper_backend_last_error",
            ]
        );
    }

    #[test]
    fn converts_ffi_transcript_to_backend_transcription() {
        let text = std::ffi::CString::new("hello").unwrap();
        let language = std::ffi::CString::new("en").unwrap();
        let ffi = LptWhisperTranscript {
            text: text.as_ptr(),
            language: language.as_ptr(),
            confidence: 0.75,
            has_confidence: 1,
        };

        let transcript = unsafe { transcript_from_ffi(&ffi) }.unwrap();

        assert_eq!(transcript.text, "hello");
        assert_eq!(transcript.language, "en");
        assert_eq!(transcript.confidence, Some(0.75));
    }
}
