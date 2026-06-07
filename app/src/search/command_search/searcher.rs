use crate::search::mixer::SearchMixer;
use crate::terminal::history::LinkedWorkflowData;

pub type CommandSearchMixer = SearchMixer<CommandSearchItemAction>;

#[derive(Clone, Debug)]
pub struct AcceptedHistoryItem {
    pub command: String,

    /// The workflow used to construct the command, if any.
    pub linked_workflow_data: Option<LinkedWorkflowData>,
}

/// The set of events that may be produced by accepting or executing a search
/// result.
#[derive(Clone, Debug)]
pub enum CommandSearchItemAction {
    /// The user accepted a history search item. The contained string is the
    /// command they accepted.
    AcceptHistory(AcceptedHistoryItem),

    /// The user requested the re-execution of a history search item. The
    /// contained string is the command they accepted.
    ExecuteHistory(String),
}

#[cfg(test)]
#[path = "searcher_tests.rs"]
mod tests;
