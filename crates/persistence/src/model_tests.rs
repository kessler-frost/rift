use super::AgentConversationData;

#[test]
fn agent_conversation_data_roundtrips_last_event_sequence() {
    let data = AgentConversationData {
        server_conversation_token: None,
        conversation_usage_metadata: None,
        reverted_action_ids: None,
        forked_from_server_conversation_token: None,
        artifacts_json: None,
        parent_agent_id: None,
        agent_name: None,
        orchestration_harness_type: Some("claude".to_string()),
        parent_conversation_id: None,
        is_remote_child: false,
        root_task_is_optimistic: None,
        run_id: None,
        autoexecute_override: None,
        last_event_sequence: Some(42),
        pinned: false,
    };
    let json = serde_json::to_string(&data).expect("serialize");
    let roundtripped: AgentConversationData = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(roundtripped.last_event_sequence, Some(42));
    assert_eq!(
        roundtripped.orchestration_harness_type.as_deref(),
        Some("claude")
    );
}

#[test]
fn agent_conversation_data_accepts_legacy_orchestration_avatar_id() {
    let legacy_json = r#"{"orchestration_avatar_id":"orbit"}"#;
    let data: AgentConversationData =
        serde_json::from_str(legacy_json).expect("legacy rows must deserialize");

    assert_eq!(data.orchestration_harness_type.as_deref(), Some("orbit"));
}

#[test]
fn agent_conversation_data_roundtrips_remote_child_marker() {
    let data = AgentConversationData {
        server_conversation_token: None,
        conversation_usage_metadata: None,
        reverted_action_ids: None,
        forked_from_server_conversation_token: None,
        artifacts_json: None,
        parent_agent_id: None,
        agent_name: None,
        orchestration_harness_type: None,
        parent_conversation_id: None,
        is_remote_child: true,
        root_task_is_optimistic: None,
        run_id: None,
        autoexecute_override: None,
        last_event_sequence: None,
        pinned: false,
    };
    let json = serde_json::to_string(&data).expect("serialize");
    let roundtripped: AgentConversationData = serde_json::from_str(&json).expect("deserialize");
    assert!(roundtripped.is_remote_child);
}

#[test]
fn agent_conversation_data_roundtrips_optimistic_root_marker() {
    let data = AgentConversationData {
        server_conversation_token: None,
        conversation_usage_metadata: None,
        reverted_action_ids: None,
        forked_from_server_conversation_token: None,
        artifacts_json: None,
        parent_agent_id: None,
        agent_name: None,
        orchestration_harness_type: None,
        parent_conversation_id: None,
        is_remote_child: false,
        root_task_is_optimistic: Some(true),
        run_id: None,
        autoexecute_override: None,
        last_event_sequence: None,
        pinned: false,
    };
    let json = serde_json::to_string(&data).expect("serialize");
    let roundtripped: AgentConversationData = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(roundtripped.root_task_is_optimistic, Some(true));
}

#[test]
fn agent_conversation_data_deserializes_legacy_payload_without_last_event_sequence() {
    // Legacy rows persisted before this feature landed omit the field
    // entirely. `#[serde(default)]` must accept them as `None`.
    let legacy_json = r#"{"server_conversation_token":null}"#;
    let data: AgentConversationData =
        serde_json::from_str(legacy_json).expect("legacy rows must deserialize");
    assert_eq!(data.last_event_sequence, None);
    assert_eq!(data.orchestration_harness_type, None);
    assert!(!data.is_remote_child);
}

#[test]
fn agent_conversation_data_skips_serializing_none_last_event_sequence() {
    let data = AgentConversationData {
        server_conversation_token: None,
        conversation_usage_metadata: None,
        reverted_action_ids: None,
        forked_from_server_conversation_token: None,
        artifacts_json: None,
        parent_agent_id: None,
        agent_name: None,
        orchestration_harness_type: None,
        parent_conversation_id: None,
        is_remote_child: false,
        root_task_is_optimistic: None,
        run_id: None,
        autoexecute_override: None,
        last_event_sequence: None,
        pinned: false,
    };
    let json = serde_json::to_string(&data).expect("serialize");
    assert!(
        !json.contains("last_event_sequence"),
        "None should be skipped in serialized output: {json}"
    );
}

#[test]
fn agent_conversation_data_roundtrips_pinned() {
    let data = AgentConversationData {
        server_conversation_token: None,
        conversation_usage_metadata: None,
        reverted_action_ids: None,
        forked_from_server_conversation_token: None,
        artifacts_json: None,
        parent_agent_id: None,
        agent_name: None,
        orchestration_harness_type: None,
        parent_conversation_id: None,
        is_remote_child: false,
        root_task_is_optimistic: None,
        run_id: None,
        autoexecute_override: None,
        last_event_sequence: None,
        pinned: true,
    };
    let json = serde_json::to_string(&data).expect("serialize");
    let roundtripped: AgentConversationData = serde_json::from_str(&json).expect("deserialize");
    assert!(roundtripped.pinned);
}

#[test]
fn agent_conversation_data_skips_serializing_unpinned() {
    let data = AgentConversationData {
        server_conversation_token: None,
        conversation_usage_metadata: None,
        reverted_action_ids: None,
        forked_from_server_conversation_token: None,
        artifacts_json: None,
        parent_agent_id: None,
        agent_name: None,
        orchestration_harness_type: None,
        parent_conversation_id: None,
        is_remote_child: false,
        root_task_is_optimistic: None,
        run_id: None,
        autoexecute_override: None,
        last_event_sequence: None,
        pinned: false,
    };
    let json = serde_json::to_string(&data).expect("serialize");
    assert!(
        !json.contains("pinned"),
        "Unpinned default should be skipped: {json}"
    );
}

#[test]
fn agent_conversation_data_legacy_rows_default_to_unpinned() {
    let legacy_json = r#"{"server_conversation_token":null}"#;
    let data: AgentConversationData =
        serde_json::from_str(legacy_json).expect("legacy rows must deserialize");
    assert!(!data.pinned);
}
