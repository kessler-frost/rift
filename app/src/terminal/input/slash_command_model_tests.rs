// These tests exercised the slash-command model through the AI-input wiring on
// `Input` (`Input::slash_command_data_source`, `Input::slash_command_model`,
// `Input::submit_queued_prompt`, `Input::set_input_mode_natural_language_detection`),
// all of which were removed along with the AI input subsystem. The
// `SlashCommandModel` type itself survives but is no longer owned by `Input`, so
// these integration tests no longer have anything to bind against and were deleted.
