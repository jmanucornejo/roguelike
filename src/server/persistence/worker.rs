use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};
use sqlx::{migrate::Migrator, mysql::MySqlPoolOptions};
use std::{fmt, sync::Mutex, thread, time::Duration};
use tokio::sync::mpsc;

use super::{
    models::{AccountId, CharacterRecord, CharacterSnapshot, CharacterSummary, NewCharacter},
    repository::CharacterRepository,
};
use crate::shared::gameplay::{
    action_bar::{ActionBarBinding, ActionBarLayout},
    components::{CharacterId, Equipment, EquipmentSlot},
    items::{Inventory, ItemDefinitionId},
    skills::{LearnedSkill, SkillId},
};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub type PersistenceRequestId = u64;

#[derive(Debug)]
pub enum PersistenceRequest {
    AuthenticateAccount {
        request_id: PersistenceRequestId,
        username: String,
        password: String,
    },
    CreateAccount {
        request_id: PersistenceRequestId,
        username: String,
        password: String,
    },
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
    AddInventoryItem {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        item_id: ItemDefinitionId,
        quantity: u32,
    },
    RemoveInventoryItem {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        item_id: ItemDefinitionId,
        quantity: u32,
    },
    EquipInventoryItem {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        item_id: ItemDefinitionId,
        slot: EquipmentSlot,
    },
    UnequipInventoryItem {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        slot: EquipmentSlot,
    },
    SaveActionBarSlot {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        slot_index: u8,
        binding: Option<ActionBarBinding>,
    },
    SaveActionBarSwap {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        first_slot: u8,
        first_binding: Option<ActionBarBinding>,
        second_slot: u8,
        second_binding: Option<ActionBarBinding>,
    },
    SaveSkillRank {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        skill_id: SkillId,
        rank: u8,
        available_points: u32,
    },
    SaveSkillPoints {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        available_points: u32,
    },
    ClearSkills {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
    },
    Shutdown,
}

