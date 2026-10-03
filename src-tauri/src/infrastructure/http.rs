//! HTTP/WebSocket adapter. Approval and revocation endpoints deliberately exist only in local IPC.
use crate::{
    application::{
        ports::{TerminalInput, TerminalOutput},
        HostService,
    },
    domain::ExecRequest,
};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        DefaultBodyLimit, Path, State, WebSocketUpgrade,
    },
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::SinkExt;
use serde::Deserialize;
use serde_json::json;
use std::{sync::atomic::Ordering, time::Duration};
type ApiResult<T> = Result<Json<T>, ApiError>;
pub struct ApiError(StatusCode, String);
impl From<String> for ApiError {
    fn from(s: String) -> Self {
        Self(StatusCode::BAD_REQUEST, s)
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
fn bearer(headers: &HeaderMap) -> Result<&str, ApiError> {
    headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .filter(|s| s.len() <= 128)
        .ok_or_else(|| {
            ApiError(
                StatusCode::UNAUTHORIZED,
                "Bearer authorization required".into(),
            )
        })
}
fn authorize(service: &HostService, headers: &HeaderMap) -> Result<(), ApiError> {
    service
        .authenticate(bearer(headers)?)
        .map(|_| ())
        .map_err(|e| ApiError(StatusCode::UNAUTHORIZED, e))
}
async fn browser_guard(request: axum::extract::Request, next: Next) -> Response {
    if request.headers().contains_key("origin") {
        return ApiError(
            StatusCode::FORBIDDEN,
            "Browser requests are not supported; use Tether or the CLI".into(),
        )
        .into_response();
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    response
}
pub fn router(service: HostService) -> Router {
    Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"service":"tether","version":"0.1.0"})) }),
        )
        .route("/v1/pair", post(pair))
        .route("/v1/pair/{id}", get(pair_status))
        .route("/v1/session", get(session).delete(release))
        .route("/v1/exec", post(exec))
        .route("/v1/jobs/{id}", get(job).delete(cancel_job))
        .route("/v1/terminal", get(terminal))
        .layer(DefaultBodyLimit::max(32768))
        .layer(middleware::from_fn(browser_guard))
        .with_state(service)
}
#[derive(Deserialize)]
struct PairRequest {
    invite: String,
    operator: String,
}
async fn pair(
    State(service): State<HostService>,
    Json(request): Json<PairRequest>,
) -> ApiResult<crate::application::PairReceipt> {
    Ok(Json(service.pair(&request.invite, &request.operator)?))
}
async fn pair_status(
    State(service): State<HostService>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<crate::application::PairStatus> {
    Ok(Json(
        service
            .pair_status(&id, bearer(&headers)?)
            .map_err(|e| ApiError(StatusCode::UNAUTHORIZED, e))?,
    ))
}
async fn session(
    State(service): State<HostService>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    let s = service
        .authenticate(bearer(&headers)?)
        .map_err(|e| ApiError(StatusCode::UNAUTHORIZED, e))?;
    Ok(Json(
        json!({"session":s,"os":std::env::consts::OS,"working_directory":service.snapshot().working_directory}),
    ))
}
async fn release(
    State(service): State<HostService>,
    headers: HeaderMap,
) -> ApiResult<serde_json::Value> {
    authorize(&service, &headers)?;
    service.release(bearer(&headers)?)?;
    Ok(Json(json!({"status":"revoked"})))
}
async fn exec(
    State(service): State<HostService>,
    headers: HeaderMap,
    Json(request): Json<ExecRequest>,
) -> ApiResult<crate::domain::JobView> {
    authorize(&service, &headers)?;
    Ok(Json(service.submit(bearer(&headers)?, request)?))
}
async fn job(
    State(service): State<HostService>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<crate::domain::JobView> {
    authorize(&service, &headers)?;
    Ok(Json(service.job(bearer(&headers)?, &id, false)?))
}
async fn cancel_job(
    State(service): State<HostService>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<crate::domain::JobView> {
    authorize(&service, &headers)?;
    Ok(Json(service.job(bearer(&headers)?, &id, true)?))
}
async fn terminal(
    State(service): State<HostService>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    authorize(&service, &headers)?;
    let lease = service.terminal(bearer(&headers)?)?;
    Ok(ws
        .max_message_size(32768)
        .max_frame_size(32768)
        .on_upgrade(move |socket| terminal_socket(service, lease, socket)))
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientFrame {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
}
async fn terminal_socket(
    service: HostService,
    mut lease: crate::application::TerminalLease,
    mut socket: WebSocket,
) {
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let mut ping = tokio::time::interval(Duration::from_secs(20));
    let mut last_seen = tokio::time::Instant::now();
    loop {
        let frame = tokio::select! {
            _=tick.tick()=>{if !service.active(&lease.session.id)||lease.connection.cancel.load(Ordering::SeqCst)||last_seen.elapsed()>Duration::from_secs(60){break;}continue;},
            _=ping.tick()=>Message::Ping(Vec::new().into()),
            data=lease.connection.output.recv()=>match data{Some(TerminalOutput::Data(data))=>Message::Text(json!({"type":"output","data":STANDARD.encode(data)}).to_string().into()),Some(TerminalOutput::Error(error))=>Message::Text(json!({"type":"error","message":error}).to_string().into()),_=>break},
            data=socket.recv()=>{last_seen=tokio::time::Instant::now();match data{Some(Ok(Message::Text(text)))=>{
                let input=match serde_json::from_str::<ClientFrame>(&text){Ok(ClientFrame::Input{data})=>match STANDARD.decode(data){Ok(data)if data.len()<=16384=>TerminalInput::Write(data),_=>break},Ok(ClientFrame::Resize{cols,rows})=>TerminalInput::Resize(cols,rows),Err(_)=>break};
                if lease.connection.input.try_send(input).is_err(){break;}continue;
            },Some(Ok(Message::Ping(data)))=>Message::Pong(data),Some(Ok(Message::Pong(_)))=>continue,_=>break}}
        };
        if !service.active(&lease.session.id) {
            break;
        }
        if !matches!(
            tokio::time::timeout(Duration::from_secs(3), socket.send(frame)).await,
            Ok(Ok(()))
        ) {
            break;
        }
    }
    lease.connection.cancel.store(true, Ordering::SeqCst);
    let _ = tokio::time::timeout(Duration::from_secs(1), socket.close()).await;
}
