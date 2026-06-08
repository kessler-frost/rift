use async_channel::Sender;
use rift_util::path::ShellFamily;

use crate::terminal::model::session::command_executor::{
    InBandCommand, InBandCommandCancelledEvent,
};
use crate::terminal::model::tmux::commands::TmuxCommand;
use crate::terminal::shell::ShellType;

/// Set of events sent by command executors.
pub enum ExecutorCommandEvent {
    /// The command should be executed.
    ExecuteCommand {
        command: InBandCommand,
        /// A Sender that can be used to signal that the command has been cancelled.
        /// Lets us unblock the command in the executor.
        cancel_tx: Sender<InBandCommandCancelledEvent>,
    },
    ExecuteTmuxCommand(TmuxCommand),
    /// The command identified by `id` should be cancelled.
    CancelCommand {
        id: String,
    },
}

pub fn shell_escape_single_quotes(command: &str, shell_type: ShellType) -> String {
    match shell_type {
        ShellType::Fish => {
            // Backslash-escape single quotes for Fish.
            command.replace('\'', r"\'")
        }
        ShellType::PowerShell => {
            // In powershell we escape single quotes using two single quotes ''
            command.replace('\'', "''")
        }
        _ => {
            // For Bash and Zsh, replace each single quote with a '"'"' sequence.
            // The first single quote completes the single quoted string to the left,
            // the next three characters: "'" evaluate to a literal single quote in
            // bash/zsh, and then the final single quote starts a new single-quoted
            // string to the right. Effectively, this concatenates the left
            // single-quoted string, a literal single quote char, and the right
            // single-quoted string.
            command.replace('\'', r#"'"'"'"#)
        }
    }
}

/// Serializes `(name, value)` pairs into a shell-specific string of constant
/// environment-variable assignments (e.g. `PATH=...` for bash/zsh, `set -x PATH ...;`
/// for fish). Recovered inline (constant-only) from the deleted cloud env-vars
/// serializer; used to propagate PATH/cwd into WSL and remote (ssh) sessions.
pub fn serialize_constant_vars_for_shell<'s>(
    pairs: impl IntoIterator<Item = (&'s str, &'s str)>,
    shell_type: ShellType,
) -> String {
    let shell_family: ShellFamily = shell_type.into();
    let (prefix, separator, postfix, delimiter) = match shell_type {
        ShellType::Fish => ("set -x ", " ", ";", " "),
        ShellType::Bash | ShellType::Zsh => ("", "=", "", " "),
        ShellType::PowerShell => ("$env:", " = ", ";", " "),
    };
    pairs
        .into_iter()
        .map(|(name, value)| {
            let serialized_value = match shell_family {
                ShellFamily::Posix => shell_family.escape(value).into_owned(),
                ShellFamily::PowerShell => format!("'{}'", value.replace('\'', "''")),
            };
            format!(
                "{prefix}{}{separator}{serialized_value}{postfix}",
                shell_family.escape(name),
            )
        })
        .collect::<Vec<_>>()
        .join(delimiter)
}
