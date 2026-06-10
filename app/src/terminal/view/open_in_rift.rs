use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use itertools::Itertools;
use lazy_static::lazy_static;
use rift_completer::completer::TopLevelCommandCaseSensitivity;
use rift_completer::parsers::classify_command;
use rift_completer::parsers::hir::{Command, Expression};
use rift_completer::parsers::simple::all_parsed_commands;
use rift_completer::signatures::CommandRegistry;
use rift_util::path::EscapeChar;
use riftui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use riftui::{SingletonEntity, ViewContext};
use settings::Setting as _;

use super::{Event, InlineBannerItem, InlineBannerType, TerminalView};
use crate::report_if_error;
use crate::terminal::event::UserBlockCompleted;
use crate::terminal::general_settings::GeneralSettings;
use crate::terminal::model::session::Session;
use crate::terminal::view::inline_banner::{OpenInRiftBannerAction, OpenInRiftBannerState};
use crate::util::openable_file_type::{is_file_openable_in_warp, OpenableFileType};

#[cfg(test)]
#[path = "open_in_rift_tests.rs"]
mod tests;

const LEARN_MORE_MARKDOWN_URL: &str =
    "https://docs.warp.dev/terminal/more-features/markdown-viewer";
const LEARN_MORE_CODE_URL: &str = "https://docs.warp.dev/code/overview#built-in-code-editor";

/// A path to a file that can be opened in Rift, along with its type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenablePath {
    pub path: PathBuf,
    pub file_type: OpenableFileType,
}

impl TerminalView {
    pub(super) fn maybe_suggest_open_in_rift(
        &mut self,
        block_completed: &UserBlockCompleted,
        ctx: &mut ViewContext<TerminalView>,
    ) {
        if let Some(active_block_metadata) = self.active_block_metadata.as_ref() {
            let Some(session) = active_block_metadata
                .session_id()
                .and_then(|id| self.sessions.as_ref(ctx).get(id))
            else {
                return;
            };
            if !session.is_local() {
                return;
            }

            let command = block_completed.command.clone();
            let working_directory = active_block_metadata
                .current_working_directory()
                .map(Into::into);
            let command_case_sensitivity = session.command_case_sensitivity();
            let escape_char = session.shell_family().escape_char();
            ctx.spawn(
                async move {
                    check_openable_in_warp(
                        command,
                        working_directory,
                        command_case_sensitivity,
                        escape_char,
                    )
                    .await
                },
                move |view, maybe_match, ctx| {
                    if let Some(openable_path) = maybe_match {
                        if matches!(openable_path.file_type, OpenableFileType::Markdown) {
                            view.suggest_open_in_rift(openable_path, session, ctx);
                        }
                    }
                },
            );
        }
    }

    /// Whether or not the "Open in Rift" banner is open.
    #[cfg(feature = "integration_tests")]
    pub fn is_open_in_rift_banner_open(&self) -> bool {
        self.inline_banners_state.open_in_rift_banner.is_some()
    }

    fn close_open_in_rift_banner(&mut self, banner_id: usize) {
        self.model
            .lock()
            .block_list_mut()
            .remove_inline_banner(banner_id);
    }

    fn open_in_rift_banner_type_dismissed(
        &self,
        file_type: OpenableFileType,
        ctx: &ViewContext<Self>,
    ) -> bool {
        let general_settings = GeneralSettings::as_ref(ctx);
        match file_type {
            OpenableFileType::Markdown => {
                *general_settings.open_in_rift_banner_dismissed_for_markdown
            }
            OpenableFileType::Code | OpenableFileType::Text => {
                *general_settings.open_in_rift_banner_dismissed_for_code_and_text
            }
        }
    }

    /// Insert a suggestion banner for opening the file `openable_path`, originating from
    /// `session`, in a Rift pane.
    fn suggest_open_in_rift(
        &mut self,
        openable_path: OpenablePath,
        session: Arc<Session>,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.open_in_rift_banner_type_dismissed(openable_path.file_type, ctx) {
            return;
        }

        // We only show a banner for the most recent command.
        if let Some(prev_state) = &self.inline_banners_state.open_in_rift_banner {
            self.close_open_in_rift_banner(prev_state.id);
        }

        let banner_id = self.inline_banners_state.next_banner_id();
        self.inline_banners_state.open_in_rift_banner = Some(OpenInRiftBannerState::new(
            banner_id,
            openable_path,
            session,
        ));
        self.model
            .lock()
            .block_list_mut()
            .append_inline_banner(InlineBannerItem::new(
                banner_id,
                InlineBannerType::OpenInRift,
            ));
        ctx.notify();
    }

