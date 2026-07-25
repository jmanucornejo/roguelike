use bevy::prelude::*;
use std::{collections::VecDeque, env};
use tokio::sync::mpsc::error::TryRecvError;

use super::worker::{self, PersistenceResponse, PersistenceResponses};

#[derive(Resource, Clone, Debug, Eq, PartialEq)]
pub enum PersistenceStatus {
    Disabled,
    Connecting,
    Ready,
    Failed(String),
}

#[derive(Resource, Default)]
pub struct PersistenceInbox(pub VecDeque<PersistenceResponse>);

pub struct PersistencePlugin;

impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PersistenceInbox>();
        let _ = dotenvy::dotenv();

        let Some(database_url) = configured_database_url(env::var("DATABASE_URL").ok()) else {
            warn!(
                "DATABASE_URL is not set; MySQL persistence is disabled and players remain in-memory"
            );
            app.insert_resource(PersistenceStatus::Disabled);
            return;
        };

        let (client, responses) = worker::start(database_url);
        app.insert_resource(client)
            .insert_resource(responses)
            .insert_resource(PersistenceStatus::Connecting)
            .add_systems(Update, poll_persistence_responses);
    }
}

fn configured_database_url(value: Option<String>) -> Option<String> {
    value.filter(|url| !url.trim().is_empty())
}

fn poll_persistence_responses(
    responses: Res<PersistenceResponses>,
    mut status: ResMut<PersistenceStatus>,
    mut inbox: ResMut<PersistenceInbox>,
) {
    let Ok(mut receiver) = responses.0.lock() else {
        *status = PersistenceStatus::Failed("database response channel was poisoned".into());
        return;
    };

    loop {
        match receiver.try_recv() {
            Ok(PersistenceResponse::DatabaseReady) => {
                info!("MySQL persistence is ready");
                *status = PersistenceStatus::Ready;
            }
            Ok(PersistenceResponse::RequestFailed {
                request_id: None,
                operation,
                message,
            }) => {
                error!("Persistence failed to {operation}: {message}");
                *status = PersistenceStatus::Failed(format!("{operation}: {message}"));
                inbox.0.push_back(PersistenceResponse::RequestFailed {
                    request_id: None,
                    operation,
                    message,
                });
            }
            Ok(PersistenceResponse::WorkerStopped) => {
                if !matches!(*status, PersistenceStatus::Failed(_)) {
                    warn!("MySQL persistence worker stopped");
                    *status = PersistenceStatus::Failed("database worker stopped".into());
                }
            }
            Ok(response) => inbox.0.push_back(response),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                if matches!(
                    *status,
                    PersistenceStatus::Connecting | PersistenceStatus::Ready
                ) {
                    *status =
                        PersistenceStatus::Failed("database response channel disconnected".into());
                }
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_empty_database_url_disables_persistence() {
        assert_eq!(configured_database_url(None), None);
        assert_eq!(configured_database_url(Some("  ".into())), None);
    }

    #[test]
    fn configured_database_url_is_preserved() {
        let url = "mysql://user:password@localhost/roguelike".to_string();
        assert_eq!(configured_database_url(Some(url.clone())), Some(url));
    }
}
