
use markdown_parser::{FormattedText, FormattedTextFragment, FormattedTextLine};
use rift_core::ui::theme::WarpTheme;
use riftui::elements::{
    Border, Container, CrossAxisAlignment, Flex, FormattedTextElement, HighlightedHyperlink,
    Hoverable, Icon, MainAxisAlignment, MainAxisSize, MouseStateHandle, ParentElement,
};
use riftui::keymap::FixedBinding;
use riftui::ui_components::components::UiComponent;
use riftui::ui_components::toggle_menu::ToggleMenuStateHandle;
use riftui::{
    AppContext, BlurContext, Element, Entity, FocusContext, SingletonEntity, TypedActionView, View,
    ViewContext,
};

use crate::appearance::Appearance;
use crate::terminal::model::ansi::SystemDetails;
use crate::terminal::model::escape_sequences;
use crate::terminal::warpify::render;
use crate::terminal::warpify::settings::WarpifySettings;
use crate::ui_components::blended_colors;
use crate::ui_components::icons::Icon as UiIcon;

/// Status of the local install-tmux script confirmation prompt.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RequestedScriptStatus {
    /// Waiting for the user to accept or dismiss the install prompt.
    #[default]
    WaitingForUser,
    /// The install script is currently running.
    Running,
}

pub const WHY_INSTALL_TMUX_URL: &str =
    "https://docs.warp.dev/terminal/warpify/ssh#why-do-i-need-tmux-on-the-remote-machine";

#[derive(Debug, Clone)]
pub struct TmuxInstallMethod {
    pub script: String,
    pub should_use_package_manager: bool,
}

#[derive(Debug, Clone)]
pub enum SshInstallTmuxBlockEvent {
    InstallTmuxAndWarpify(TmuxInstallMethod),
    ToggleScriptVisibility,
    Cancel,
    Interrupt,
    ToggleTmuxInstallVisibility,
    UnhideTmuxInstall,
}

#[derive(Debug, Clone)]
pub enum ScriptTarget {
    First,
    Second,
    Toggle,
}

#[derive(Debug, Clone)]
pub enum SshInstallTmuxBlockAction {
    SetInstallScriptChoice(ScriptTarget),
    OnToggleInstallScriptChoice,
    InstallTmux,
    /// If the script is pending, this means show or hide the full script.
    /// If the script is running, this means show or hide the detail (ie, the long-running block).
    ToggleVisibility,
    AddSshHostToDenylist(String),
    Cancel,
    Interrupt,
    Focus,
}

pub struct SshKeyEvent {
    is_ctrl_c: bool,
}

impl SshKeyEvent {
    pub fn from_chars(chars: &str) -> Self {
        Self {
            is_ctrl_c: chars == "\x03",
        }
    }

    pub fn from_bytes(chars: &[u8]) -> Self {
        Self {
            is_ctrl_c: chars == [escape_sequences::C0::ETX],
        }
    }

    pub fn is_ctrl_c(&self) -> bool {
        self.is_ctrl_c
    }
}

pub struct SshInstallTmuxBlock {
    install_button_mouse_state: MouseStateHandle,
    skip_button_mouse_state: MouseStateHandle,
    why_install_tmux_highlight_index: HighlightedHyperlink,
    never_warpify_mouse_state_handle: MouseStateHandle,
    block_mouse_state: MouseStateHandle,
    is_focused: bool,
    is_collapsed: bool,
    show_tmux_install_block: bool,
    script_status: RequestedScriptStatus,
    system_details: SystemDetails,
    /// The script to install tmux locally, in a ~/.rift directory
    tmux_local_install_script: String,
    ssh_host: Option<String>,
    ssh_command: String,
    system_install_state: Option<SystemInstallState>,
    outdated_version: bool,
}

pub struct SystemInstallState {
    /// The script to install tmux via a package manager, which requires root access
    tmux_system_install_script: String,
    toggle_menu_mouse_states: Vec<MouseStateHandle>,
    toggle_menu_state_handle: ToggleMenuStateHandle,
    is_first_script_active: bool,
}

