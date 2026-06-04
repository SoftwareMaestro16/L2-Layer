use super::{ApiError, AppState};
use crate::observer::{ObserverReplayConfig, ObserverReplayRequest, ObserverReplayService};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;

pub(super) async fn operator_observer_checkpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Option<crate::storage::ObserverCheckpoint>>, ApiError> {
    state.admin_auth.authorize(&headers)?;
    let service = observer_service(&state).await;
    Ok(Json(service.latest_checkpoint().await?))
}

pub(super) async fn operator_observer_replay(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ObserverReplayRequest>,
) -> Result<Json<crate::observer::ObserverReplayReport>, ApiError> {
    state.admin_auth.authorize(&headers)?;
    let service = observer_service(&state).await;
    Ok(Json(service.replay(request).await?))
}

async fn observer_service(state: &AppState) -> ObserverReplayService {
    let config = {
        let sequencer = state.sequencer.read().await;
        ObserverReplayConfig::from_sequencer_config(&sequencer.config)
    };
    ObserverReplayService::new(state.storage.clone(), state.da.clone(), config)
}
