use riftui::windowing::{StateEvent, WindowManager};
use riftui::{Entity, ModelContext, SingletonEntity};

#[cfg(target_os = "macos")]
mod mac;

#[cfg(target_os = "macos")]
use mac::*;

#[allow(dead_code)]
#[cfg(not(target_os = "macos"))]
mod non_mac {
    pub fn can_become_default_terminal() -> bool {
        false
    }

    pub fn is_rift_default_terminal() -> bool {
        false
    }

    /// Sets Rift as the default terminal
    pub fn set_rift_as_default_terminal() -> Result<(), String> {
        Err("Not implemented".to_string())
    }
}

#[allow(unused_imports)]
#[cfg(not(target_os = "macos"))]
use non_mac::*;

pub struct DefaultTerminal {
    /// Whether the OS will treat Rift as the default app for scripts/executables.
    is_rift_default: bool,
}

impl DefaultTerminal {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        ctx.subscribe_to_model(
            &WindowManager::handle(ctx),
            Self::handle_window_manager_event,
        );

        // This can be slow to compute due to calling into platform APIs, so in unit
        // tests, where we shouldn't care, just pretend that we are not.
        let is_rift_default = if cfg!(test) {
            false
        } else {
            is_rift_default_terminal()
        };

        Self { is_rift_default }
    }

    /// This is an OS-level setting. Unlike most other settings, where Rift is the source-of-truth
    /// for the value of the setting, it can be changed outside of Rift. We monitor if it gets
    /// changed externally by checking when Rift is focused.
    fn handle_window_manager_event(&mut self, event: &StateEvent, ctx: &mut ModelContext<Self>) {
        match event {
            StateEvent::ValueChanged { current, previous } => {
                if current.active_window.is_some() && previous.active_window.is_none() {
                    let is_rift_default_now = is_rift_default_terminal();
                    if is_rift_default_now != self.is_rift_default {
                        self.set_is_rift_default(is_rift_default_now, ctx);
                    }
                }
            }
        }
    }

    fn set_is_rift_default(&mut self, value: bool, ctx: &mut ModelContext<Self>) {
        self.is_rift_default = value;
        ctx.emit(DefaultTerminalEvent::ValueChanged);
        ctx.notify();
    }

    pub fn can_rift_become_default() -> bool {
        if cfg!(test) {
            // Determining whether or not we can become the default terminal requires
            // calling into platform APIs, which can be slow, and we can't actually
            // set the default terminal in unit tests, so just say we can't.
            false
        } else {
            can_become_default_terminal()
        }
    }

    pub fn is_rift_default(&self) -> bool {
        self.is_rift_default
    }

    /// This is a one-way operation. Once we set the default terminal to Rift, we can't really
    /// "unset" it unless we pick a new default terminal. Picking a new default is complicated.
    pub fn make_rift_default(&mut self, ctx: &mut ModelContext<Self>) {
        if let Err(e) = set_rift_as_default_terminal() {
            log::error!("Error setting Warp as default terminal: {e:#}");
        } else {
            self.set_is_rift_default(true, ctx);
        }
    }
}

pub enum DefaultTerminalEvent {
    ValueChanged,
}

impl Entity for DefaultTerminal {
    type Event = DefaultTerminalEvent;
}

impl SingletonEntity for DefaultTerminal {}
