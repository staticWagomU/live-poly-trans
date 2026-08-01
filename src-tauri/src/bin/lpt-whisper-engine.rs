use live_poly_trans_lib::whisper_engine_protocol::{engine_output_line, parse_engine_input_line};
use live_poly_trans_lib::whisper_engine_sidecar::{SidecarAction, WhisperEngineSidecar};
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut sidecar = WhisperEngineSidecar::with_default_capacity();

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
