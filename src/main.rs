use std::process::ExitCode;

fn main() -> ExitCode {
    match mh::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {}", mh::errors::format_user_error(&error));
            ExitCode::FAILURE
        }
    }
}
