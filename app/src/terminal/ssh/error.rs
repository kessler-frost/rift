use markdown_parser::{FormattedText, FormattedTextFragment, FormattedTextLine};
use rift_core::channel::ChannelState;
use rift_core::ui::theme::RiftTheme;
use riftui::elements::{
    Border, Container, CrossAxisAlignment, Flex, HighlightedHyperlink, Hoverable, Icon,
    MainAxisAlignment, MainAxisSize, MouseStateHandle, ParentElement,
};
use riftui::keymap::FixedBinding;
use riftui::platform::Cursor;
use riftui::ui_components::button::ButtonVariant;
use riftui::ui_components::components::{UiComponent, UiComponentStyles};
use riftui::{
    AppContext, BlurContext, Element, Entity, FocusContext, SingletonEntity, TypedActionView, View,
    ViewContext,
};

use crate::appearance::Appearance;
use crate::terminal::model::ansi::RiftificationUnavailableReason;
use crate::terminal::riftify;
use crate::terminal::riftify::render::{apply_spacing_styles, build_description_row};
use crate::terminal::riftify::settings::RiftifySettings;
use crate::ui_components::icons::Icon as UiIcon;

const TMUX_NOT_INSTALLED_ERROR: &str =
    "tmux is not installed on the remote machine. Please install tmux and try again.";
const UNSUPPORTED_TMUX_VERSION_ERROR: &str =
    "The tmux version available on the remote machine is below 3.0. Please install tmux 3.0 or greater using a different method and try again.";
const TMUX_FAILED_ERROR: &str =
    "tmux failed to execute on the remote machine. Please re-install tmux and try again.";
const RIFTIFY_TIMEOUT_ERROR: &str = "Riftifying the session hit a timeout.";
const UNSUPPORTED_SHELL_ERROR: &str =
    "Unsupported shell. Please set bash, zsh, or fish as your default shell and try again.";
const TMUX_INSTALL_FAILED_ERROR: &str =
    "The tmux install hit an unexpected error. Please install tmux manually and try again.";

const SSH_GITHUB_ISSUE_URL: &str = "https://github.com/kessler-frost/rift/issues/new";

fn get_ssh_github_issue_url(title: &str) -> String {
    let url = if let Some(version) = ChannelState::app_version() {
        format!("{SSH_GITHUB_ISSUE_URL}&warp-version={version}")
    } else {
        SSH_GITHUB_ISSUE_URL.to_string()
    };
    // prepend the title with "SSH tmux bug report: " and uri encode it
    let title = format!("SSH tmux bug report: {title:?}");
    let title = urlencoding::encode(&title);
    format!("{url}&title={title}")
}

impl RiftificationUnavailableReason {
    fn error_message(&self) -> &'static str {
        match self {
            RiftificationUnavailableReason::TmuxNotInstalled { .. } => TMUX_NOT_INSTALLED_ERROR,
            RiftificationUnavailableReason::UnsupportedTmuxVersion { .. } => {
                UNSUPPORTED_TMUX_VERSION_ERROR
            }
            RiftificationUnavailableReason::TmuxFailed => TMUX_FAILED_ERROR,
            RiftificationUnavailableReason::Timeout { .. } => RIFTIFY_TIMEOUT_ERROR,
            RiftificationUnavailableReason::UnsupportedShell { .. } => UNSUPPORTED_SHELL_ERROR,
            RiftificationUnavailableReason::TmuxInstallFailed { .. } => TMUX_INSTALL_FAILED_ERROR,
        }
    }

    fn error_title(&self) -> &'static str {
        match self {
            RiftificationUnavailableReason::TmuxNotInstalled { .. } => "tmux Not Installed",
            RiftificationUnavailableReason::UnsupportedTmuxVersion { .. } => {
                "Unsupported Tmux Version"
            }
            RiftificationUnavailableReason::TmuxFailed => "tmux Failed",
            RiftificationUnavailableReason::Timeout {
                is_tmux_install, ..
            } => {
                if *is_tmux_install {
                    "tmux Install Timeout"
                } else {
                    "SSH Riftify Timeout"
                }
            }
            RiftificationUnavailableReason::UnsupportedShell { .. } => "Unsupported Shell",
            RiftificationUnavailableReason::TmuxInstallFailed { .. } => "tmux Install Failed",
        }
    }
}

