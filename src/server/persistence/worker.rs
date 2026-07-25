use sqlx::{migrate::Migrator, mysql::MySqlPoolOptions};
use std::{fmt, sync::Mutex, thread, time::Duration};
use tokio::sync::mpsc;

use super::{
    models::{AccountId, CharacterRecord, CharacterSnapshot, CharacterSummary, NewCharacter},
    repository::CharacterRepository,
};
use crate::shared::gameplay::components::CharacterId;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub type PersistenceRequestId = u64;

#[derive(Debug)]
pub enum PersistenceRequest {
    ListCharacters {
        request_id: PersistenceRequestId,
        account_id: AccountId,
    },
    CreateCharacter {
        request_id: PersistenceRequestId,
        character: NewCharacter,
    },
    LoadOrCreateDefaultCharacter {
        request_id: PersistenceRequestId,
        account_id: AccountId,
    },
    LoadCharacter {
        request_id: PersistenceRequestId,
        account_id: AccountId,
        character_id: CharacterId,
    },
    SaveCharacter {
        request_id: PersistenceRequestId,
        snapshot: CharacterSnapshot,
    },
    Shutdown,
}

#[derive(Debug)]
pub enum PersistenceResponse {
    DatabaseReady,
    CharactersListed {
        request_id: PersistenceRequestId,
        characters: Vec<CharacterSummary>,
    },
    CharacterCreated {
        request_id: PersistenceRequestId,
        character: CharacterRecord,
    },
    CharacterLoaded {
        request_id: PersistenceRequestId,
        character: Option<CharacterRecord>,
    },
    CharacterSaved {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        revision: u64,
    },
    RequestFailed {
        request_id: Option<PersistenceRequestId>,
        operation: &'static str,
        message: String,
    },
    WorkerStopped,
}

#[derive(bevy::prelude::Resource)]
pub struct PersistenceClient {
    requests: mpsc::UnboundedSender<PersistenceRequest>,
}

impl PersistenceClient {
    pub fn send(&self, request: PersistenceRequest) -> Result<(), PersistenceUnavailable> {
        self.requests
            .send(request)
            .map_err(|_| PersistenceUnavailable)
    }

    #[cfg(test)]
    pub(crate) fn test_channel() -> (Self, mpsc::UnboundedReceiver<PersistenceRequest>) {
        let (requests, receiver) = mpsc::unbounded_channel();
        (Self { requests }, receiver)
    }
}

#[derive(Debug)]
pub struct PersistenceUnavailable;

impl fmt::Display for PersistenceUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the persistence worker is not running")
    }
}

impl std::error::Error for PersistenceUnavailable {}

#[derive(bevy::prelude::Resource)]
pub(super) struct PersistenceResponses(
    pub(super) Mutex<mpsc::UnboundedReceiver<PersistenceResponse>>,
);

pub(super) fn start(database_url: String) -> (PersistenceClient, PersistenceResponses) {
    let (request_sender, request_receiver) = mpsc::unbounded_channel();
    let (response_sender, response_receiver) = mpsc::unbounded_channel();

    thread::Builder::new()
        .name("mysql-persistence".into())
        .spawn(move || run_worker(database_url, request_receiver, response_sender))
        .expect("persistence thread should be created");

    (
        PersistenceClient {
            requests: request_sender,
        },
        PersistenceResponses(Mutex::new(response_receiver)),
    )
}

fn run_worker(
    database_url: String,
    request_receiver: mpsc::UnboundedReceiver<PersistenceRequest>,
    response_sender: mpsc::UnboundedSender<PersistenceResponse>,
) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = response_sender.send(PersistenceResponse::RequestFailed {
                request_id: None,
                operation: "start database runtime",
                message: error.to_string(),
            });
            return;
        }
    };

    runtime.block_on(run_database_loop(
        database_url,
        request_receiver,
        response_sender,
    ));
}

async fn run_database_loop(
    database_url: String,
    mut requests: mpsc::UnboundedReceiver<PersistenceRequest>,
    responses: mpsc::UnboundedSender<PersistenceResponse>,
) {
    let pool = match MySqlPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await
    {
        Ok(pool) => pool,
        Err(error) => {
            let _ = responses.send(PersistenceResponse::RequestFailed {
                request_id: None,
                operation: "connect to MySQL",
                message: error.to_string(),
            });
            return;
        }
    };

    if let Err(error) = MIGRATOR.run(&pool).await {
        let _ = responses.send(PersistenceResponse::RequestFailed {
            request_id: None,
            operation: "run database migrations",
            message: error.to_string(),
        });
        return;
    }

    let repository = CharacterRepository::new(pool.clone());
    if responses.send(PersistenceResponse::DatabaseReady).is_err() {
        return;
    }

    while let Some(request) = requests.recv().await {
        let response = match request {
            PersistenceRequest::ListCharacters {
                request_id,
                account_id,
            } => match repository.list_characters(account_id).await {
                Ok(characters) => PersistenceResponse::CharactersListed {
                    request_id,
                    characters,
                },
                Err(error) => request_failed(request_id, "list characters", error),
            },
            PersistenceRequest::CreateCharacter {
                request_id,
                character,
            } => match repository.create_character(character).await {
                Ok(character) => PersistenceResponse::CharacterCreated {
                    request_id,
                    character,
                },
                Err(error) => request_failed(request_id, "create character", error),
            },
            PersistenceRequest::LoadOrCreateDefaultCharacter {
                request_id,
                account_id,
            } => match repository
                .load_or_create_default_character(account_id)
                .await
            {
                Ok(character) => PersistenceResponse::CharacterLoaded {
                    request_id,
                    character: Some(character),
                },
                Err(error) => request_failed(request_id, "load or create default character", error),
            },
            PersistenceRequest::LoadCharacter {
                request_id,
                account_id,
                character_id,
            } => match repository.load_character(account_id, character_id).await {
                Ok(character) => PersistenceResponse::CharacterLoaded {
                    request_id,
                    character,
                },
                Err(error) => request_failed(request_id, "load character", error),
            },
            PersistenceRequest::SaveCharacter {
                request_id,
                snapshot,
            } => {
                let character_id = snapshot.character_id;
                match repository.save_character(snapshot).await {
                    Ok(revision) => PersistenceResponse::CharacterSaved {
                        request_id,
                        character_id,
                        revision,
                    },
                    Err(error) => request_failed(request_id, "save character", error),
                }
            }
            PersistenceRequest::Shutdown => break,
        };

        if responses.send(response).is_err() {
            break;
        }
    }

    pool.close().await;
    let _ = responses.send(PersistenceResponse::WorkerStopped);
}

fn request_failed(
    request_id: PersistenceRequestId,
    operation: &'static str,
    error: impl fmt::Display,
) -> PersistenceResponse {
    PersistenceResponse::RequestFailed {
        request_id: Some(request_id),
        operation,
        message: error.to_string(),
    }
}
