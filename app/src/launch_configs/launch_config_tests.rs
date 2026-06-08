use std::path::PathBuf;

use super::{LaunchConfig, PaneMode, PaneTemplateType};
use crate::app_state::{
    AppState, BranchSnapshot, LeafContents, LeafSnapshot, PaneFlex, PaneNodeSnapshot,
    SplitDirection, TabSnapshot, TerminalPaneSnapshot, WindowSnapshot,
};
use crate::tab::SelectedTabColor;

fn single_tab_snapshot(root: PaneNodeSnapshot) -> AppState {
    AppState {
        windows: vec![WindowSnapshot {
            tabs: vec![TabSnapshot {
                custom_title: None,
                default_directory_color: None,
                selected_color: SelectedTabColor::default(),
                root,
                left_panel: None,
                right_panel: None,
            }],
            active_tab_index: 0,
            bounds: None,
            quake_mode: false,
            universal_search_width: None,
            warp_ai_width: None,
            voltron_width: None,
            warp_drive_index_width: None,
            left_panel_open: false,
            vertical_tabs_panel_open: false,
            fullscreen_state: Default::default(),
            left_panel_width: None,
            right_panel_width: None,
        }],
        active_window_index: Some(0),
    }
}

fn multi_tab_snapshot(active_tab_index: usize, tabs: Vec<TabSnapshot>) -> AppState {
    AppState {
        windows: vec![WindowSnapshot {
            tabs,
            active_tab_index,
            bounds: None,
            quake_mode: false,
            universal_search_width: None,
            warp_ai_width: None,
            voltron_width: None,
            warp_drive_index_width: None,
            left_panel_open: false,
            vertical_tabs_panel_open: false,
            fullscreen_state: Default::default(),
            left_panel_width: None,
            right_panel_width: None,
        }],
        active_window_index: Some(0),
    }
}

fn terminal_pane(cwd: &str) -> TerminalPaneSnapshot {
    TerminalPaneSnapshot {
        uuid: vec![],
        cwd: Some(cwd.into()),
        is_active: true,
        is_read_only: false,
        shell_launch_data: None,
        active_profile_id: None,
    }
}

#[test]
fn test_config_from_snapshot_flattens_single_pane() {
    // If only one pane of the branch can be saved into a launch configuration, it should
    // be flattened to a single leaf. Non-terminal panes are filtered out.

    let state = single_tab_snapshot(PaneNodeSnapshot::Branch(BranchSnapshot {
        direction: SplitDirection::Vertical,
        children: vec![
            (
                PaneFlex(1.),
                PaneNodeSnapshot::Leaf(LeafSnapshot {
                    is_focused: true,
                    custom_vertical_tabs_title: None,
                    contents: LeafContents::NetworkLog,
                }),
            ),
            (
                PaneFlex(1.),
                PaneNodeSnapshot::Leaf(LeafSnapshot {
                    is_focused: true,
                    custom_vertical_tabs_title: None,
                    contents: LeafContents::Terminal(terminal_pane("/some/dir")),
                }),
            ),
        ],
    }));

    let template = LaunchConfig::from_snapshot("Test".into(), &state);
    assert_eq!(
        template.windows[0].tabs[0].layout,
        PaneTemplateType::PaneTemplate {
            is_focused: Some(true),
            cwd: PathBuf::from("/some/dir"),
            commands: vec![],
            pane_mode: PaneMode::Terminal,
            shell: None,
        },
    )
}

#[test]
fn test_config_from_snapshot_filters_panes() {
    let state = single_tab_snapshot(PaneNodeSnapshot::Branch(BranchSnapshot {
        direction: SplitDirection::Vertical,
        children: vec![
            (
                PaneFlex(1.),
                PaneNodeSnapshot::Leaf(LeafSnapshot {
                    is_focused: true,
                    custom_vertical_tabs_title: None,
                    contents: LeafContents::Terminal(terminal_pane("/path/to/dir")),
                }),
            ),
            (
                PaneFlex(1.),
                PaneNodeSnapshot::Leaf(LeafSnapshot {
                    is_focused: false,
                    custom_vertical_tabs_title: None,
                    contents: LeafContents::NetworkLog,
                }),
            ),
            (
                PaneFlex(1.),
                PaneNodeSnapshot::Leaf(LeafSnapshot {
                    is_focused: false,
                    custom_vertical_tabs_title: None,
                    contents: LeafContents::Terminal(terminal_pane("/some/dir")),
                }),
            ),
        ],
    }));

    let template = LaunchConfig::from_snapshot("Test".into(), &state);
    assert_eq!(
        template.windows[0].tabs[0].layout,
        PaneTemplateType::PaneBranchTemplate {
            split_direction: SplitDirection::Vertical.into(),
            panes: vec![
                PaneTemplateType::PaneTemplate {
                    is_focused: Some(true),
                    cwd: PathBuf::from("/path/to/dir"),
                    commands: vec![],
                    pane_mode: PaneMode::Terminal,
                    shell: None,
                },
                PaneTemplateType::PaneTemplate {
                    is_focused: Some(false),
                    cwd: PathBuf::from("/some/dir"),
                    commands: vec![],
                    pane_mode: PaneMode::Terminal,
                    shell: None,
                },
            ]
        }
    )
}