#[derive(Debug, Clone)]
pub enum SshErrorBlockEvent {
    ContinueWithoutRiftification,
    RiftifyWithoutTmux,
}

#[derive(Debug, Clone)]
pub enum SshErrorBlockAction {
    ContinueWithoutRiftification,
    RiftifyWithoutTmux,
    OpenUrl(String),
    AddSshHostToDenylist(String),
    Focus,
}

pub struct SshErrorBlock {
    error_reason: RiftificationUnavailableReason,
    ssh_host: Option<String>,
    riftify_without_tmux_button_mouse_state: MouseStateHandle,
    continue_button_mouse_state: MouseStateHandle,
    report_link_highlight_index: HighlightedHyperlink,
    never_riftify_mouse_state_handle: MouseStateHandle,
    block_mouse_state: MouseStateHandle,
    is_focused: bool,
}

pub fn init(app: &mut AppContext) {
    use riftui::keymap::macros::*;

    app.register_fixed_bindings([
        FixedBinding::new(
            "enter",
            SshErrorBlockAction::RiftifyWithoutTmux,
            id!(SshErrorBlock::ui_name()),
        ),
        FixedBinding::new(
            "escape",
            SshErrorBlockAction::ContinueWithoutRiftification,
            id!(SshErrorBlock::ui_name()),
        ),
        FixedBinding::new(
            "ctrl-c",
            SshErrorBlockAction::ContinueWithoutRiftification,
            id!(SshErrorBlock::ui_name()),
        ),
    ]);
}

impl SshErrorBlock {
    #[allow(clippy::new_without_default)]
    pub fn new(error_reason: RiftificationUnavailableReason, ssh_host: Option<String>) -> Self {
        Self {
            error_reason,
            ssh_host,
            riftify_without_tmux_button_mouse_state: Default::default(),
            continue_button_mouse_state: Default::default(),
            report_link_highlight_index: Default::default(),
            never_riftify_mouse_state_handle: Default::default(),
            block_mouse_state: Default::default(),
            is_focused: false,
        }
    }

    pub fn focus(&self, ctx: &mut ViewContext<Self>) {
        ctx.focus_self();
        ctx.notify();
    }

    fn should_show_report_to_rift_button(&self) -> bool {
        matches!(
            self.error_reason,
            RiftificationUnavailableReason::Timeout { .. }
                | RiftificationUnavailableReason::TmuxInstallFailed { .. }
        )
    }

    fn render_title_ui(
        &self,
        app: &AppContext,
        theme: &RiftTheme,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let header_contents = riftify::render::build_header_row(
            "Error Riftifying session",
            Icon::new(UiIcon::AlertTriangle.into(), theme.ui_error_color()),
            theme,
            appearance,
        )
        .with_margin_right(8.)
        .finish();

        let right_hand_size = riftify::render::render_never_riftify_ssh_link(
            &self.ssh_host,
            app,
            appearance,
            self.never_riftify_mouse_state_handle.clone(),
            move |ctx, ssh_host| {
                ctx.dispatch_typed_action(SshErrorBlockAction::AddSshHostToDenylist(
                    ssh_host.to_owned(),
                ));
            },
        );

        let mut row = Flex::row()
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_cross_axis_alignment(CrossAxisAlignment::End)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(header_contents);

        if let Some(right_hand_size) = right_hand_size {
            row.add_child(right_hand_size);
        }

        riftify::render::apply_spacing_styles(Container::new(row.finish())).finish()
    }
}

impl Entity for SshErrorBlock {
    type Event = SshErrorBlockEvent;
}

pub const SSH_ERROR_BLOCK_VISIBLE_KEY: &str = "SshErrorBlockVisible";

impl View for SshErrorBlock {
    fn ui_name() -> &'static str {
        "SshErrorBlock"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let mut content = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        content.add_child(self.render_title_ui(app, theme, appearance));

        content.add_child(riftify::render::description_row(
            self.error_reason.error_message(),
            theme,
            appearance,
        ));