pub fn init(app: &mut AppContext) {
    use riftui::keymap::macros::*;

    app.register_fixed_bindings([
        FixedBinding::new(
            "enter",
            SshInstallTmuxBlockAction::InstallTmux,
            id!(SshInstallTmuxBlock::ui_name()),
        ),
        FixedBinding::new(
            "escape",
            SshInstallTmuxBlockAction::Cancel,
            id!(SshInstallTmuxBlock::ui_name()),
        ),
        FixedBinding::new(
            "ctrl-c",
            SshInstallTmuxBlockAction::Interrupt,
            id!(SshInstallTmuxBlock::ui_name()),
        ),
        FixedBinding::new(
            "down",
            SshInstallTmuxBlockAction::ToggleVisibility,
            id!(SshInstallTmuxBlock::ui_name()),
        ),
        FixedBinding::new(
            "tab",
            SshInstallTmuxBlockAction::SetInstallScriptChoice(ScriptTarget::Toggle),
            id!(SshInstallTmuxBlock::ui_name()),
        ),
        FixedBinding::new(
            "left",
            SshInstallTmuxBlockAction::SetInstallScriptChoice(ScriptTarget::First),
            id!(SshInstallTmuxBlock::ui_name()),
        ),
        FixedBinding::new(
            "right",
            SshInstallTmuxBlockAction::SetInstallScriptChoice(ScriptTarget::Second),
            id!(SshInstallTmuxBlock::ui_name()),
        ),
    ]);
}

impl SshInstallTmuxBlock {
    #[allow(clippy::new_without_default)]
    pub fn new(
        system_details: SystemDetails,
        tmux_local_install_script: String,
        tmux_system_install_script: Option<String>,
        ssh_command: String,
        ssh_host: Option<String>,
        outdated_version: bool,
    ) -> Self {
        Self {
            install_button_mouse_state: Default::default(),
            skip_button_mouse_state: Default::default(),
            why_install_tmux_highlight_index: Default::default(),
            never_warpify_mouse_state_handle: Default::default(),
            block_mouse_state: Default::default(),
            is_focused: false,
            is_collapsed: true,
            show_tmux_install_block: false,
            script_status: RequestedScriptStatus::WaitingForUser,
            system_details,
            tmux_local_install_script,
            ssh_host,
            ssh_command,
            outdated_version,
            system_install_state: tmux_system_install_script.map(|tmux_root_install_script| {
                SystemInstallState {
                    tmux_system_install_script: tmux_root_install_script,
                    toggle_menu_mouse_states: vec![Default::default(), Default::default()],
                    toggle_menu_state_handle: Default::default(),
                    is_first_script_active: true,
                }
            }),
        }
    }

    pub fn get_install_method(&self) -> TmuxInstallMethod {
        if let Some(ref system_install_state) = self.system_install_state {
            // The user has selected the first script, which is the system install
            if system_install_state.is_first_script_active {
                return TmuxInstallMethod {
                    script: system_install_state.tmux_system_install_script.clone(),
                    should_use_package_manager: true,
                };
            }
        }
        TmuxInstallMethod {
            script: self.tmux_local_install_script.clone(),
            should_use_package_manager: false,
        }
    }

    pub fn focus(&self, ctx: &mut ViewContext<Self>) {
        ctx.focus_self();
        ctx.notify();
    }

    pub fn system_details(&self) -> SystemDetails {
        self.system_details.clone()
    }

    pub fn emit_install_tmux(
        &mut self,
        install_method: TmuxInstallMethod,
        ctx: &mut ViewContext<Self>,
    ) {
        self.script_status = RequestedScriptStatus::Running;
        ctx.emit(SshInstallTmuxBlockEvent::InstallTmuxAndWarpify(
            install_method,
        ));
        ctx.notify()
    }
}

impl Entity for SshInstallTmuxBlock {
    type Event = SshInstallTmuxBlockEvent;
}

impl SshInstallTmuxBlock {
    /// Returns `true` if the script was previously visible and is now collapsed.
    pub fn collapse_script(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        let was_expanded = !self.is_collapsed;
        if was_expanded {
            self.is_collapsed = true;
            ctx.notify();
            return true;
        }
        false
    }

