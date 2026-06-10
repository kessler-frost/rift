use riftui::platform::WindowStyle;
use riftui::App;

use super::*;
use crate::settings_view::keybindings::KeybindingChangedNotifier;
use crate::test_util::settings::initialize_settings_for_tests;

fn initialize_app(app: &mut App) {
    initialize_settings_for_tests(app);

    app.add_singleton_model(|_| Appearance::mock());
    app.add_singleton_model(|_| ResizableData::default());
    app.add_singleton_model(|_| KeybindingChangedNotifier::mock());
}

#[test]
fn test_render_view() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);

        let (_window_id, _view) =
            app.add_window(WindowStyle::NotStealFocus, CommandSearchView::new);

        app.update(|_| {
            // This will force a redraw of the window, which lays out the
            // window, including the command search view.
        });
    });
}