#[derive(Debug)]
pub enum PersistenceResponse {
    DatabaseReady,
    AccountAuthenticated {
        request_id: PersistenceRequestId,
        account_id: Option<AccountId>,
    },
    AccountCreated {
        request_id: PersistenceRequestId,
        account_id: AccountId,
    },
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
        inventory: Inventory,
        equipment: Equipment,
        action_bar: ActionBarLayout,
        learned_skills: Vec<LearnedSkill>,
    },
    CharacterSaved {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        revision: u64,
    },
    InventoryItemAdded {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        item_id: ItemDefinitionId,
        quantity: u32,
    },
    InventoryItemRemoved {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        item_id: ItemDefinitionId,
        quantity: u32,
    },
    InventoryItemEquipped {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        item_id: ItemDefinitionId,
        slot: EquipmentSlot,
    },
    InventoryItemUnequipped {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        item_id: ItemDefinitionId,
        slot: EquipmentSlot,
    },
    ActionBarSlotSaved {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        slot_index: u8,
    },
    ActionBarSwapSaved {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        first_slot: u8,
        second_slot: u8,
    },
    SkillRankSaved {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        skill_id: SkillId,
        rank: u8,
    },
    SkillPointsSaved {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
        available_points: u32,
    },
    SkillsCleared {
        request_id: PersistenceRequestId,
        character_id: CharacterId,
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
            PersistenceRequest::AuthenticateAccount {
                request_id,
                username,
                password,
            } => match validate_account_credentials(&username, &password) {
                Err(message) => PersistenceResponse::RequestFailed {
                    request_id: Some(request_id),
                    operation: "authenticate account",
                    message,
                },
                Ok(()) => match repository.account_credentials(username.trim()).await {
                    Ok(Some((account_id, password_hash))) => {
                        let authenticated =
                            PasswordHash::new(&password_hash)
                                .ok()
                                .is_some_and(|parsed_hash| {
                                    Argon2::default()
                                        .verify_password(password.as_bytes(), &parsed_hash)
                                        .is_ok()
                                });
                        PersistenceResponse::AccountAuthenticated {
                            request_id,
                            account_id: authenticated.then_some(AccountId(account_id)),
                        }
                    }
                    Ok(None) => PersistenceResponse::AccountAuthenticated {
                        request_id,
                        account_id: None,
                    },
                    Err(error) => request_failed(request_id, "authenticate account", error),
                },
            },
            PersistenceRequest::CreateAccount {
                request_id,
                username,
                password,
            } => match validate_account_credentials(&username, &password) {
                Err(message) => PersistenceResponse::RequestFailed {
                    request_id: Some(request_id),
                    operation: "create account",
                    message,
                },
                Ok(()) => {
                    let salt = SaltString::generate(&mut OsRng);
                    match Argon2::default().hash_password(password.as_bytes(), &salt) {
                        Ok(password_hash) => match repository
                            .create_account(username.trim(), &password_hash.to_string())
                            .await
                        {
                            Ok(account_id) => PersistenceResponse::AccountCreated {
                                request_id,
                                account_id,
                            },
                            Err(error) => request_failed(request_id, "create account", error),
                        },
                        Err(error) => PersistenceResponse::RequestFailed {
                            request_id: Some(request_id),
                            operation: "create account",
                            message: format!("could not protect the account password: {error}"),
                        },
                    }
                }
            },
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
                Ok(character) => {
                    let character_id = CharacterId(character.id);
                    match repository.load_inventory(character_id).await {
                        Ok(inventory) => match repository.load_equipment(character_id).await {
                            Ok(equipment) => match repository.load_action_bar(character_id).await {
                                Ok(action_bar) => {
                                    match repository.load_skills(character_id).await {
                                        Ok(learned_skills) => {
                                            PersistenceResponse::CharacterLoaded {
                                                request_id,
                                                character: Some(character),
                                                inventory,
                                                equipment,
                                                action_bar,
                                                learned_skills,
                                            }
                                        }
                                        Err(error) => {
                                            request_failed(request_id, "load skills", error)
                                        }
                                    }
                                }
                                Err(error) => request_failed(request_id, "load action bar", error),
                            },
                            Err(error) => request_failed(request_id, "load equipment", error),
                        },
                        Err(error) => request_failed(request_id, "load inventory", error),
                    }
                }
                Err(error) => request_failed(request_id, "load or create default character", error),
            },
            PersistenceRequest::LoadCharacter {
                request_id,
                account_id,
                character_id,
            } => match repository.load_character(account_id, character_id).await {
                Ok(character) => {
                    let inventory = if character.is_some() {
                        match repository.load_inventory(character_id).await {
                            Ok(inventory) => inventory,
                            Err(error) => {
                                let response = request_failed(request_id, "load inventory", error);
                                if responses.send(response).is_err() {
                                    break;
                                }
                                continue;
                            }
                        }
                    } else {
                        Inventory::default()
                    };
                    let action_bar = if character.is_some() {
                        match repository.load_action_bar(character_id).await {
                            Ok(action_bar) => action_bar,
                            Err(error) => {
                                let response = request_failed(request_id, "load action bar", error);
                                if responses.send(response).is_err() {
                                    break;
                                }
                                continue;
                            }
                        }
                    } else {
                        ActionBarLayout::default()
                    };
                    let equipment = if character.is_some() {
                        match repository.load_equipment(character_id).await {
                            Ok(equipment) => equipment,
                            Err(error) => {
                                let response = request_failed(request_id, "load equipment", error);
                                if responses.send(response).is_err() {
                                    break;
                                }
                                continue;
                            }
                        }
                    } else {
                        Equipment::default()
                    };
                    let learned_skills = if character.is_some() {
                        match repository.load_skills(character_id).await {
                            Ok(learned_skills) => learned_skills,
                            Err(error) => {
                                let response = request_failed(request_id, "load skills", error);
                                if responses.send(response).is_err() {
                                    break;
                                }
                                continue;
                            }
                        }
                    } else {
                        Vec::new()
                    };
                    PersistenceResponse::CharacterLoaded {
                        request_id,
                        character,
                        inventory,
                        equipment,
                        action_bar,
                        learned_skills,
                    }
                }
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
            PersistenceRequest::AddInventoryItem {
                request_id,
                character_id,
                item_id,
                quantity,
            } => match repository
                .add_inventory_item(character_id, item_id, quantity)
                .await
            {
                Ok(quantity) => PersistenceResponse::InventoryItemAdded {
                    request_id,
                    character_id,
                    item_id,
                    quantity,
                },
                Err(error) => request_failed(request_id, "add inventory item", error),
            },
            PersistenceRequest::RemoveInventoryItem {
                request_id,
                character_id,
                item_id,
                quantity,
            } => match repository
                .remove_inventory_item(character_id, item_id, quantity)
                .await
            {
                Ok(quantity) => PersistenceResponse::InventoryItemRemoved {
                    request_id,
                    character_id,
                    item_id,
                    quantity,
                },
                Err(error) => request_failed(request_id, "remove inventory item", error),
            },
            PersistenceRequest::EquipInventoryItem {
                request_id,
                character_id,
                item_id,
                slot,
            } => match repository
                .equip_inventory_item(character_id, item_id, slot)
                .await
            {
                Ok(()) => PersistenceResponse::InventoryItemEquipped {
                    request_id,
                    character_id,
                    item_id,
                    slot,
                },
                Err(error) => request_failed(request_id, "equip inventory item", error),
            },
            PersistenceRequest::UnequipInventoryItem {
                request_id,
                character_id,
                slot,
            } => match repository.unequip_inventory_item(character_id, slot).await {
                Ok(item_id) => PersistenceResponse::InventoryItemUnequipped {
                    request_id,
                    character_id,
                    item_id,
                    slot,
                },
                Err(error) => request_failed(request_id, "unequip inventory item", error),
            },
            PersistenceRequest::SaveActionBarSlot {
                request_id,
                character_id,
                slot_index,
                binding,
            } => match repository
                .save_action_bar_slot(character_id, slot_index, binding)
                .await
            {
                Ok(()) => PersistenceResponse::ActionBarSlotSaved {
                    request_id,
                    character_id,
                    slot_index,
                },
                Err(error) => request_failed(request_id, "save action bar slot", error),
            },
            PersistenceRequest::SaveActionBarSwap {
                request_id,
                character_id,
                first_slot,
                first_binding,
                second_slot,
                second_binding,
            } => match repository
                .save_action_bar_swap(
                    character_id,
                    first_slot,
                    first_binding,
                    second_slot,
                    second_binding,
                )
                .await
            {
                Ok(()) => PersistenceResponse::ActionBarSwapSaved {
                    request_id,
                    character_id,
                    first_slot,
                    second_slot,
                },
                Err(error) => request_failed(request_id, "save action bar swap", error),
            },
            PersistenceRequest::SaveSkillRank {
                request_id,
                character_id,
                skill_id,
                rank,
                available_points,
            } => match repository
                .save_skill_rank(character_id, skill_id, rank, available_points)
                .await
            {
                Ok(()) => PersistenceResponse::SkillRankSaved {
                    request_id,
                    character_id,
                    skill_id,
                    rank,
                },
                Err(error) => request_failed(request_id, "save skill rank", error),
            },
            PersistenceRequest::SaveSkillPoints {
                request_id,
                character_id,
                available_points,
            } => match repository
                .save_skill_points(character_id, available_points)
                .await
            {
                Ok(()) => PersistenceResponse::SkillPointsSaved {
                    request_id,
                    character_id,
                    available_points,
                },
                Err(error) => request_failed(request_id, "save skill points", error),
            },
            PersistenceRequest::ClearSkills {
                request_id,
                character_id,
            } => match repository.clear_skills(character_id).await {
                Ok(()) => PersistenceResponse::SkillsCleared {
                    request_id,
                    character_id,
                },
                Err(error) => request_failed(request_id, "clear skills", error),
            },
            PersistenceRequest::Shutdown => break,
        };

        if responses.send(response).is_err() {
            break;
        }
    }

    pool.close().await;
    let _ = responses.send(PersistenceResponse::WorkerStopped);
}

fn validate_account_credentials(username: &str, password: &str) -> Result<(), String> {
    let username = username.trim();
    if !(3..=32).contains(&username.len())
        || !username
            .bytes()
            .all(|character| character.is_ascii_alphanumeric() || character == b'_')
    {
        return Err("usernames must contain 3-32 ASCII letters, numbers, or underscores".into());
    }
    if !(8..=128).contains(&password.len()) {
        return Err("passwords must contain 8-128 characters".into());
    }
    Ok(())
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

#[cfg(test)]
mod account_tests {
    use super::validate_account_credentials;

    #[test]
    fn account_credentials_accept_expected_values() {
        assert!(validate_account_credentials("player_01", "long-enough-password").is_ok());
    }

    #[test]
    fn account_credentials_reject_invalid_names_and_short_passwords() {
        assert!(validate_account_credentials("no spaces", "long-enough-password").is_err());
        assert!(validate_account_credentials("valid_name", "short").is_err());
    }
}