    fn render_system_install_ui(
        &self,
        SystemInstallState {
            is_first_script_active,
            tmux_system_install_script,
            ..
        }: &SystemInstallState,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let package_manager = &self.system_details.package_manager;
        let title = if *is_first_script_active {
            format!("Install with {package_manager}")
        } else {
            "Install to ~/.rift".to_string()
        };
        let script = if *is_first_script_active {
            tmux_system_install_script
        } else {
            &self.tmux_local_install_script
        };
        self.render_install_prompt(&title, script, app)
    }

    fn render_local_install_ui(&self, app: &AppContext) -> Box<dyn Element> {
        self.render_install_prompt(
            "Run this script to install tmux?",
            &self.tmux_local_install_script.clone(),
            app,
        )
    }

    /// Renders the install-tmux prompt: the script text plus Install / Skip buttons.
    fn render_install_prompt(
        &self,
        title: &str,
        script: &str,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::handle(app).as_ref(app);
        let theme = appearance.theme();
        let is_running = matches!(self.script_status, RequestedScriptStatus::Running);

        let script_block = Container::new(
            riftui::elements::Text::new(
                script.to_owned(),
                appearance.monospace_font_family(),
                appearance.monospace_font_size() - 1.,
            )
            .with_color(theme.main_text_color(theme.background()).into_solid())
            .finish(),
        )
        .with_uniform_padding(8.)
        .with_background(theme.surface_1())
        .with_corner_radius(riftui::elements::CornerRadius::with_all(
            riftui::elements::Radius::Pixels(6.),
        ))
        .finish();

        let install_button = appearance
            .ui_builder()
            .button(
                riftui::ui_components::button::ButtonVariant::Text,
                self.install_button_mouse_state.clone(),
            )
            .with_text_label(if is_running { "Installing…" } else { "Install" }.to_string())
            .build()
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(SshInstallTmuxBlockAction::InstallTmux);
            })
            .finish();

        let skip_button = appearance
            .ui_builder()
            .button(
                riftui::ui_components::button::ButtonVariant::Text,
                self.skip_button_mouse_state.clone(),
            )
            .with_text_label("Skip".to_string())
            .build()
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(SshInstallTmuxBlockAction::Cancel);
            })
            .finish();

        let buttons = Flex::row()
            .with_spacing(8.)
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_child(skip_button)
            .with_child(install_button)
            .finish();

        Container::new(
            Flex::column()
                .with_spacing(8.)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(
                    riftui::elements::Text::new(
                        title.to_owned(),
                        appearance.ui_font_family(),
                        appearance.monospace_font_size(),
                    )
                    .with_color(theme.main_text_color(theme.background()).into_solid())
                    .finish(),
                )
                .with_child(script_block)
                .with_child(buttons)
                .finish(),
        )
        .with_margin_top(16.)
        .finish()
    }

    fn render_title_ui(
        &self,
        app: &AppContext,
        theme: &WarpTheme,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let header_contents = render::build_header_row(
            "Install tmux?",
            Icon::new(UiIcon::Warp.into(), theme.active_ui_detail()),
            theme,
            appearance,
        )
        .with_margin_right(8.)
        .finish();

        let is_awaiting_action = self.script_status == RequestedScriptStatus::WaitingForUser;

        let right_hand_size = is_awaiting_action
            .then(|| {
                render::render_never_warpify_ssh_link(
                    &self.ssh_host,
                    app,
                    appearance,
                    self.never_warpify_mouse_state_handle.clone(),
                    move |ctx, ssh_host| {
                        ctx.dispatch_typed_action(SshInstallTmuxBlockAction::AddSshHostToDenylist(
                            ssh_host.to_owned(),
                        ));
                    },
                )
            })
            .flatten();

        let mut row = Flex::row()
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_cross_axis_alignment(CrossAxisAlignment::End)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(header_contents);

        if let Some(right_hand_size) = right_hand_size {
            row.add_child(right_hand_size);
        }

        render::apply_spacing_styles(Container::new(row.finish())).finish()
    }
}

impl View for SshInstallTmuxBlock {
    fn ui_name() -> &'static str {
        "SshInstallTmuxBlock"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        let mut content = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        content.add_child(self.render_title_ui(app, theme, appearance));

