use std::path::PathBuf;

use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use riftui::elements::PositionedElementOffsetBounds;
use riftui::EntityId;

use super::{
    branch_label_display, coalesce_summary_branch_entries, compact_branch_subtitle_display,
    detail_sidecar_width_and_bounds, detail_target_for_hovered_row,
    non_terminal_search_text_fragments, pane_ids_for_display_granularity,
    pane_search_text_fragments, push_normalized_unique_summary_label,
    search_fragments_contain_query, select_summary_pane_kind_icons,
    should_keep_detail_sidecar_visible_for_mouse_position, summary_overflow_count,
    summary_search_text_fragments, terminal_primary_line_data, uses_outer_group_container,
    visible_pane_ids_for_detail_target, vtab_diff_stats_text, SummaryPaneKind,
    SummaryPaneKindIcons, TerminalPrimaryLineData, TerminalPrimaryLineFont,
    VerticalTabsDetailTarget, VerticalTabsDetailTargetKind, VerticalTabsSummaryBranchEntry,
    VerticalTabsSummaryData, VerticalTabsSummaryPrimaryLabel,
};
use crate::context_chips::display_chip::GitLineChanges;
use crate::pane_group::{PaneId, TerminalPaneId};
use crate::safe_triangle::SafeTriangle;
use crate::workspace::tab_settings::VerticalTabsDisplayGranularity;

fn label(text: &str) -> VerticalTabsSummaryPrimaryLabel {
    VerticalTabsSummaryPrimaryLabel {
        text: text.to_string(),
    }
}

fn pane_id() -> PaneId {
    TerminalPaneId::dummy_terminal_pane_id().into()
}

#[test]
fn summary_pane_kind_icons_render_single_icon_for_homogeneous_tabs() {
    assert_eq!(
        select_summary_pane_kind_icons([
            (EntityId::from_usize(10), SummaryPaneKind::Terminal),
            (EntityId::from_usize(20), SummaryPaneKind::Terminal),
        ]),
        Some(SummaryPaneKindIcons::Single(SummaryPaneKind::Terminal))
    );
}

fn collect_normalized_unique_summary_texts(
    texts: impl IntoIterator<Item = impl AsRef<str>>,
) -> Vec<String> {
    texts
        .into_iter()
        .filter_map(|text| {
            let normalized = text
                .as_ref()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            (!normalized.is_empty()).then_some(normalized)
        })
        .fold(Vec::new(), |mut values, normalized| {
            if !values.contains(&normalized) {
                values.push(normalized);
            }
            values
        })
}

#[test]
fn detail_target_matches_panes_granularity() {
    let pane_group_id = EntityId::new();
    let hovered_pane_id = pane_id();

    assert_eq!(
        detail_target_for_hovered_row(
            pane_group_id,
            hovered_pane_id,
            VerticalTabsDisplayGranularity::Panes,
        ),
        VerticalTabsDetailTarget::Pane {
            pane_group_id,
            pane_id: hovered_pane_id,
        }
    );
}

#[test]
fn detail_target_matches_tabs_granularity() {
    let pane_group_id = EntityId::new();
    let hovered_pane_id = pane_id();

    assert_eq!(
        detail_target_for_hovered_row(
            pane_group_id,
            hovered_pane_id,
            VerticalTabsDisplayGranularity::Tabs,
        ),
        VerticalTabsDetailTarget::Tab {
            pane_group_id,
            source_pane_id: hovered_pane_id,
        }
    );
}

#[test]
fn pane_detail_target_returns_hovered_pane_when_supported() {
    let hovered_pane_id = pane_id();

    assert_eq!(
        visible_pane_ids_for_detail_target(
            &[hovered_pane_id],
            hovered_pane_id,
            VerticalTabsDetailTargetKind::Pane,
            |pane_id| pane_id == hovered_pane_id,
        ),
        Some(vec![hovered_pane_id])
    );
}

