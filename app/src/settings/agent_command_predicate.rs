//! Local, offline `AgentModeCommandExecutionPredicate` plus the default command
//! allowlist/denylist.
//!
//! These are pure local data types used to express the agent-mode command allow/deny lists in
//! settings. They carry no network code; this module defines them directly in the app crate.

use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Predicate types to match commands that can be executed by Agent Mode.
#[derive(Debug, Serialize, Deserialize, Clone)]
enum AgentModeCommandExecutionPredicateType {
    /// A regex with start (`^`) and end (`$`) anchors.
    #[serde(with = "serde_regex")]
    AnchoredRegex(Regex),
}

impl AgentModeCommandExecutionPredicateType {
    fn new_regex(regex: &str) -> Result<Self, regex::Error> {
        let anchored_regex = Regex::new(&format!("^{regex}$"))?;
        Ok(Self::AnchoredRegex(anchored_regex))
    }

    fn matches(&self, cmd: &str) -> bool {
        match self {
            Self::AnchoredRegex(regex) => regex.is_match(cmd),
        }
    }
}

impl PartialEq for AgentModeCommandExecutionPredicateType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::AnchoredRegex(a), Self::AnchoredRegex(b)) => {
                // Indexing is safe: both strings always carry the surrounding anchors.
                let a_unanchored = &a.as_str()[1..a.as_str().len() - 1];
                let b_unanchored = &b.as_str()[1..b.as_str().len() - 1];
                a_unanchored == b_unanchored
            }
        }
    }
}

impl std::fmt::Display for AgentModeCommandExecutionPredicateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AnchoredRegex(regex) => {
                write!(f, "{}", &regex.as_str()[1..regex.as_str().len() - 1])
            }
        }
    }
}

/// A wrapper around [`AgentModeCommandExecutionPredicateType`] to enforce the use of the provided
/// constructors rather than direct construction of the variants.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(transparent)]
pub struct AgentModeCommandExecutionPredicate(AgentModeCommandExecutionPredicateType);

impl schemars::JsonSchema for AgentModeCommandExecutionPredicate {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("AgentModeCommandExecutionPredicate")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        generator.subschema_for::<String>()
    }
}

impl AgentModeCommandExecutionPredicate {
    pub fn new_regex(regex: &str) -> Result<Self, regex::Error> {
        Ok(Self(AgentModeCommandExecutionPredicateType::new_regex(
            regex,
        )?))
    }

    #[cfg_attr(
        not(any(test, feature = "integration_tests", feature = "test-util")),
        allow(dead_code)
    )]
    pub fn matches(&self, cmd: &str) -> bool {
        self.0.matches(cmd)
    }
}

impl std::fmt::Display for AgentModeCommandExecutionPredicate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl settings_value::SettingsValue for AgentModeCommandExecutionPredicate {
    fn to_file_value(&self) -> serde_json::Value {
        serde_json::Value::String(self.to_string())
    }

    fn from_file_value(value: &serde_json::Value) -> Option<Self> {
        value.as_str().and_then(|s| Self::new_regex(s).ok())
    }
}

lazy_static! {
    static ref OPTIONAL_ARGS_REGEX: Regex =
        Regex::new(r"(\s.*)?").expect("Can parse optional args regex");
}

cfg_if::cfg_if! {
    if #[cfg(test)] {
        lazy_static! {
            // Compiling the default regexes is slow in unoptimized builds, so use empty lists in tests.
            pub static ref DEFAULT_COMMAND_EXECUTION_ALLOWLIST: Vec<AgentModeCommandExecutionPredicate> = vec![];
            pub static ref DEFAULT_COMMAND_EXECUTION_DENYLIST: Vec<AgentModeCommandExecutionPredicate> = vec![];
        }
    } else {
        lazy_static! {
            pub static ref DEFAULT_COMMAND_EXECUTION_ALLOWLIST: Vec<AgentModeCommandExecutionPredicate> = vec![
                AgentModeCommandExecutionPredicate::new_regex(&format!("cat{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default cat rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("echo{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default echo rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex("find .*").expect("Can parse default find rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("grep{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default grep rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("ls{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default ls rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex("which .*").expect("Can parse default which rule into regex"),
            ];

            pub static ref DEFAULT_COMMAND_EXECUTION_DENYLIST: Vec<AgentModeCommandExecutionPredicate> = vec![
                AgentModeCommandExecutionPredicate::new_regex(&format!("bash{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default bash rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("fish{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default fish rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("pwsh{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default pwsh rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("sh{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default sh rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("zsh{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default zsh rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("curl{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default curl rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("eval{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default eval rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("exec{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default exec rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("source{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default source rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("wget{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default wget rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("dig{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default dig rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("nslookup{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default nslookup rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("host{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default host rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("ssh{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default ssh rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("scp{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default scp rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("rsync{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default rsync rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("telnet{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default telnet rule into regex"),
                AgentModeCommandExecutionPredicate::new_regex(&format!("rm{}", OPTIONAL_ARGS_REGEX.as_str())).expect("Can parse default rm rule into regex"),
            ];
        }
    }
}
