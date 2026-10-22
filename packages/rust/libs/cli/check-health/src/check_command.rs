pub mod health;
pub mod health_report;

pub mod output {
    pub use bloomery_check_output::check_command::output::*;
    pub use bloomery_cli_output::*;
}

pub mod execution {
    pub use super::health_report::{health_discovery_value, health_policy_value};
}

pub mod model {
    pub use bloomery_check_support::check_command::model::*;
}

pub mod store {
    pub use bloomery_check_storage::check_command::store::*;
}

pub mod command {
    pub use bloomery_check_support::check_command::context::{
        CheckError, HealthContext as BaseHealthContext,
    };
    pub use bloomery_cli_types::{HealthArgs, HealthUpdateArgs};

    pub type CheckContext<'a> =
        BaseHealthContext<'a, dyn bloomery_check_catalog::check_command::catalog::NixBackend + 'a>;

    use crate::output;
    use std::io::Write;
    use std::process::ExitCode;

    pub fn write_page(
        json_mode: bool,
        color: output::ColorMode,
        bytes: Vec<u8>,
        stdout: &mut impl Write,
        stderr: &mut impl Write,
    ) -> ExitCode {
        let bytes = if json_mode {
            bytes
        } else {
            output::colorize_check(
                &String::from_utf8_lossy(&bytes),
                output::color_enabled(output::Stream::Stdout, false, color),
            )
            .into_bytes()
        };
        match stdout.write_all(&bytes).and_then(|()| stdout.flush()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                let _ = writeln!(stderr, "bloomery: unable to write check output: {error}");
                ExitCode::from(2)
            }
        }
    }

    pub fn report_error(
        json_mode: bool,
        error: &CheckError,
        run: Option<&str>,
        stdout: &mut impl Write,
        stderr: &mut impl Write,
    ) -> ExitCode {
        if json_mode {
            let mut document = output::error_document(error.code, &error.message, run);
            if let Some(run) = run {
                document["error"]["details"]["run"] = serde_json::Value::String(run.to_owned());
            }
            match output::compact_json(&document) {
                Ok(bytes) => {
                    let _ = stdout.write_all(&bytes).and_then(|()| stdout.flush());
                }
                Err(message) => {
                    let _ = writeln!(stderr, "bloomery: {message}");
                }
            }
        } else {
            let message = format!("error: {}", error.message);
            let rendered = output::colorize_error(
                &message,
                output::color_enabled(output::Stream::Stderr, false, output::ColorMode::Auto),
            );
            let _ = writeln!(stderr, "{rendered}");
        }
        ExitCode::from(2)
    }
}