#[test]
fn pane_detail_target_returns_none_when_hovered_pane_is_not_supported() {
    let hovered_pane_id = pane_id();

    assert_eq!(
        visible_pane_ids_for_detail_target(
            &[hovered_pane_id],
            hovered_pane_id,
            VerticalTabsDetailTargetKind::Pane,
            |_| false,
        ),
        None
    );
}

#[test]
fn tab_detail_target_returns_all_visible_panes_when_every_pane_is_supported() {
    let pane_1 = pane_id();
    let pane_2 = pane_id();
    let pane_3 = pane_id();
    let visible_pane_ids = vec![pane_1, pane_2, pane_3];

    assert_eq!(
        visible_pane_ids_for_detail_target(
            &visible_pane_ids,
            pane_2,
            VerticalTabsDetailTargetKind::Tab,
            |_| true,
        ),
        Some(visible_pane_ids)
    );
}

#[test]
fn tab_detail_target_returns_none_for_mixed_support_tabs() {
    let pane_1 = pane_id();
    let pane_2 = pane_id();
    let pane_3 = pane_id();

    assert_eq!(
        visible_pane_ids_for_detail_target(
            &[pane_1, pane_2, pane_3],
            pane_2,
            VerticalTabsDetailTargetKind::Tab,
            |pane_id| pane_id != pane_3,
        ),
        None
    );
}

#[test]
fn panes_granularity_returns_all_visible_panes_in_order() {
    let pane_1 = pane_id();
    let pane_2 = pane_id();
    let pane_3 = pane_id();
    let visible_pane_ids = vec![pane_1, pane_2, pane_3];

    assert_eq!(
        pane_ids_for_display_granularity(
            &visible_pane_ids,
            pane_2,
            VerticalTabsDisplayGranularity::Panes,
        ),
        visible_pane_ids
    );
}

#[test]
fn tabs_granularity_returns_focused_pane_when_present() {
    let pane_1 = pane_id();
    let pane_2 = pane_id();
    let pane_3 = pane_id();

    assert_eq!(
        pane_ids_for_display_granularity(
            &[pane_1, pane_2, pane_3],
            pane_2,
            VerticalTabsDisplayGranularity::Tabs,
        ),
        vec![pane_2]
    );
}

#[test]
fn tabs_granularity_falls_back_to_first_visible_pane_when_focused_pane_is_absent() {
    let pane_1 = pane_id();
    let pane_2 = pane_id();
    let pane_3 = pane_id();
    let focused_pane = pane_id();

    assert_eq!(
        pane_ids_for_display_granularity(
            &[pane_1, pane_2, pane_3],
            focused_pane,
            VerticalTabsDisplayGranularity::Tabs,
        ),
        vec![pane_1]
    );
}

#[test]
fn tabs_granularity_returns_empty_for_empty_visible_panes() {
    assert_eq!(
        pane_ids_for_display_granularity(&[], pane_id(), VerticalTabsDisplayGranularity::Tabs,),
        Vec::<PaneId>::new()
    );
}

#[test]
fn detail_sidecar_uses_default_width_when_space_allows() {
    let (width, bounds) = detail_sidecar_width_and_bounds(400.);
    assert_eq!(width, 320.);
    assert!(matches!(
        bounds,
        PositionedElementOffsetBounds::WindowBySize
    ));
}

#[test]
fn detail_sidecar_shrinks_to_fit_before_hitting_min_width() {
    let (width, bounds) = detail_sidecar_width_and_bounds(280.);
    assert_eq!(width, 280.);
    assert!(matches!(
        bounds,
        PositionedElementOffsetBounds::WindowBySize
    ));
}

#[test]
fn detail_sidecar_stops_shrinking_at_min_width_and_allows_clipping() {
    let (width, bounds) = detail_sidecar_width_and_bounds(180.);
    assert_eq!(width, 240.);
    assert!(matches!(bounds, PositionedElementOffsetBounds::Unbounded));
}

