use crate::whisper_engine_backend::{
    WhisperBackend, WhisperBackendConfig, WhisperBackendError, WhisperTranscription,
};
use std::{
    ffi::{c_char, c_void, CStr, CString},
    ptr::NonNull,
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
    create: CreateFn,
    free: FreeFn,
    transcribe: TranscribeFn,
    free_transcript: FreeTranscriptFn,
    last_error: LastErrorFn,
    handle: Option<NonNull<c_void>>,
}

type CreateFn = unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void;
type FreeFn = unsafe extern "C" fn(*mut c_void);
type TranscribeFn =
    unsafe extern "C" fn(*mut c_void, *const f32, usize, *mut LptWhisperTranscript) -> i32;
type FreeTranscriptFn = unsafe extern "C" fn(*mut LptWhisperTranscript);
type LastErrorFn = unsafe extern "C" fn() -> *const c_char;

impl WhisperCppBackend {
    pub fn from_library(path: &Path) -> Result<Self, WhisperBackendError> {
        let library = unsafe { libloading::Library::new(path) }.map_err(|error| {
            WhisperBackendError {
                message: format!("failed to load whisper cpp shim {}: {error}", path.display()),
            }
        })?;

        let create = unsafe { load_symbol::<CreateFn>(&library, "lpt_whisper_backend_create")? };
        let free = unsafe { load_symbol::<FreeFn>(&library, "lpt_whisper_backend_free")? };
        let transcribe =
            unsafe { load_symbol::<TranscribeFn>(&library, "lpt_whisper_backend_transcribe")? };
        let free_transcript = unsafe {
            load_symbol::<FreeTranscriptFn>(&library, "lpt_whisper_backend_free_transcript")?
        };
        let last_error =
            unsafe { load_symbol::<LastErrorFn>(&library, "lpt_whisper_backend_last_error")? };

        Ok(Self {
            _library: library,
            create,
            free,
            transcribe,
            free_transcript,
            last_error,
            handle: None,
        })
    }
}

impl WhisperBackend for WhisperCppBackend {
    fn load_model(&mut self, config: WhisperBackendConfig) -> Result<(), WhisperBackendError> {
        if let Some(handle) = self.handle.take() {
            unsafe { (self.free)(handle.as_ptr()) };
        }

        let model_path = CString::new(config.model_path).map_err(|_| WhisperBackendError {
            message: "model path contains a NUL byte".to_string(),
        })?;
        let language = CString::new(config.language).map_err(|_| WhisperBackendError {
            message: "language contains a NUL byte".to_string(),
        })?;
        let handle = unsafe { (self.create)(model_path.as_ptr(), language.as_ptr()) };
        let Some(handle) = NonNull::new(handle) else {
            return Err(self.last_error_message("failed to create whisper cpp backend"));
        };

        self.handle = Some(handle);
        Ok(())
    }

    fn transcribe(&mut self, samples: &[f32]) -> Result<Option<WhisperTranscription>, WhisperBackendError> {
        if samples.is_empty() {
            return Ok(None);
        }
        let Some(handle) = self.handle else {
            return Err(WhisperBackendError {
                message: "whisper cpp backend model is not loaded".to_string(),
            });
        };

        let mut transcript = LptWhisperTranscript {
            text: std::ptr::null(),
            language: std::ptr::null(),
            confidence: 0.0,
            has_confidence: 0,
        };
        let status = unsafe {
            (self.transcribe)(
                handle.as_ptr(),
                samples.as_ptr(),
                samples.len(),
                &mut transcript,
            )
        };
        match status {
            1.. => {
                let converted = unsafe { transcript_from_ffi(&transcript) };
                unsafe { (self.free_transcript)(&mut transcript) };
                converted.map(Some)
            }
            0 => Ok(None),
            _ => Err(self.last_error_message("whisper cpp transcription failed")),
        }
    }
}

impl Drop for WhisperCppBackend {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            unsafe { (self.free)(handle.as_ptr()) };
        }
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

unsafe fn load_symbol<T: Copy>(
    library: &libloading::Library,
    name: &str,
) -> Result<T, WhisperBackendError> {
    let symbol = unsafe { library.get::<T>(format!("{name}\0").as_bytes()) }.map_err(|error| {
        WhisperBackendError {
            message: format!("failed to load whisper cpp shim symbol {name}: {error}"),
        }
    })?;
    Ok(*symbol)
}

impl WhisperCppBackend {
    fn last_error_message(&self, fallback: &str) -> WhisperBackendError {
        let message = unsafe {
            let ptr = (self.last_error)();
            if ptr.is_null() {
                fallback.to_string()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        };
        WhisperBackendError { message }
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