#[test]
fn test_config_from_snapshot_filters_tabs() {
    // If no panes of a tab are valid, it's filtered out entirely.

    let state = single_tab_snapshot(PaneNodeSnapshot::Branch(BranchSnapshot {
        direction: SplitDirection::Vertical,
        children: vec![(
            PaneFlex(1.),
            PaneNodeSnapshot::Leaf(LeafSnapshot {
                is_focused: true,
                custom_vertical_tabs_title: None,
                contents: LeafContents::NetworkLog,
            }),
        )],
    }));

    let template = LaunchConfig::from_snapshot("Test".into(), &state);
    assert!(template.windows[0].tabs.is_empty())
}

#[test]
fn test_config_with_active_tab_index() {
    let state = multi_tab_snapshot(
        1,
        vec![
            TabSnapshot {
                custom_title: None,
                default_directory_color: None,
                selected_color: SelectedTabColor::default(),
                root: PaneNodeSnapshot::Branch(BranchSnapshot {
                    direction: SplitDirection::Vertical,
                    children: vec![(
                        PaneFlex(1.),
                        PaneNodeSnapshot::Leaf(LeafSnapshot {
                            is_focused: true,
                            custom_vertical_tabs_title: None,
                            contents: LeafContents::Terminal(terminal_pane("/path/to/dir")),
                        }),
                    )],
                }),
                left_panel: None,
                right_panel: None
            };
            3
        ],
    );

    let template = LaunchConfig::from_snapshot("Test".into(), &state);
    assert_eq!(template.windows[0].active_tab_index, Some(1))
}

#[test]
fn test_config_with_active_tab_index_and_filtered_tabs() {
    let state = multi_tab_snapshot(
        1,
        vec![
            TabSnapshot {
                custom_title: None,
                default_directory_color: None,
                selected_color: SelectedTabColor::default(),
                root: PaneNodeSnapshot::Branch(BranchSnapshot {
                    direction: SplitDirection::Vertical,
                    children: vec![(
                        PaneFlex(1.),
                        PaneNodeSnapshot::Leaf(LeafSnapshot {
                            is_focused: true,
                            custom_vertical_tabs_title: None,
                            contents: LeafContents::NetworkLog,
                        }),
                    )],
                }),
                left_panel: None,
                right_panel: None,
            },
            TabSnapshot {
                custom_title: None,
                default_directory_color: None,
                selected_color: SelectedTabColor::default(),
                root: PaneNodeSnapshot::Branch(BranchSnapshot {
                    direction: SplitDirection::Vertical,
                    children: vec![(
                        PaneFlex(1.),
                        PaneNodeSnapshot::Leaf(LeafSnapshot {
                            is_focused: true,
                            custom_vertical_tabs_title: None,
                            contents: LeafContents::Terminal(terminal_pane("/path/to/dir")),
                        }),
                    )],
                }),
                left_panel: None,
                right_panel: None,
            },
        ],
    );

    let template = LaunchConfig::from_snapshot("Test".into(), &state);
    assert_eq!(template.windows[0].active_tab_index, Some(0))
}

#[test]
fn test_config_with_active_tab_being_filtered() {
    let state = multi_tab_snapshot(
        1,
        vec![
            TabSnapshot {
                custom_title: None,
                default_directory_color: None,
                selected_color: SelectedTabColor::default(),
                root: PaneNodeSnapshot::Branch(BranchSnapshot {
                    direction: SplitDirection::Vertical,
                    children: vec![(
                        PaneFlex(1.),
                        PaneNodeSnapshot::Leaf(LeafSnapshot {
                            is_focused: true,
                            custom_vertical_tabs_title: None,
                            contents: LeafContents::Terminal(terminal_pane("/path/to/dir")),
                        }),
                    )],
                }),
                left_panel: None,
                right_panel: None,
            },
            TabSnapshot {
                custom_title: None,
                default_directory_color: None,
                selected_color: SelectedTabColor::default(),
                root: PaneNodeSnapshot::Branch(BranchSnapshot {
                    direction: SplitDirection::Vertical,
                    children: vec![(
                        PaneFlex(1.),
                        PaneNodeSnapshot::Leaf(LeafSnapshot {
                            is_focused: true,
                            custom_vertical_tabs_title: None,
                            contents: LeafContents::NetworkLog,
                        }),
                    )],
                }),
                left_panel: None,
                right_panel: None,
            },
        ],
    );

    let template = LaunchConfig::from_snapshot("Test".into(), &state);
    assert_eq!(template.windows[0].active_tab_index, None)
}
