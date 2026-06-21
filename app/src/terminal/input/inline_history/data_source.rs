//! Data source for the inline history menu, providing both conversations and commands.
//!
//! Ordering semantics match the legacy up-arrow history menu:
//! - Items from different sessions appear before items from the current session
//! - Within each group, items are sorted by timestamp (oldest first)
//! - Commands are deduplicated, keeping the most recent occurrence
//! - The result is that current session items appear at the bottom (closer to input)

use chrono::{DateTime, Local};
use ordered_float::OrderedFloat;
use riftui::{AppContext, Entity, EntityId, ModelHandle, SingletonEntity};

use crate::input_suggestions::HistoryInputSuggestion;
use crate::search::data_source::{Query, QueryFilter, QueryResult};
use crate::search::mixer::DataSourceRunErrorWrapper;
use crate::search::SyncDataSource;
use crate::terminal::history::{History, UpArrowHistoryConfig};
use crate::terminal::input::inline_history::search_item::InlineHistoryItem;
use crate::terminal::input::inline_menu::{
    InlineMenuAction, InlineMenuClickBehavior, InlineMenuType,
};
use crate::terminal::model::session::active_session::ActiveSession;

#[derive(Clone, Debug)]
pub enum AcceptHistoryItem {
    Command { command: String },
    AIPrompt { query_text: String },
}

impl AcceptHistoryItem {
    pub fn buffer_replacement_text(&self) -> Option<&String> {
        match self {
            AcceptHistoryItem::Command { command, .. } => Some(command),
            AcceptHistoryItem::AIPrompt { query_text } => Some(query_text),
        }
    }
}

impl InlineMenuAction for AcceptHistoryItem {
    const MENU_TYPE: InlineMenuType = InlineMenuType::InlineHistoryMenu;

    fn click_behavior(&self) -> InlineMenuClickBehavior {
        match self {
            AcceptHistoryItem::Command { .. } | AcceptHistoryItem::AIPrompt { .. } => {
                InlineMenuClickBehavior::SelectOnClick
            }
        }
    }
}

/// Data source that provides command history for a terminal view.
pub struct InlineHistoryMenuDataSource {
    terminal_view_id: EntityId,
    active_session: ModelHandle<ActiveSession>,
}

impl InlineHistoryMenuDataSource {
    pub fn new(terminal_view_id: EntityId, active_session: ModelHandle<ActiveSession>) -> Self {
        Self {
            terminal_view_id,
            active_session,
        }
    }
}

#[derive(Clone)]
struct MenuEntry {
    item: MenuItem,
}

#[derive(Clone)]
enum MenuItem {
    Command {
        command: String,
        display_timestamp: DateTime<Local>,
        prefix_match_len: usize,
    },
}

impl SyncDataSource for InlineHistoryMenuDataSource {
    type Action = AcceptHistoryItem;

    fn run_query(
        &self,
        query: &Query,
        app: &AppContext,
    ) -> Result<Vec<QueryResult<Self::Action>>, DataSourceRunErrorWrapper> {
        let trimmed_query = query.text.trim();
        let prefix_match_len = trimmed_query.len();

        let session_id = self.active_session.as_ref(app).session(app).map(|s| s.id());

        let include_commands =
            query.filters.is_empty() || query.filters.contains(&QueryFilter::Commands);

        let history = History::handle(app).as_ref(app);

        let command_entries = if include_commands {
            history
                .up_arrow_suggestions_for_terminal_view(
                    self.terminal_view_id,
                    session_id,
                    UpArrowHistoryConfig {
                        include_commands: true,
                    },
                    app,
                )
                .into_iter()
                .filter_map(|suggestion| {
                    let HistoryInputSuggestion::Command { entry } = &suggestion;

                    let command = entry.command.trim();
                    if command.is_empty() {
                        return None;
                    }
                    if !trimmed_query.is_empty() && !command.starts_with(trimmed_query) {
                        return None;
                    }

                    let display_timestamp = entry.start_ts.unwrap_or_else(Local::now);

                    Some(MenuEntry {
                        item: MenuItem::Command {
                            command: command.to_string(),
                            display_timestamp,
                            prefix_match_len,
                        },
                    })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let mut results: Vec<QueryResult<AcceptHistoryItem>> = Vec::new();
        for entry in command_entries {
            let score = OrderedFloat(results.len() as f64);
            let search_item = match entry.item {
                MenuItem::Command {
                    command,
                    display_timestamp,
                    prefix_match_len,
                } => InlineHistoryItem::command(command, display_timestamp)
                    .with_prefix_match_len(prefix_match_len),
            };

            results.push(QueryResult::from(search_item.with_score(score)));
        }

        Ok(results)
    }
}

impl Entity for InlineHistoryMenuDataSource {
    type Event = ();
}

#[cfg(test)]
#[path = "data_source_tests.rs"]
mod tests;