        let ui_builder = appearance.ui_builder();

        if self.should_show_report_to_rift_button() {
            let report_issue_text = build_description_row(FormattedText::new([FormattedTextLine::Line(vec![
                    FormattedTextFragment::plain_text("We are actively working on improving the stability of SSH in Warp. Please consider "),
                    FormattedTextFragment::hyperlink("filing an issue", get_ssh_github_issue_url(self.error_reason.error_title())),
                    FormattedTextFragment::plain_text(" on GitHub so we can better identify the problem."),
                ])]),
                theme, appearance, self.report_link_highlight_index.clone())
                .with_hyperlink_font_color(theme.accent().into())
                .register_default_click_handlers(|link, ctx, _| {
                    ctx.dispatch_typed_action(SshErrorBlockAction::OpenUrl(link.url));
                }).finish();
            content.add_child(apply_spacing_styles(Container::new(report_issue_text)).finish());
        }

        let buttons = Flex::row()
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(
                Container::new(
                    ui_builder
                        .button(
                            ButtonVariant::Accent,
                            self.riftify_without_tmux_button_mouse_state.clone(),
                        )
                        .with_centered_text_label("Riftify without TMUX".into())
                        .with_style(UiComponentStyles {
                            font_size: Some(appearance.monospace_font_size()),
                            ..Default::default()
                        })
                        .build()
                        .with_cursor(Cursor::PointingHand)
                        .on_click(move |ctx, _, _| {
                            ctx.dispatch_typed_action(SshErrorBlockAction::RiftifyWithoutTmux)
                        })
                        .finish(),
                )
                .with_margin_right(8.)
                .finish(),
            )
            .with_child(
                ui_builder
                    .button(
                        ButtonVariant::Secondary,
                        self.continue_button_mouse_state.clone(),
                    )
                    .with_centered_text_label("Continue without Riftification".into())
                    .with_style(UiComponentStyles {
                        font_size: Some(appearance.monospace_font_size()),
                        ..Default::default()
                    })
                    .build()
                    .with_cursor(Cursor::PointingHand)
                    .on_click(move |ctx, _, _| {
                        ctx.dispatch_typed_action(SshErrorBlockAction::ContinueWithoutRiftification)
                    })
                    .finish(),
            );

        content.add_child(
            Container::new(buttons.finish())
                .with_uniform_margin(20.)
                .finish(),
        );

        Hoverable::new(self.block_mouse_state.clone(), |_| {
            Container::new(content.finish())
                .with_padding_top(10.)
                .with_background(theme.foreground().with_opacity(10))
                .with_border(Border::top(1.).with_border_fill(theme.outline()))
                .finish()
        })
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(SshErrorBlockAction::Focus);
        })
        .finish()
    }

    fn on_focus(&mut self, focus_ctx: &FocusContext, ctx: &mut ViewContext<Self>) {
        if focus_ctx.is_self_focused() {
            self.is_focused = true;
            ctx.notify();
        }
    }

    fn on_blur(&mut self, blur_ctx: &BlurContext, ctx: &mut ViewContext<Self>) {
        if blur_ctx.is_self_blurred() {
            self.is_focused = false;
            ctx.notify();
        }
    }
}

impl TypedActionView for SshErrorBlock {
    type Action = SshErrorBlockAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            SshErrorBlockAction::RiftifyWithoutTmux => {
                ctx.emit(SshErrorBlockEvent::RiftifyWithoutTmux)
            }
            SshErrorBlockAction::ContinueWithoutRiftification => {
                ctx.emit(SshErrorBlockEvent::ContinueWithoutRiftification)
            }
            SshErrorBlockAction::OpenUrl(url) => {
                ctx.open_url(url);
            }
            SshErrorBlockAction::AddSshHostToDenylist(ssh_host) => {
                let settings = RiftifySettings::handle(ctx);
                settings.update(ctx, |riftify, ctx| {
                    riftify.denylist_ssh_host(ssh_host, ctx);
                });
                ctx.emit(SshErrorBlockEvent::ContinueWithoutRiftification);
                ctx.notify()
            }
            SshErrorBlockAction::Focus => {
                self.focus(ctx);
            }
        }
    }
}
