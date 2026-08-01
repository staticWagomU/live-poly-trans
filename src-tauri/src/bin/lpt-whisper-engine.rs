use live_poly_trans_lib::whisper_engine_protocol::{engine_output_line, parse_engine_input_line};
use live_poly_trans_lib::whisper_engine_sidecar::{SidecarAction, WhisperEngineSidecar};
use live_poly_trans_lib::whisper_engine_whisper_cpp::{
    first_existing_library_candidate, whisper_cpp_library_candidates, WhisperCppBackend,
};
use std::io::{self, BufRead, Write};
use std::path::Path;

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut sidecar = WhisperEngineSidecar::with_backend(
        live_poly_trans_lib::whisper_engine_sidecar::DEFAULT_RING_BUFFER_SAMPLES,
        match whisper_cpp_backend() {
            Ok(backend) => backend,
            Err(error) => {
                eprintln!("{}", error.message);
                std::process::exit(1);
            }
        },
    );

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                eprintln!("failed to read whisper engine input: {error}");
                std::process::exit(1);
            }
        };

        let input = match parse_engine_input_line(&line) {
            Ok(input) => input,
            Err(error) => {
                eprintln!("failed to parse whisper engine input: {error}");
                std::process::exit(1);
            }
        };

        match sidecar.handle_input(input) {
            SidecarAction::Continue(Some(output)) => {
                let line = match engine_output_line(&output) {
                    Ok(line) => line,
                    Err(error) => {
                        eprintln!("failed to encode whisper engine output: {error}");
                        std::process::exit(1);
                    }
                };
                if let Err(error) = writeln!(stdout, "{line}") {
                    eprintln!("failed to write whisper engine output: {error}");
                    std::process::exit(1);
                }
                if let Err(error) = stdout.flush() {
                    eprintln!("failed to flush whisper engine output: {error}");
                    std::process::exit(1);
                }
            }
            SidecarAction::Continue(None) => {}
            SidecarAction::Shutdown => break,
        }
    }
}

fn whisper_cpp_backend(
) -> Result<WhisperCppBackend, live_poly_trans_lib::whisper_engine_backend::WhisperBackendError> {
    let executable_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| Path::new(".").to_path_buf());
    let candidates = whisper_cpp_library_candidates(
        &executable_dir,
        Path::new(env!("CARGO_MANIFEST_DIR")),
        std::env::consts::OS,
    );
    let Some(path) = first_existing_library_candidate(&candidates) else {
        return Err(
            live_poly_trans_lib::whisper_engine_backend::WhisperBackendError {
                message: format!("whisper cpp shim was not found in {:?}", candidates),
            },
        );
    };

    WhisperCppBackend::from_library(&path)
}