        content.add_child(
            render::build_command_row(self.ssh_command.clone(), theme, appearance, false).finish(),
        );

        let explanation = if self.outdated_version {
            "In order to Warpify your SSH session, a more recent version of tmux (>=3.0) must be installed. "
        } else {
            "In order to Warpify your SSH session, tmux must be installed. "
        };

        let warpify_description = vec![
            FormattedTextFragment::plain_text(explanation),
            FormattedTextFragment::hyperlink("Why do I need tmux?", WHY_INSTALL_TMUX_URL),
        ];

        let text_color =
            blended_colors::text_sub(appearance.theme(), appearance.theme().surface_1());

        let warpify_description = FormattedTextElement::new(
            FormattedText::new([FormattedTextLine::Line(warpify_description)]),
            appearance.monospace_font_size(),
            appearance.monospace_font_family(),
            appearance.monospace_font_family(),
            text_color,
            self.why_install_tmux_highlight_index.clone(),
        )
        .with_hyperlink_font_color(appearance.theme().accent().into_solid())
        .register_default_click_handlers(|url, _, ctx| {
            ctx.open_url(&url.url);
        })
        .finish();

        content
            .add_child(render::apply_spacing_styles(Container::new(warpify_description)).finish());

        if let Some(root_install_state) = &self.system_install_state {
            content.add_child(self.render_system_install_ui(root_install_state, app));
        } else {
            content.add_child(self.render_local_install_ui(app));
        }

        Hoverable::new(self.block_mouse_state.clone(), |_| {
            Container::new(content.finish())
                .with_padding_top(10.)
                .with_background(theme.foreground().with_opacity(10))
                .with_border(Border::top(1.).with_border_fill(theme.outline()))
                .finish()
        })
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(SshInstallTmuxBlockAction::Focus);
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

impl TypedActionView for SshInstallTmuxBlock {
    type Action = SshInstallTmuxBlockAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        let is_pending = self.script_status == RequestedScriptStatus::WaitingForUser;
        match (action, is_pending) {
            (SshInstallTmuxBlockAction::Cancel, true) => ctx.emit(SshInstallTmuxBlockEvent::Cancel),
            (SshInstallTmuxBlockAction::OnToggleInstallScriptChoice, true) => {
                if let Some(ref mut root_install_state) = self.system_install_state {
                    root_install_state.is_first_script_active =
                        !root_install_state.is_first_script_active;
                }
            }
            (SshInstallTmuxBlockAction::SetInstallScriptChoice(target), true) => {
                if let Some(ref mut root_install_state) = self.system_install_state {
                    let new_index = match target {
                        ScriptTarget::First => 0,
                        ScriptTarget::Second => 1,
                        ScriptTarget::Toggle => root_install_state.is_first_script_active as usize,
                    };
                    root_install_state
                        .toggle_menu_state_handle
                        .set_selected_idx(new_index);
                    root_install_state.is_first_script_active = new_index == 0;
                }
                ctx.notify();
            }
            (SshInstallTmuxBlockAction::ToggleVisibility, true) => {
                self.is_collapsed = !self.is_collapsed;
                ctx.focus_self();
                ctx.emit(SshInstallTmuxBlockEvent::ToggleScriptVisibility);
                ctx.notify();
            }
            (SshInstallTmuxBlockAction::ToggleVisibility, false) => {
                self.show_tmux_install_block = !self.show_tmux_install_block;
                ctx.emit(SshInstallTmuxBlockEvent::ToggleTmuxInstallVisibility);
                ctx.notify();
            }
            (SshInstallTmuxBlockAction::InstallTmux, true) => {
                let selected_root_access_option = self.get_install_method();
                self.is_collapsed = true;
                self.show_tmux_install_block = true;
                ctx.emit(SshInstallTmuxBlockEvent::UnhideTmuxInstall);
                self.emit_install_tmux(selected_root_access_option, ctx);
            }
            (SshInstallTmuxBlockAction::Interrupt, _) => {
                ctx.emit(SshInstallTmuxBlockEvent::Interrupt);
            }
            (SshInstallTmuxBlockAction::AddSshHostToDenylist(ssh_host), true) => {
                let settings = WarpifySettings::handle(ctx);
                settings.update(ctx, |warpify, ctx| {
                    warpify.denylist_ssh_host(ssh_host, ctx);
                });
                ctx.emit(SshInstallTmuxBlockEvent::Cancel);
                ctx.notify();
            }
            (SshInstallTmuxBlockAction::Focus, _) => {
                self.focus(ctx);
            }
            (_, false) => {}
        }
    }
}

