//! `aoe repl` launches an agent's raw interactive REPL.

use anyhow::{bail, Result};
use clap::Args;
use std::io::ErrorKind;
use std::process::{Command, Stdio};

use crate::session::profile_config;

#[derive(Debug, Args)]
pub struct ReplArgs {
    /// Override the configured REPL command. The default is
    /// `codex -s danger-full-access`, configurable via `[repl].command`.
    #[arg(long, value_name = "COMMAND")]
    pub cmd: Option<String>,

    /// Additional arguments to append after the command. Use `--` before them.
    #[arg(last = true, trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Launch the configured command with the current terminal attached.
pub fn run(profile: &str, args: ReplArgs) -> Result<()> {
    let config = profile_config::resolve_config_or_warn(profile);
    let command = args.cmd.as_deref().unwrap_or(&config.repl.command);
    let mut argv = shell_words::split(command)
        .map_err(|e| anyhow::anyhow!("Invalid REPL command {command:?}: {e}"))?;
    if argv.is_empty() {
        bail!("REPL command is empty; configure [repl].command or pass --cmd <command>");
    }

    let program = argv.remove(0);
    let status = Command::new(&program)
        .args(argv)
        .args(args.args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                anyhow::anyhow!(
                    "REPL command binary not found: {program:?}; configure [repl].command or pass --cmd <command>"
                )
            } else {
                anyhow::anyhow!("Failed to start REPL command {program:?}: {e}")
            }
        })?;

    // `main` currently returns anyhow::Result, so process::exit is the only
    // way for this interactive command to preserve every child exit code.
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(test)]
mod tests {
    #[test]
    fn command_arguments_are_shell_word_parsed() {
        let words = shell_words::split("sh -c 'printf %s hello'").expect("valid command");
        assert_eq!(words, ["sh", "-c", "printf %s hello"]);
    }
}
