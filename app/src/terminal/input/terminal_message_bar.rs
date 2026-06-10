use std::sync::Arc;

use parking_lot::FairMutex;
use riftui::elements::{Container, Empty, Element};
use riftui::keymap::Keystroke;
use riftui::{AppContext, Entity, ModelHandle, View, ViewContext};

use super::buffer_model::InputBufferModel;
use super::message_bar::common::render_terminal_message;
use super::message_bar::{Message, MessageItem, MessageProvider};
use crate::terminal::input::inline_history::{AcceptHistoryItem, HistoryTab};
use crate::terminal::input::inline_menu::{InlineMenuModel, InlineMenuModelEvent};
use crate::terminal::input::suggestions_mode_model::{
    InputSuggestionsModeEvent, InputSuggestionsModeModel,
};
use crate::terminal::model::TerminalModel;

/// Renders contextual hint text at the bottom of the terminal input.
pub struct TerminalInputMessageBar {
    #[allow(dead_code)]
    terminal_model: Arc<FairMutex<TerminalModel>>,
    #[allow(dead_code)]
    input_buffer_model: ModelHandle<InputBufferModel>,
    suggestions_mode_model: ModelHandle<InputSuggestionsModeModel>,
    inline_history_model: ModelHandle<InlineMenuModel<AcceptHistoryItem, HistoryTab>>,
}

impl Entity for TerminalInputMessageBar {
    type Event = ();
}

impl TerminalInputMessageBar {
    pub fn new(
        terminal_model: Arc<FairMutex<TerminalModel>>,
        input_buffer_model: ModelHandle<InputBufferModel>,
        suggestions_mode_model: ModelHandle<InputSuggestionsModeModel>,
        inline_history_model: ModelHandle<InlineMenuModel<AcceptHistoryItem, HistoryTab>>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        ctx.subscribe_to_model(&input_buffer_model, |_, _, _, ctx| {
            ctx.notify();
        });
        ctx.subscribe_to_model(&suggestions_mode_model, |_, _, event, ctx| {
            let InputSuggestionsModeEvent::ModeChanged { .. } = event;
            ctx.notify();
        });
        ctx.subscribe_to_model(&inline_history_model, |_, _, event, ctx| {
            if let InlineMenuModelEvent::UpdatedSelectedItem = event {
                ctx.notify();
            }
        });

        Self {
            terminal_model,
            input_buffer_model,
            suggestions_mode_model,
            inline_history_model,
        }
    }
}

impl View for TerminalInputMessageBar {
    fn ui_name() -> &'static str {
        "TerminalInputMessageBar"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        if self
            .suggestions_mode_model
            .as_ref(app)
            .is_inline_history_menu()
        {
            let selected = self.inline_history_model.as_ref(app).selected_item();
            let message = InlineHistoryMessageProducer
                .produce_message(selected)
                .unwrap_or_default();
            return Container::new(render_terminal_message(message, app))
                .with_padding_bottom(8.)
                .with_padding_right(8.)
                .finish();
        }

        Container::new(Empty::new().finish()).finish()
    }
}


struct InlineHistoryMessageProducer;
impl MessageProvider<Option<&AcceptHistoryItem>> for InlineHistoryMessageProducer {
    fn produce_message(&self, selected: Option<&AcceptHistoryItem>) -> Option<Message> {
        let enter = MessageItem::keystroke(Keystroke {
            key: "enter".to_owned(),
            ..Default::default()
        });
        let items = match selected {
            Some(AcceptHistoryItem::Command { .. }) => {
                vec![enter, MessageItem::text(" to execute")]
            }
            Some(AcceptHistoryItem::AIPrompt { .. }) => {
                vec![enter, MessageItem::text(" to send")]
            }
            None => {
                vec![MessageItem::text("")]
            }
        };
        Some(Message::new(items))
    }
}
