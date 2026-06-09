use riftui::integration::AssertionCallback;
use riftui::async_assert;

use crate::integration_testing::view_getters::workspace_view;

pub fn assert_command_search_is_open() -> AssertionCallback {
    Box::new(move |app, window_id| {
        let workspace_view = workspace_view(app, window_id);
        workspace_view.read(app, |workspace, _ctx| {
            async_assert!(workspace.is_command_search_open())
        })
    })
}
