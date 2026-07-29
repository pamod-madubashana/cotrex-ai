use std::process::Command;
use std::time::Instant;

use crate::error::ExecutionError;
use crate::executor::Executor;
use crate::request::{ExecutionAction, ExecutionRequest};
use crate::result::ExecutionResult;

/// Executes OS commands using `std::process::Command`.
///
/// Spawns a child process, captures stdout and stderr, and waits for
/// completion. Non-zero exit codes are treated as successful execution
/// (the executor worked — the command just failed).
pub struct CommandExecutor;

impl Executor for CommandExecutor {
    fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionResult, ExecutionError> {
        let (program, working_directory) = match &request.action {
            ExecutionAction::CommandRun {
                command,
                working_directory,
            } => (command.as_str(), working_directory),
            other => {
                return Err(ExecutionError::Internal(format!(
                    "CommandExecutor received non-CommandRun action: {:?}",
                    other
                )));
            }
        };

        let start = Instant::now();

        #[cfg(not(windows))]
        let output = Command::new(program)
            .current_dir(working_directory)
            .output()
            .map_err(|e| {
                let summary = format!("command spawn failed: {}", e);
                ExecutionError::ExecutorFailed(summary)
            })?;

        #[cfg(windows)]
        let output = Command::new("cmd.exe")
            .arg("/C")
            .arg(program)
            .current_dir(working_directory)
            .output()
            .map_err(|e| {
                let summary = format!("command spawn failed: {}", e);
                ExecutionError::ExecutorFailed(summary)
            })?;

        let duration_ms = start.elapsed().as_millis() as u64;

        let exit_code = code_to_i32(&output.status);

        #[cfg(unix)]
        let error_summary = {
            use std::os::unix::process::ExitStatusExt;
            output
                .status
                .signal()
                .map(|sig| format!("process terminated by signal {}", sig))
        };

        #[cfg(not(unix))]
        let error_summary = {
            if !output.status.success() && exit_code.is_none() {
                Some("process terminated abnormally".into())
            } else {
                None
            }
        };

        let success = error_summary.is_none();

        Ok(ExecutionResult {
            execution_id: request.id,
            success,
            exit_code,
            duration_ms,
            error: error_summary,
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

fn code_to_i32(status: &std::process::ExitStatus) -> Option<i32> {
    status.code()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::ExecutionAction;
    use std::path::PathBuf;

    #[cfg(not(windows))]
    fn command_request(command: &str) -> ExecutionRequest {
        ExecutionRequest::new(
            ExecutionAction::CommandRun {
                command: command.into(),
                working_directory: PathBuf::from("."),
            },
            vec![],
        )
    }

    #[cfg(windows)]
    fn command_request(command: &str) -> ExecutionRequest {
        ExecutionRequest::new(
            ExecutionAction::CommandRun {
                command: command.into(),
                working_directory: PathBuf::from("."),
            },
            vec![],
        )
    }

    #[test]
    fn successful_command() {
        let executor = CommandExecutor;
        let req = command_request("echo hello");
        let result = executor.execute(&req).unwrap();

        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert!(!result.stdout.is_empty());
        assert!(result.error.is_none());
    }

    #[test]
    fn non_zero_exit_is_success() {
        let executor = CommandExecutor;
        let req = command_request("exit 5");
        let result = executor.execute(&req).unwrap();

        // Non-zero exit means the executor worked successfully
        assert!(result.success);
        assert_eq!(result.exit_code, Some(5));
        assert!(result.error.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn missing_executable() {
        let executor = CommandExecutor;
        let req = command_request("command_that_does_not_exist_xyz");
        let result = executor.execute(&req);

        match result {
            Err(ExecutionError::ExecutorFailed(summary)) => {
                assert!(summary.contains("command spawn failed:"));
            }
            other => panic!("expected ExecutorFailed, got: {:?}", other),
        }
    }

    #[cfg(windows)]
    #[test]
    fn missing_executable() {
        let executor = CommandExecutor;
        let req = command_request("command_that_does_not_exist_xyz");
        let result = executor.execute(&req).unwrap();

        // On Windows, cmd.exe spawns fine but returns non-zero exit code
        assert!(result.success);
        assert_eq!(result.exit_code, Some(1));
    }

    #[cfg(unix)]
    #[test]
    fn permission_failure() {
        let executor = CommandExecutor;
        let req = command_request("/");
        let result = executor.execute(&req);

        match result {
            Err(ExecutionError::ExecutorFailed(summary)) => {
                assert!(summary.contains("command spawn failed:"));
            }
            other => panic!("expected ExecutorFailed, got: {:?}", other),
        }
    }

    #[cfg(windows)]
    #[test]
    fn permission_failure() {
        let executor = CommandExecutor;
        let req = command_request("/");
        let result = executor.execute(&req).unwrap();

        // On Windows, cmd.exe spawns fine but returns non-zero exit code
        assert!(result.success);
        assert_eq!(result.exit_code, Some(1));
    }

    #[test]
    fn signal_termination() {
        // kill -9 $$ sends SIGKILL to current process; use sh -c to isolate
        #[cfg(unix)]
        {
            let executor = CommandExecutor;
            let req = command_request("sh -c \"kill -9 $$\"");
            let result = executor.execute(&req).unwrap();

            assert!(!result.success);
            assert!(result.error.is_some());
            let err = result.error.unwrap();
            assert!(err.contains("signal"), "expected signal info: {}", err);
        }
    }

    #[test]
    fn stdout_captured() {
        let executor = CommandExecutor;
        let req = command_request("echo alpha");
        let result = executor.execute(&req).unwrap();

        let stdout = String::from_utf8(result.stdout).unwrap();
        assert!(stdout.contains("alpha"));
    }

    #[test]
    fn stderr_captured() {
        let executor = CommandExecutor;
        // sh -c 'echo err >&2' writes to stderr
        #[cfg(unix)]
        let req = command_request("sh -c \"echo err >&2\"");
        #[cfg(windows)]
        let req = command_request("echo err 1>&2");
        let result = executor.execute(&req).unwrap();

        let stderr = String::from_utf8(result.stderr).unwrap();
        assert!(stderr.contains("err"));
    }

    #[test]
    fn non_command_run_action_returns_error() {
        use std::path::PathBuf;
        let executor = CommandExecutor;
        let req = ExecutionRequest::new(
            ExecutionAction::FileWrite {
                path: PathBuf::from("test.txt"),
                content: b"hello".to_vec(),
            },
            vec![],
        );
        let result = executor.execute(&req);

        match result {
            Err(ExecutionError::Internal(msg)) => {
                assert!(msg.contains("non-CommandRun"));
            }
            other => panic!("expected Internal error, got: {:?}", other),
        }
    }
}
