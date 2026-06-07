use std::collections::HashMap;
use std::sync::Arc;

use riftui::App;

use super::super::proto::{
    server_message, write_file_response, Authenticate, Initialize, ServerMessage,
    WriteFileResponse, WriteFileSuccess,
};
use super::super::protocol::RequestId;
use super::super::server_buffer_tracker::ServerBufferTracker;
use super::{ConnectionId, PendingFileOps, ServerModel};
use crate::auth::auth_state::AuthState;

fn test_model(_app: &mut App) -> ServerModel {
    ServerModel {
        connection_senders: HashMap::new(),
        snapshot_sent_roots_by_connection: HashMap::new(),
        grace_timer_cancel: None,
        in_progress: HashMap::new(),
        host_id: "test-host-id".to_string(),
        executors: HashMap::new(),
        pending_file_ops: PendingFileOps::new(),
        auth_state: Arc::new(AuthState::new_logged_out_for_test()),
        buffers: ServerBufferTracker::new(),
        host_scoped_requests: HashMap::new(),
    }
}

#[test]
fn fresh_model_starts_without_auth_token() {
    App::test((), |mut app| async move {
        let model = test_model(&mut app);

        assert_eq!(model.auth_token().as_deref(), None);
        assert_eq!(model.auth_state.user_id(), None);
        assert_eq!(model.auth_state.user_email(), None);
    });
}

#[test]
fn initialize_with_auth_token_stores_token() {
    App::test((), |mut app| async move {
        let mut model = test_model(&mut app);

        model.apply_initialize_auth(&Initialize {
            auth_token: "initial-token".to_string(),
            user_id: "test-user-id".to_string(),
            user_email: "test@example.com".to_string(),
            crash_reporting_enabled: true,
            codebase_index_limits: None,
        });

        assert_eq!(model.auth_token().as_deref(), Some("initial-token"));
        assert_eq!(
            model.auth_state.user_id().unwrap().as_string(),
            "test-user-id"
        );
        assert_eq!(
            model.auth_state.user_email().as_deref(),
            Some("test@example.com")
        );
    });
}

#[test]
fn empty_initialize_clears_auth_context() {
    App::test((), |mut app| async move {
        let mut model = test_model(&mut app);
        model.apply_initialize_auth(&Initialize {
            auth_token: "initial-token".to_string(),
            user_id: "test-user-id".to_string(),
            user_email: "test@example.com".to_string(),
            crash_reporting_enabled: true,
            codebase_index_limits: None,
        });

        model.apply_initialize_auth(&Initialize {
            auth_token: String::new(),
            user_id: String::new(),
            user_email: String::new(),
            crash_reporting_enabled: true,
            codebase_index_limits: None,
        });

        assert_eq!(model.auth_token().as_deref(), None);
        assert_eq!(model.auth_state.user_id(), None);
        assert_eq!(model.auth_state.user_email(), None);
    });
}

#[test]
fn authenticate_with_auth_token_replaces_auth_token() {
    App::test((), |mut app| async move {
        let mut model = test_model(&mut app);
        model.apply_initialize_auth(&Initialize {
            auth_token: "initial-token".to_string(),
            user_id: String::new(),
            user_email: String::new(),
            crash_reporting_enabled: true,
            codebase_index_limits: None,
        });

        model.handle_authenticate(Authenticate {
            auth_token: "rotated-token".to_string(),
        });

        assert_eq!(model.auth_token().as_deref(), Some("rotated-token"));
    });
}

#[test]
fn empty_authenticate_clears_auth_token() {
    App::test((), |mut app| async move {
        let mut model = test_model(&mut app);
        model.apply_initialize_auth(&Initialize {
            auth_token: "initial-token".to_string(),
            user_id: String::new(),
            user_email: String::new(),
            crash_reporting_enabled: true,
            codebase_index_limits: None,
        });

        model.handle_authenticate(Authenticate {
            auth_token: String::new(),
        });

        assert_eq!(model.auth_token().as_deref(), None);
    });
}

// ── Daemon host-scoped response failover ────────────────────────────

/// A throwaway host-scoped response payload used to assert routing.
fn write_file_success_message() -> server_message::Message {
    server_message::Message::WriteFileResponse(WriteFileResponse {
        result: Some(write_file_response::Result::Success(WriteFileSuccess {})),
    })
}

#[test]
fn host_scoped_response_fails_over_when_target_send_fails() {
    App::test((), |mut app| async move {
        let mut model = test_model(&mut app);
        let request_id = RequestId::new();
        let target: ConnectionId = uuid::Uuid::new_v4();
        let alternate: ConnectionId = uuid::Uuid::new_v4();

        // The target connection's receiver is dropped, so its sender still
        // exists in the map but `try_send` fails (channel closed).
        let (target_tx, target_rx) = async_channel::bounded(1);
        drop(target_rx);
        model.connection_senders.insert(target, target_tx);

        // The alternate connection has a live receiver.
        let (alt_tx, alt_rx) = async_channel::unbounded();
        model.connection_senders.insert(alternate, alt_tx);

        // Mark the request as host-scoped so failover is eligible.
        model
            .host_scoped_requests
            .insert(request_id.clone(), target);

        model.send_server_message(
            Some(target),
            Some(&request_id),
            write_file_success_message(),
        );

        // The response was re-routed to the alternate connection.
        let received = alt_rx
            .try_recv()
            .expect("alternate should receive failover response");
        assert_eq!(received.request_id, request_id.to_string());
        // The host-scoped entry is consumed regardless of delivery path.
        assert!(!model.host_scoped_requests.contains_key(&request_id));
    });
}

#[test]
fn host_scoped_response_fails_over_when_target_missing() {
    App::test((), |mut app| async move {
        let mut model = test_model(&mut app);
        let request_id = RequestId::new();
        let target: ConnectionId = uuid::Uuid::new_v4();
        let alternate: ConnectionId = uuid::Uuid::new_v4();

        // Target connection is gone entirely (not in the senders map), but the
        // request is still tracked as host-scoped.
        let (alt_tx, alt_rx) = async_channel::unbounded();
        model.connection_senders.insert(alternate, alt_tx);
        model
            .host_scoped_requests
            .insert(request_id.clone(), target);

        model.send_server_message(
            Some(target),
            Some(&request_id),
            write_file_success_message(),
        );

        let received = alt_rx
            .try_recv()
            .expect("alternate should receive failover response");
        assert_eq!(received.request_id, request_id.to_string());
        assert!(!model.host_scoped_requests.contains_key(&request_id));
    });
}

#[test]
fn non_host_scoped_response_is_not_failed_over() {
    App::test((), |mut app| async move {
        let mut model = test_model(&mut app);
        let request_id = RequestId::new();
        let target: ConnectionId = uuid::Uuid::new_v4();
        let alternate: ConnectionId = uuid::Uuid::new_v4();

        // Target sender exists but is closed; the request is NOT tracked as
        // host-scoped, so the message must be dropped rather than re-routed.
        let (target_tx, target_rx) = async_channel::bounded(1);
        drop(target_rx);
        model.connection_senders.insert(target, target_tx);
        let (alt_tx, alt_rx) = async_channel::unbounded::<ServerMessage>();
        model.connection_senders.insert(alternate, alt_tx);

        model.send_server_message(
            Some(target),
            Some(&request_id),
            write_file_success_message(),
        );

        assert!(
            alt_rx.try_recv().is_err(),
            "non-host-scoped response must not fail over to another connection"
        );
    });
}
