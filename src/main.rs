use std::process::ExitCode;

fn main() -> ExitCode {
    match http_probe::run_from_env() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(match error {
                http_probe::error::AppError::InvalidConfiguration(_) => 2,
                http_probe::error::AppError::CurlSetup { .. }
                | http_probe::error::AppError::TimingRead { .. }
                | http_probe::error::AppError::Internal(_) => 3,
            })
        }
    }
}
