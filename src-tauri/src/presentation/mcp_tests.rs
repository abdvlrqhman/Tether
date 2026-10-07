use super::mcp::TetherMcp;
use crate::{
    application::agent::AgentService,
    infrastructure::{
        remote::NativeRemoteTransport, session_store::PrivateSessionStore, SystemClock,
    },
};
use rmcp::{model::CallToolRequestParams, ServiceExt};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
#[tokio::test]
async fn mcp_protocol_pair_command_and_logout() {
    let (host, url, http) = crate::infrastructure::integration_tests::fixture().await;
    let temp = tempfile::tempdir().unwrap();
    let invitation = temp.path().join("invitation.json");
    std::fs::write(
        &invitation,
        json!({"url":url,"invite":host.snapshot().invite.unwrap()}).to_string(),
    )
    .unwrap();
    let agent = AgentService::new(
        Arc::new(NativeRemoteTransport::new()),
        Arc::new(PrivateSessionStore::new(temp.path().join("session.json"))),
        Arc::new(SystemClock),
    );
    let server = TetherMcp::for_test(agent);
    let (client_io, server_io) = tokio::io::duplex(65536);
    let server_task = tokio::spawn(async move {
        server
            .serve(server_io)
            .await
            .unwrap()
            .waiting()
            .await
            .unwrap();
    });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 7);
    assert!(tools
        .iter()
        .all(|t| t
            .annotations
            .as_ref()
            .is_some_and(|a| a.read_only_hint.is_some()
                && a.destructive_hint.is_some()
                && a.idempotent_hint.is_some()
                && a.open_world_hint.is_some())));
    assert!(tools.iter().any(|t| t.name == "tether_exec"
        && t.annotations.as_ref().and_then(|a| a.destructive_hint) == Some(true)));
    let before = client
        .call_tool(CallToolRequestParams::new("tether_session_status"))
        .await
        .unwrap();
    assert_eq!(before.is_error, Some(true));
    let request = client
        .call_tool(
            CallToolRequestParams::new("tether_pair").with_arguments(
                json!({"invitation_file":invitation,"operator":"MCP integration test"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_ne!(request.is_error, Some(true));
    let receipt = request.structured_content.unwrap();
    let id = receipt["request_id"].as_str().unwrap();
    assert!(!receipt.to_string().contains("proof"));
    host.approve(id, 15).unwrap();
    let approved = client
        .call_tool(CallToolRequestParams::new("tether_pair_status"))
        .await
        .unwrap();
    let approved = approved.structured_content.unwrap();
    assert_eq!(approved["status"], "approved");
    assert!(approved.get("token").is_none());
    let job = client
        .call_tool(
            CallToolRequestParams::new("tether_exec").with_arguments(
                json!({"command":"echo mcp-shell-ok; exit 7","timeout_seconds":10})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_ne!(job.is_error, Some(true));
    let job_id = job.structured_content.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let result: Value = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let result = client
                .call_tool(
                    CallToolRequestParams::new("tether_job_status")
                        .with_arguments(json!({"job_id":job_id}).as_object().unwrap().clone()),
                )
                .await
                .unwrap();
            assert_ne!(result.is_error, Some(true));
            let value = result.structured_content.unwrap();
            if value["status"] == "finished" {
                break value;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(result["result"]["exit_code"], 7);
    assert!(result["result"]["stdout"]
        .as_str()
        .unwrap()
        .contains("mcp-shell-ok"));
    #[cfg(windows)]
    let slow = "Start-Sleep -Seconds 20";
    #[cfg(unix)]
    let slow = "sleep 20";
    let job = client
        .call_tool(
            CallToolRequestParams::new("tether_exec").with_arguments(
                json!({"command":slow,"timeout_seconds":30})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap()
        .structured_content
        .unwrap();
    let id = job["id"].as_str().unwrap();
    let cancelled = client
        .call_tool(
            CallToolRequestParams::new("tether_cancel_job")
                .with_arguments(json!({"job_id":id}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_ne!(cancelled.is_error, Some(true));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let job = client
                .call_tool(
                    CallToolRequestParams::new("tether_job_status")
                        .with_arguments(json!({"job_id":id}).as_object().unwrap().clone()),
                )
                .await
                .unwrap()
                .structured_content
                .unwrap();
            if job["status"] == "finished" {
                assert_eq!(job["result"]["cancelled"], true);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let logout = client
        .call_tool(CallToolRequestParams::new("tether_logout"))
        .await
        .unwrap();
    assert_ne!(logout.is_error, Some(true));
    assert!(!temp.path().join("session.json").exists());
    assert!(host.snapshot().session.is_none());
    client.cancel().await.unwrap();
    server_task.await.unwrap();
    host.stop();
    http.abort();
}
