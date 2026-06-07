use fuzzy_match::{match_indices_case_insensitive, FuzzyMatchResult};
use itertools::Itertools;
use riftui::{AppContext, Entity};

use super::RepoSearchItem;
use crate::search::command_palette::mixer::CommandPaletteItemAction;
use crate::search::data_source::{Query, QueryResult};
use crate::search::mixer::{DataSourceRunErrorWrapper, SyncDataSource};

const MAX_REPOS_CONSIDERED: usize = 50;

pub struct RepoDataSource {}

impl Default for RepoDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl RepoDataSource {
    pub fn new() -> Self {
        Self {}
    }

    pub fn top_n(&self, _limit: usize, _app: &AppContext) -> impl Iterator<Item = RepoSearchItem> {
        // The recent-workspaces list was backed by a cloud-synced model that has been
        // removed; there is no local source of recent repos yet.
        std::iter::empty()
    }
}

impl Entity for RepoDataSource {
    type Event = ();
}

impl SyncDataSource for RepoDataSource {
    type Action = CommandPaletteItemAction;

    fn run_query(
        &self,
        query: &Query,
        app: &AppContext,
    ) -> Result<Vec<QueryResult<Self::Action>>, DataSourceRunErrorWrapper> {
        let query_str = query.text.as_str();

        let repos = self.top_n(MAX_REPOS_CONSIDERED, app);

        let results = repos
            .filter_map(|mut repo| {
                let match_result = if query_str.is_empty() {
                    Some(FuzzyMatchResult::no_match())
                } else {
                    match_indices_case_insensitive(repo.display_name.as_str(), query_str)
                };

                // Boost repo results so they compete fairly with other sources
                match_result.map(|mut match_result| {
                    match_result.score *= 4;
                    repo.match_result = match_result;
                    repo.into()
                })
            })
            .collect_vec();

        Ok(results)
    }
}