#[test]
fn detail_sidecar_visibility_helper_keeps_sidecar_visible_inside_sidecar_bounds() {
    let row_rect = RectF::new(Vector2F::new(0., 100.), Vector2F::new(100., 40.));
    let sidecar_rect = RectF::new(Vector2F::new(120., 50.), Vector2F::new(180., 220.));
    let mut safe_triangle = SafeTriangle::new();

    assert!(should_keep_detail_sidecar_visible_for_mouse_position(
        Vector2F::new(200., 120.),
        Some(row_rect),
        Some(sidecar_rect),
        &mut safe_triangle,
    ));
}

#[test]
fn detail_sidecar_visibility_helper_keeps_sidecar_visible_in_safe_triangle() {
    let row_rect = RectF::new(Vector2F::new(0., 100.), Vector2F::new(100., 40.));
    let sidecar_rect = RectF::new(Vector2F::new(120., 50.), Vector2F::new(180., 220.));
    let mut safe_triangle = SafeTriangle::new();
    safe_triangle.set_target_rect(Some(sidecar_rect));
    safe_triangle.update_position(Vector2F::new(90., 120.));

    assert!(should_keep_detail_sidecar_visible_for_mouse_position(
        Vector2F::new(110., 120.),
        Some(row_rect),
        Some(sidecar_rect),
        &mut safe_triangle,
    ));
}

#[test]
fn detail_sidecar_visibility_helper_clears_sidecar_outside_row_sidecar_and_safe_triangle() {
    let row_rect = RectF::new(Vector2F::new(0., 100.), Vector2F::new(100., 40.));
    let sidecar_rect = RectF::new(Vector2F::new(120., 50.), Vector2F::new(180., 220.));
    let mut safe_triangle = SafeTriangle::new();
    safe_triangle.update_position(Vector2F::new(200., 120.));

    assert!(!should_keep_detail_sidecar_visible_for_mouse_position(
        Vector2F::new(340., 120.),
        Some(row_rect),
        Some(sidecar_rect),
        &mut safe_triangle,
    ));
}

#[test]
fn panes_granularity_uses_outer_group_container() {
    assert!(uses_outer_group_container(
        VerticalTabsDisplayGranularity::Panes
    ));
}

#[test]
fn tabs_granularity_does_not_use_outer_group_container() {
    assert!(!uses_outer_group_container(
        VerticalTabsDisplayGranularity::Tabs
    ));
}

#[test]
fn terminal_primary_line_uses_terminal_title_when_distinct_from_working_directory() {
    let line = terminal_primary_line_data(
        "nvim src/workspace/view/vertical_tabs.rs",
        "~/rift",
        Some("cargo nextest run".to_string()),
    );

    assert_eq!(line.text(), "nvim src/workspace/view/vertical_tabs.rs");
}

#[test]
fn terminal_primary_line_uses_last_completed_command_when_shell_title_matches_working_directory() {
    let line =
        terminal_primary_line_data("~/rift", "~/rift", Some("cargo nextest run".to_string()));

    assert_eq!(line.text(), "cargo nextest run");
}

#[test]
fn terminal_primary_line_falls_back_to_new_session() {
    let line = terminal_primary_line_data("~/rift", "~/rift", None);

    assert_eq!(line.text(), "New session");
    assert!(matches!(
        line,
        TerminalPrimaryLineData::Text {
            font: TerminalPrimaryLineFont::Ui,
            ..
        }
    ));
}

#[test]
fn terminal_primary_line_uses_monospace_for_last_completed_command() {
    let line =
        terminal_primary_line_data("~/rift", "~/rift", Some("cargo nextest run".to_string()));

    assert!(matches!(
        line,
        TerminalPrimaryLineData::Text {
            font: TerminalPrimaryLineFont::Monospace,
            ..
        }
    ));
}

#[test]
fn pane_search_fragments_prepend_custom_title_and_keep_generated_metadata() {
    let fragments = pane_search_text_fragments(
        Some("Production API"),
        vec![
            "cargo nextest run".to_string(),
            "~/rift".to_string(),
            "Claude".to_string(),
        ],
    );

    assert_eq!(fragments[0], "Production API");
    assert!(search_fragments_contain_query(&fragments, "production api"));
    assert!(search_fragments_contain_query(&fragments, "cargo nextest"));
    assert!(search_fragments_contain_query(&fragments, "~/rift"));
    assert!(search_fragments_contain_query(&fragments, "claude"));
}