/// If we have an "install tmux" script bundled into the app that matches the system details, then returns
/// the script as a string. Otherwise, returns None.
#[cfg(not(test))]
#[allow(unused_variables)]
pub fn install_tmux_script(system: &SystemDetails, app: &AppContext) -> Option<String> {
    use asset_macro::bundled_asset;
    use riftui::assets::asset_cache::{AssetCache, AssetState};

    let asset_source = match (
        system.operating_system.as_str(),
        system.package_manager.as_str(),
        system.shell.as_str(),
    ) {
        ("Linux", _, "bash" | "zsh") => {
            bundled_asset!("ssh/bash_zsh/install_tmux_and_warpify_linux.sh")
        }
        ("Linux", _, "fish") => {
            bundled_asset!("ssh/fish/install_tmux_and_warpify_linux.sh")
        }
        ("Darwin", "homebrew", "bash" | "zsh") => {
            bundled_asset!("ssh/bash_zsh/install_tmux_and_warpify_brew.sh")
        }
        ("Darwin", "homebrew", "fish") => {
            bundled_asset!("ssh/fish/install_tmux_and_warpify_brew.sh")
        }
        _ => return None,
    };

    match AssetCache::as_ref(app).load_asset::<String>(asset_source) {
        AssetState::Loaded { data } => Some(data.to_string()),
        _ => panic!("install tmux script should be available as a string"),
    }
}

/// If we have an "install tmux via root" script bundled into the app that matches the system details, then returns
/// the script as a string. Otherwise, returns None.
#[cfg(not(test))]
#[allow(unused_variables)]
pub fn install_root_tmux_script(
    system: &SystemDetails,
    app: &AppContext,
    can_run_sudo: bool,
) -> Option<String> {
    use asset_macro::bundled_asset;
    use riftui::assets::asset_cache::{AssetCache, AssetState};

    let asset_source = match (
        system.operating_system.as_str(),
        system.package_manager.as_str(),
        system.shell.as_str(),
    ) {
        ("Linux", "apt", "bash" | "zsh") => {
            bundled_asset!("ssh/bash_zsh/root/install_tmux_and_warpify_apt.sh")
        }
        ("Linux", "dnf", "bash" | "zsh") => {
            bundled_asset!("ssh/bash_zsh/root/install_tmux_and_warpify_dnf.sh")
        }
        ("Linux", "pacman", "bash" | "zsh") => {
            bundled_asset!("ssh/bash_zsh/root/install_tmux_and_warpify_pacman.sh")
        }
        ("Linux", "yum", "bash" | "zsh") => {
            bundled_asset!("ssh/bash_zsh/root/install_tmux_and_warpify_yum.sh")
        }
        ("Linux", "zypper", "bash" | "zsh") => {
            bundled_asset!("ssh/bash_zsh/root/install_tmux_and_warpify_zypper.sh")
        }
        _ => return None,
    };

    let asset_source = match AssetCache::as_ref(app).load_asset::<String>(asset_source) {
        AssetState::Loaded { data } => data.to_string(),
        _ => panic!("install tmux script should be available as a string"),
    };
    if !can_run_sudo {
        return Some(asset_source.replace("sudo ", ""));
    }
    Some(asset_source)
}

/// This method has a separate test-only implementation so we don't try to access a bundled
/// asset when executing a unit test
#[cfg(test)]
#[allow(unused_variables)]
pub fn install_tmux_script(system: &SystemDetails, app: &AppContext) -> Option<String> {
    None
}

/// This method has a separate test-only implementation so we don't try to access a bundled
/// asset when executing a unit test
#[cfg(test)]
#[allow(unused_variables)]
pub fn install_root_tmux_script(
    system: &SystemDetails,
    app: &AppContext,
    can_run_sudo: bool,
) -> Option<String> {
    None
}