    pub fn handle_open_in_rift_banner_action(
        &mut self,
        action: OpenInRiftBannerAction,
        ctx: &mut ViewContext<Self>,
    ) {
        match action {
            OpenInRiftBannerAction::OpenFile => {
                if let Some(banner_state) = self.inline_banners_state.open_in_rift_banner.take() {
                    match banner_state.target.file_type {
                        OpenableFileType::Markdown => {
                            ctx.emit(Event::OpenFileInWarp {
                                path: banner_state.target.path,
                                session: banner_state.session,
                            });
                        }
                        OpenableFileType::Code | OpenableFileType::Text => {
                            ctx.emit(Event::OpenFileInWarp {
                                path: banner_state.target.path,
                                session: banner_state.session,
                            });
                        }
                    }
                    self.close_open_in_rift_banner(banner_state.id);
                    ctx.notify();
                }
            }
            OpenInRiftBannerAction::LearnMore => {
                if let Some(banner_state) = &self.inline_banners_state.open_in_rift_banner {
                    let url = match banner_state.target.file_type {
                        OpenableFileType::Markdown => LEARN_MORE_MARKDOWN_URL,
                        OpenableFileType::Code | OpenableFileType::Text => LEARN_MORE_CODE_URL,
                    };
                    ctx.open_url(url);
                }
            }
            OpenInRiftBannerAction::Close => {
                if let Some(banner_state) = self.inline_banners_state.open_in_rift_banner.take() {
                    self.close_open_in_rift_banner(banner_state.id);
                    match banner_state.target.file_type {
                        OpenableFileType::Markdown => {
                            GeneralSettings::handle(ctx).update(ctx, |settings, ctx| {
                                report_if_error!(settings
                                    .open_in_rift_banner_dismissed_for_markdown
                                    .set_value(true, ctx));
                            });
                        }
                        OpenableFileType::Code | OpenableFileType::Text => {
                            GeneralSettings::handle(ctx).update(ctx, |settings, ctx| {
                                report_if_error!(settings
                                    .open_in_rift_banner_dismissed_for_code_and_text
                                    .set_value(true, ctx));
                            });
                        }
                    }
                    ctx.notify();
                }
            }
        }
    }

    pub fn open_in_rift_banner_accessibility_content(
        &self,
        action: OpenInRiftBannerAction,
    ) -> ActionAccessibilityContent {
        match action {
            OpenInRiftBannerAction::OpenFile => {
                match &self.inline_banners_state.open_in_rift_banner {
                    Some(banner_state) => {
                        ActionAccessibilityContent::Custom(AccessibilityContent::new_without_help(
                            format!("Open {} in Warp", banner_state.target.path.display()),
                            WarpA11yRole::UserAction,
                        ))
                    }
                    None => ActionAccessibilityContent::Empty,
                }
            }
            OpenInRiftBannerAction::Close => {
                ActionAccessibilityContent::Custom(AccessibilityContent::new_without_help(
                    "Close View in Warp banner",
                    WarpA11yRole::UserAction,
                ))
            }
            OpenInRiftBannerAction::LearnMore => {
                ActionAccessibilityContent::Custom(AccessibilityContent::new(
                    "Learn more",
                    "Learn more about opening Markdown files in Warp",
                    WarpA11yRole::UserAction,
                ))
            }
        }
    }
}

lazy_static! {
    static ref FILE_VIEWER_COMMANDS: HashSet<&'static str> =
        HashSet::from(["bat", "cat", "glow", "less", "open"]);
}

/// Examines `command` for a file openable in Rift, returning the resolved path and type if found.
async fn check_openable_in_warp(
    command: String,
    working_directory: Option<String>,
    command_case_sensitivity: TopLevelCommandCaseSensitivity,
    escape_char: EscapeChar,
) -> Option<OpenablePath> {
    // We can use PathBuf/Path here because, at the moment, only local sessions are supported.
    let working_directory = working_directory.map(PathBuf::from);
    for command in all_parsed_commands(command, escape_char) {
        // We want to parse the command enough to distinguish file names from arguments, but no
        // more than necessary.
        // TODO(ben): Expand aliases as well.
        let mut tokens = command.parts.iter().map(|s| s.as_str()).collect_vec();
        let command_registry = CommandRegistry::global_instance();
        let Some(classified_command) = classify_command(
            command.clone(),
            &mut tokens,
            &command_registry,
            command_case_sensitivity,
        ) else {
            continue;
        };
        if !FILE_VIEWER_COMMANDS.contains(classified_command.command.command_name_span().item) {
            continue;
        }

        // All the supported viewers take files as positional arguments.
        let positionals = match classified_command.command {
            Command::Classified(shell_command) => shell_command.args.positionals,
            Command::Unclassified(command) => command.args.positionals,
        };

        if let Some(positionals) = positionals {
            for arg in positionals.iter() {
                // Skip commands and environment variables.
                if !matches!(
                    arg.expression(),
                    Expression::Literal | Expression::ValidatableArgument(_) | Expression::Unknown
                ) {
                    continue;
                }

                let relative_path = Path::new(arg.value().as_str());

                let Some(file_type) = is_file_openable_in_warp(relative_path) else {
                    continue;
                };

                let resolved = working_directory.as_ref().map_or_else(
                    || relative_path.to_path_buf(),
                    |cwd| cwd.join(relative_path),
                );

                if async_fs::metadata(&resolved).await.is_ok() {
                    // We've found a file that exists and can be opened in Rift.
                    return Some(OpenablePath {
                        path: resolved,
                        file_type,
                    });
                }
            }
        }
    }
    None
}