#[test]
fn pane_search_fragments_dedupe_custom_title_against_generated_text() {
    assert_eq!(
        pane_search_text_fragments(
            Some("  Production   API  "),
            vec![
                "Production API".to_string(),
                "~/rift".to_string(),
                "~/rift".to_string(),
            ],
        ),
        vec!["Production API".to_string(), "~/rift".to_string()]
    );
}

#[test]
fn non_terminal_search_fragments_only_include_rendered_text() {
    let fragments = non_terminal_search_text_fragments("Pane title", "and 2 more");

    assert!(search_fragments_contain_query(&fragments, "pane title"));
    assert!(search_fragments_contain_query(&fragments, "and 2 more"));
    assert!(!search_fragments_contain_query(&fragments, "notebook"));
    assert!(!search_fragments_contain_query(&fragments, "unsaved"));
}

#[test]
fn diff_stats_text_matches_rendered_badge_text() {
    assert_eq!(
        vtab_diff_stats_text(&GitLineChanges {
            files_changed: 1,
            lines_added: 2,
            lines_removed: 3,
        }),
        "+2 -3"
    );
    assert_eq!(
        vtab_diff_stats_text(&GitLineChanges {
            files_changed: 1,
            lines_added: 0,
            lines_removed: 0,
        }),
        "0"
    );
}

#[test]
fn branch_label_display_falls_back_without_branch_icon() {
    assert_eq!(
        branch_label_display(None, "~/rift"),
        ("~/rift".to_string(), false)
    );
    assert_eq!(
        branch_label_display(Some(""), "~/rift"),
        ("~/rift".to_string(), false)
    );
    assert_eq!(
        branch_label_display(Some("main"), "~/rift"),
        ("main".to_string(), true)
    );
}

#[test]
fn compact_branch_subtitle_falls_back_to_working_directory_without_branch_icon() {
    assert_eq!(
        compact_branch_subtitle_display(None, Some("~/rift")),
        Some(("~/rift".to_string(), false))
    );
    assert_eq!(
        compact_branch_subtitle_display(Some(""), Some("~/rift")),
        Some(("~/rift".to_string(), false))
    );
    assert_eq!(
        compact_branch_subtitle_display(Some("main"), Some("~/rift")),
        Some(("main".to_string(), true))
    );
}

#[test]
fn collect_normalized_unique_summary_texts_dedupes_after_whitespace_normalization() {
    assert_eq!(
        collect_normalized_unique_summary_texts([
            "  cargo   test  ",
            "cargo test",
            "",
            " git   status ",
        ]),
        vec!["cargo test".to_string(), "git status".to_string()]
    );
}

#[test]
fn collect_normalized_unique_summary_texts_preserves_first_seen_order() {
    assert_eq!(
        collect_normalized_unique_summary_texts([
            "~/rift-internal",
            "~/rift-server",
            "~/rift-internal",
            "~/rift-terraform",
        ]),
        vec![
            "~/rift-internal".to_string(),
            "~/rift-server".to_string(),
            "~/rift-terraform".to_string(),
        ]
    );
}

#[test]
fn coalesce_summary_branch_entries_groups_by_repo_and_branch() {
    let repo_a = PathBuf::from("/tmp/repo-a");
    let repo_b = PathBuf::from("/tmp/repo-b");
    let entries = vec![
        VerticalTabsSummaryBranchEntry {
            repo_path: repo_a.clone(),
            branch_name: "main".to_string(),
            diff_stats: None,
            pull_request_label: None,
        },
        VerticalTabsSummaryBranchEntry {
            repo_path: repo_a.clone(),
            branch_name: "main".to_string(),
            diff_stats: Some(GitLineChanges {
                files_changed: 1,
                lines_added: 2,
                lines_removed: 3,
            }),
            pull_request_label: Some("#123".to_string()),
        },
        VerticalTabsSummaryBranchEntry {
            repo_path: repo_b.clone(),
            branch_name: "main".to_string(),
            diff_stats: Some(GitLineChanges {
                files_changed: 4,
                lines_added: 5,
                lines_removed: 6,
            }),
            pull_request_label: Some("#456".to_string()),
        },
    ];

    assert_eq!(
        coalesce_summary_branch_entries(entries),
        vec![
            VerticalTabsSummaryBranchEntry {
                repo_path: repo_a,
                branch_name: "main".to_string(),
                diff_stats: Some(GitLineChanges {
                    files_changed: 1,
                    lines_added: 2,
                    lines_removed: 3,
                }),
                pull_request_label: Some("#123".to_string()),
            },
            VerticalTabsSummaryBranchEntry {
                repo_path: repo_b,
                branch_name: "main".to_string(),
                diff_stats: Some(GitLineChanges {
                    files_changed: 4,
                    lines_added: 5,
                    lines_removed: 6,
                }),
                pull_request_label: Some("#456".to_string()),
            },
        ]
    );
}

#[test]
fn summary_overflow_count_caps_visible_region() {
    assert_eq!(summary_overflow_count(5, 3), 2);
    assert_eq!(summary_overflow_count(3, 3), 0);
    assert_eq!(summary_overflow_count(2, 3), 0);
}

#[test]
fn primary_labels_dedupe_preserves_first_seen_text() {
    let mut values = Vec::new();
    let mut seen = std::collections::HashMap::new();
    push_normalized_unique_summary_label(&mut values, &mut seen, "  cargo   test  ");
    push_normalized_unique_summary_label(&mut values, &mut seen, "cargo test");

    assert_eq!(
        values,
        vec![VerticalTabsSummaryPrimaryLabel {
            text: "cargo test".to_string(),
        }]
    );
}

#[test]
fn summary_search_fragments_include_hidden_overflow_values() {
    let summary = VerticalTabsSummaryData {
        primary_labels: vec![
            label("Claude"),
            label("Oz"),
            label("cargo"),
            label("code review"),
            label("hidden work"),
        ],
        working_directories: vec!["~/rift-internal".to_string(), "~/rift-server".to_string()],
        branch_entries: vec![
            VerticalTabsSummaryBranchEntry {
                repo_path: PathBuf::from("/tmp/repo-a"),
                branch_name: "main".to_string(),
                diff_stats: Some(GitLineChanges {
                    files_changed: 1,
                    lines_added: 2,
                    lines_removed: 3,
                }),
                pull_request_label: Some("#123".to_string()),
            },
            VerticalTabsSummaryBranchEntry {
                repo_path: PathBuf::from("/tmp/repo-b"),
                branch_name: "feature/hidden".to_string(),
                diff_stats: None,
                pull_request_label: None,
            },
            VerticalTabsSummaryBranchEntry {
                repo_path: PathBuf::from("/tmp/repo-c"),
                branch_name: "cleanup".to_string(),
                diff_stats: None,
                pull_request_label: None,
            },
            VerticalTabsSummaryBranchEntry {
                repo_path: PathBuf::from("/tmp/repo-d"),
                branch_name: "hidden-branch".to_string(),
                diff_stats: None,
                pull_request_label: Some("#789".to_string()),
            },
        ],
        has_unread_activity: false,
    };

    let fragments = summary_search_text_fragments(&summary, Some("Custom tab"));

    assert!(search_fragments_contain_query(&fragments, "custom tab"));
    assert!(search_fragments_contain_query(&fragments, "claude"));
    assert!(search_fragments_contain_query(&fragments, "hidden work"));
    assert!(search_fragments_contain_query(&fragments, "hidden-branch"));
    assert!(search_fragments_contain_query(&fragments, "#789"));
    assert!(search_fragments_contain_query(&fragments, "+2"));
    assert!(search_fragments_contain_query(&fragments, "-3"));
}
