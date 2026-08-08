use sqlx::{mysql::MySqlQueryResult, MySql, MySqlPool, Transaction};
use std::{error::Error, fmt};

use super::models::{
    AccountId, CharacterRecord, CharacterSnapshot, CharacterSummary, NewCharacter,
};
use crate::shared::gameplay::action_bar::{
    ActionBarBinding, ActionBarLayout, ACTION_BAR_SLOT_COUNT,
};
use crate::shared::gameplay::components::{CharacterId, Equipment, EquipmentSlot};
use crate::shared::gameplay::items::{Inventory, ItemDefinitionId};
use crate::shared::gameplay::skills::{LearnedSkill, SkillId};
use crate::shared::network::messages::CHARACTER_SLOT_COUNT;

const CHARACTER_COLUMNS: &str = r#"
    id, account_id, slot, name, class_id,
    base_level, base_experience, job_level, job_experience,
    might, finesse, agility, vitality, intellect, spirit,
    attribute_points, skill_points,
    hp, max_hp, sp, max_sp, gold,
    map_name, save_map_name, save_position_x, save_position_y, save_position_z,
    position_x, position_y, position_z, facing, revision
"#;

#[derive(Clone)]
pub(super) struct CharacterRepository {
    pool: MySqlPool,
}

impl CharacterRepository {
    pub(super) fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }

    pub(super) async fn account_credentials(
        &self,
        username: &str,
    ) -> Result<Option<(u64, String)>, sqlx::Error> {
        sqlx::query_as::<_, (u64, String)>(
            "SELECT id, password_hash FROM accounts WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
    }

    pub(super) async fn create_account(
        &self,
        username: &str,
        password_hash: &str,
    ) -> Result<AccountId, RepositoryError> {
        // Older development builds created `dev_<id>` rows with an explicit
        // non-password marker. Activating one preserves its existing characters.
        let activated = sqlx::query(
            r#"
            UPDATE accounts
            SET password_hash = ?
            WHERE username = ? AND password_hash = 'development-account-no-password'
            "#,
        )
        .bind(password_hash)
        .bind(username)
        .execute(&self.pool)
        .await?;
        if activated.rows_affected() == 1 {
            let account_id =
                sqlx::query_scalar::<_, u64>("SELECT id FROM accounts WHERE username = ?")
                    .bind(username)
                    .fetch_one(&self.pool)
                    .await?;
            return Ok(AccountId(account_id));
        }

        let result = sqlx::query("INSERT INTO accounts (username, password_hash) VALUES (?, ?)")
            .bind(username)
            .bind(password_hash)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                if error
                    .as_database_error()
                    .is_some_and(|database_error| database_error.is_unique_violation())
                {
                    RepositoryError::UsernameTaken
                } else {
                    RepositoryError::Database(error)
                }
            })?;

        Ok(AccountId(result.last_insert_id()))
    }

    pub(super) async fn list_characters(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<CharacterSummary>, sqlx::Error> {
        sqlx::query_as::<_, CharacterSummary>(
            r#"
            SELECT id, slot, name, class_id, base_level, job_level
            FROM characters
            WHERE account_id = ? AND deleted_at IS NULL
            ORDER BY slot
            "#,
        )
        .bind(account_id.0)
        .fetch_all(&self.pool)
        .await
    }

    pub(super) async fn create_character(
        &self,
        character: NewCharacter,
    ) -> Result<CharacterRecord, RepositoryError> {
        validate_character_name(&character.name)?;
        validate_character_slot(character.slot)?;

        let mut transaction = self.pool.begin().await?;
        let result = insert_character(&mut transaction, &character).await?;
        let character_id = CharacterId(result.last_insert_id());
        transaction.commit().await?;

        self.load_character(character.account_id, character_id)
            .await?
            .ok_or(RepositoryError::CreatedCharacterMissing(character_id))
    }

    pub(super) async fn load_character(
        &self,
        account_id: AccountId,
        character_id: CharacterId,
    ) -> Result<Option<CharacterRecord>, sqlx::Error> {
        let query = format!(
            r#"
            SELECT {CHARACTER_COLUMNS}
            FROM characters
            WHERE id = ? AND account_id = ? AND deleted_at IS NULL
            "#,
        );

        sqlx::query_as::<_, CharacterRecord>(&query)
            .bind(character_id.0)
            .bind(account_id.0)
            .fetch_optional(&self.pool)
            .await
    }

    /// Temporary development flow used until account authentication and the
    /// character-selection screen are available.
    pub(super) async fn load_or_create_default_character(
        &self,
        account_id: AccountId,
    ) -> Result<CharacterRecord, RepositoryError> {
        self.ensure_development_account(account_id).await?;

        if let Some(character_id) = self.default_character_id(account_id).await? {
            return self
                .load_character(account_id, character_id)
                .await?
                .ok_or(RepositoryError::CreatedCharacterMissing(character_id));
        }

        self.create_character(NewCharacter {
            account_id,
            slot: 0,
            name: development_character_name(account_id),
        })
        .await
    }

    pub(super) async fn save_character(
        &self,
        snapshot: CharacterSnapshot,
    ) -> Result<u64, RepositoryError> {
        let save_map_name = snapshot
            .save_point
            .as_ref()
            .map(|save_point| save_point.map_name.as_str());
        let save_position_x = snapshot
            .save_point
            .as_ref()
            .map(|save_point| save_point.position[0]);
        let save_position_y = snapshot
            .save_point
            .as_ref()
            .map(|save_point| save_point.position[1]);
        let save_position_z = snapshot
            .save_point
            .as_ref()
            .map(|save_point| save_point.position[2]);
        let result = sqlx::query(
            r#"
            UPDATE characters
            SET
                class_id = ?,
                base_level = ?,
                base_experience = ?,
                job_level = ?,
                job_experience = ?,
                might = ?,
                finesse = ?,
                agility = ?,
                vitality = ?,
                intellect = ?,
                spirit = ?,
                attribute_points = ?,
                hp = ?,
                max_hp = ?,
                sp = ?,
                max_sp = ?,
                gold = ?,
                map_name = ?,
                save_map_name = ?,
                save_position_x = ?,
                save_position_y = ?,
                save_position_z = ?,
                position_x = ?,
                position_y = ?,
                position_z = ?,
                facing = ?,
                revision = revision + 1
            WHERE id = ? AND revision = ? AND deleted_at IS NULL
            "#,
        )
        .bind(snapshot.class_id)
        .bind(snapshot.base_level)
        .bind(snapshot.base_experience)
        .bind(snapshot.job_level)
        .bind(snapshot.job_experience)
        .bind(snapshot.stats.might)
        .bind(snapshot.stats.finesse)
        .bind(snapshot.stats.agility)
        .bind(snapshot.stats.vitality)
        .bind(snapshot.stats.intellect)
        .bind(snapshot.stats.spirit)
        .bind(snapshot.stats.available_points)
        .bind(snapshot.hp)
        .bind(snapshot.max_hp)
        .bind(snapshot.sp)
        .bind(snapshot.max_sp)
        .bind(snapshot.gold)
        .bind(snapshot.map_name)
        .bind(save_map_name)
        .bind(save_position_x)
        .bind(save_position_y)
        .bind(save_position_z)
        .bind(snapshot.position[0])
        .bind(snapshot.position[1])
        .bind(snapshot.position[2])
        .bind(snapshot.facing)
        .bind(snapshot.character_id.0)
        .bind(snapshot.expected_revision)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::RevisionConflict(snapshot.character_id));
        }

        Ok(snapshot.expected_revision + 1)
    }

    pub(super) async fn load_inventory(
        &self,
        character_id: CharacterId,
    ) -> Result<Inventory, sqlx::Error> {
        let rows = sqlx::query_as::<_, (u32, u32)>(
            r#"
            SELECT item_definition_id, quantity
            FROM inventory_items
            WHERE character_id = ? AND equipped_slot IS NULL
            ORDER BY item_definition_id, id
            "#,
        )
        .bind(character_id.0)
        .fetch_all(&self.pool)
        .await?;

        let mut inventory = Inventory::default();
        for (item_definition_id, quantity) in rows {
            inventory.add(ItemDefinitionId(item_definition_id), quantity);
        }
        Ok(inventory)
    }

    pub(super) async fn load_equipment(
        &self,
        character_id: CharacterId,
    ) -> Result<Equipment, sqlx::Error> {
        let rows = sqlx::query_as::<_, (u16, u32)>(
            r#"
            SELECT equipped_slot, item_definition_id
            FROM inventory_items
            WHERE character_id = ? AND equipped_slot IS NOT NULL
            ORDER BY equipped_slot, id
            "#,
        )
        .bind(character_id.0)
        .fetch_all(&self.pool)
        .await?;

        let mut equipment = Equipment::default();
        for (slot_index, item_definition_id) in rows {
            if let Some(slot) = EquipmentSlot::from_index(slot_index) {
                equipment.set(slot, Some(ItemDefinitionId(item_definition_id)));
            }
        }
        Ok(equipment)
    }

    pub(super) async fn load_skills(
        &self,
        character_id: CharacterId,
    ) -> Result<Vec<LearnedSkill>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (u32, u16)>(
            r#"
            SELECT skill_id, skill_level
            FROM character_skills
            WHERE character_id = ?
            ORDER BY skill_id
            "#,
        )
        .bind(character_id.0)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(skill_id, rank)| {
                Some(LearnedSkill {
                    id: SkillId(u16::try_from(skill_id).ok()?),
                    rank: u8::try_from(rank).ok()?,
                })
            })
            .collect())
    }

    pub(super) async fn save_skill_rank(
        &self,
        character_id: CharacterId,
        skill_id: SkillId,
        rank: u8,
        available_points: u32,
    ) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            r#"
            UPDATE characters
            SET skill_points = ?
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(available_points)
        .bind(character_id.0)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO character_skills (character_id, skill_id, skill_level)
            VALUES (?, ?, ?)
            ON DUPLICATE KEY UPDATE skill_level = VALUES(skill_level)
            "#,
        )
        .bind(character_id.0)
        .bind(skill_id.0)
        .bind(rank)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub(super) async fn save_skill_points(
        &self,
        character_id: CharacterId,
        available_points: u32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE characters
            SET skill_points = ?
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(available_points)
        .bind(character_id.0)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn clear_skills(&self, character_id: CharacterId) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            r#"
            UPDATE characters
            SET skill_points = 0
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(character_id.0)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM character_skills WHERE character_id = ?")
            .bind(character_id.0)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub(super) async fn load_action_bar(
        &self,
        character_id: CharacterId,
    ) -> Result<ActionBarLayout, sqlx::Error> {
        let rows = sqlx::query_as::<_, (u8, u8, u32)>(
            r#"
            SELECT slot_index, binding_kind, binding_id
            FROM character_action_bar_slots
            WHERE character_id = ?
            ORDER BY slot_index
            "#,
        )
        .bind(character_id.0)
        .fetch_all(&self.pool)
        .await?;

        let mut action_bar = ActionBarLayout::default();
        for (slot_index, binding_kind, binding_id) in rows {
            let binding = persisted_action_bar_binding(binding_kind, binding_id);
            action_bar.set(slot_index as usize, binding);
        }
        Ok(action_bar)
    }

    pub(super) async fn save_action_bar_slot(
        &self,
        character_id: CharacterId,
        slot_index: u8,
        binding: Option<ActionBarBinding>,
    ) -> Result<(), RepositoryError> {
        validate_action_bar_slot(slot_index)?;
        let (binding_kind, binding_id) = persisted_action_bar_values(binding);

        sqlx::query(
            r#"
            INSERT INTO character_action_bar_slots (
                character_id,
                slot_index,
                binding_kind,
                binding_id
            )
            VALUES (?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                binding_kind = VALUES(binding_kind),
                binding_id = VALUES(binding_id)
            "#,
        )
        .bind(character_id.0)
        .bind(slot_index)
        .bind(binding_kind)
        .bind(binding_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub(super) async fn save_action_bar_swap(
        &self,
        character_id: CharacterId,
        first_slot: u8,
        first_binding: Option<ActionBarBinding>,
        second_slot: u8,
        second_binding: Option<ActionBarBinding>,
    ) -> Result<(), RepositoryError> {
        validate_action_bar_slot(first_slot)?;
        validate_action_bar_slot(second_slot)?;

        let mut transaction = self.pool.begin().await?;
        for (slot_index, binding) in [(first_slot, first_binding), (second_slot, second_binding)] {
            let (binding_kind, binding_id) = persisted_action_bar_values(binding);
            sqlx::query(
                r#"
                INSERT INTO character_action_bar_slots (
                    character_id,
                    slot_index,
                    binding_kind,
                    binding_id
                )
                VALUES (?, ?, ?, ?)
                ON DUPLICATE KEY UPDATE
                    binding_kind = VALUES(binding_kind),
                    binding_id = VALUES(binding_id)
                "#,
            )
            .bind(character_id.0)
            .bind(slot_index)
            .bind(binding_kind)
            .bind(binding_id)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;

        Ok(())
    }

    pub(super) async fn add_inventory_item(
        &self,
        character_id: CharacterId,
        item_id: ItemDefinitionId,
        quantity: u32,
    ) -> Result<u32, RepositoryError> {
        if quantity == 0 {
            return Ok(0);
        }

        let mut transaction = self.pool.begin().await?;

        // Lock the character row first. The persistence worker is currently
        // serial, but this also keeps stacking correct if more workers are
        // introduced later.
        sqlx::query_scalar::<_, u64>(
            r#"
            SELECT id
            FROM characters
            WHERE id = ? AND deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(character_id.0)
        .fetch_one(&mut *transaction)
        .await?;

        let existing = sqlx::query_as::<_, (u64, u32)>(
            r#"
            SELECT id, quantity
            FROM inventory_items
            WHERE character_id = ?
              AND item_definition_id = ?
              AND equipped_slot IS NULL
            ORDER BY id
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(character_id.0)
        .bind(item_id.0)
        .fetch_optional(&mut *transaction)
        .await?;

        let new_quantity = if let Some((inventory_item_id, current_quantity)) = existing {
            let new_quantity = current_quantity.saturating_add(quantity);
            sqlx::query(
                r#"
                UPDATE inventory_items
                SET quantity = ?
                WHERE id = ?
                "#,
            )
            .bind(new_quantity)
            .bind(inventory_item_id)
            .execute(&mut *transaction)
            .await?;
            new_quantity
        } else {
            sqlx::query(
                r#"
                INSERT INTO inventory_items (
                    character_id,
                    item_definition_id,
                    quantity
                )
                VALUES (?, ?, ?)
                "#,
            )
            .bind(character_id.0)
            .bind(item_id.0)
            .bind(quantity)
            .execute(&mut *transaction)
            .await?;
            quantity
        };

        transaction.commit().await?;
        Ok(new_quantity)
    }

    pub(super) async fn remove_inventory_item(
        &self,
        character_id: CharacterId,
        item_id: ItemDefinitionId,
        quantity: u32,
    ) -> Result<u32, RepositoryError> {
        if quantity == 0 {
            return Ok(0);
        }

        let mut transaction = self.pool.begin().await?;
        sqlx::query_scalar::<_, u64>(
            r#"
            SELECT id
            FROM characters
            WHERE id = ? AND deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(character_id.0)
        .fetch_one(&mut *transaction)
        .await?;

        let existing = sqlx::query_as::<_, (u64, u32)>(
            r#"
            SELECT id, quantity
            FROM inventory_items
            WHERE character_id = ?
              AND item_definition_id = ?
              AND equipped_slot IS NULL
            ORDER BY id
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(character_id.0)
        .bind(item_id.0)
        .fetch_optional(&mut *transaction)
        .await?;

        let Some((inventory_item_id, current_quantity)) = existing else {
            return Err(RepositoryError::InsufficientInventoryItem {
                character_id,
                item_id,
            });
        };
        if current_quantity < quantity {
            return Err(RepositoryError::InsufficientInventoryItem {
                character_id,
                item_id,
            });
        }

        let new_quantity = current_quantity - quantity;
        if new_quantity == 0 {
            sqlx::query("DELETE FROM inventory_items WHERE id = ?")
                .bind(inventory_item_id)
                .execute(&mut *transaction)
                .await?;
        } else {
            sqlx::query("UPDATE inventory_items SET quantity = ? WHERE id = ?")
                .bind(new_quantity)
                .bind(inventory_item_id)
                .execute(&mut *transaction)
                .await?;
        }

        transaction.commit().await?;
        Ok(new_quantity)
    }

    pub(super) async fn equip_inventory_item(
        &self,
        character_id: CharacterId,
        item_id: ItemDefinitionId,
        slot: EquipmentSlot,
    ) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        lock_character(&mut transaction, character_id).await?;

        let occupied = sqlx::query_scalar::<_, u64>(
            r#"
            SELECT id
            FROM inventory_items
            WHERE character_id = ? AND equipped_slot = ?
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(character_id.0)
        .bind(slot.index() as u16)
        .fetch_optional(&mut *transaction)
        .await?;
        if occupied.is_some() {
            return Err(RepositoryError::EquipmentSlotOccupied { character_id, slot });
        }

        let existing = sqlx::query_as::<_, (u64, u32)>(
            r#"
            SELECT id, quantity
            FROM inventory_items
            WHERE character_id = ?
              AND item_definition_id = ?
              AND equipped_slot IS NULL
            ORDER BY id
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(character_id.0)
        .bind(item_id.0)
        .fetch_optional(&mut *transaction)
        .await?;

        let Some((inventory_item_id, quantity)) = existing else {
            return Err(RepositoryError::InsufficientInventoryItem {
                character_id,
                item_id,
            });
        };

        if quantity == 1 {
            sqlx::query("UPDATE inventory_items SET equipped_slot = ? WHERE id = ?")
                .bind(slot.index() as u16)
                .bind(inventory_item_id)
                .execute(&mut *transaction)
                .await?;
        } else {
            sqlx::query("UPDATE inventory_items SET quantity = ? WHERE id = ?")
                .bind(quantity - 1)
                .bind(inventory_item_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query(
                r#"
                INSERT INTO inventory_items (
                    character_id,
                    item_definition_id,
                    quantity,
                    equipped_slot
                )
                VALUES (?, ?, 1, ?)
                "#,
            )
            .bind(character_id.0)
            .bind(item_id.0)
            .bind(slot.index() as u16)
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    pub(super) async fn unequip_inventory_item(
        &self,
        character_id: CharacterId,
        slot: EquipmentSlot,
    ) -> Result<ItemDefinitionId, RepositoryError> {
        let mut transaction = self.pool.begin().await?;
        lock_character(&mut transaction, character_id).await?;

        let equipped = sqlx::query_as::<_, (u64, u32, u32)>(
            r#"
            SELECT id, item_definition_id, quantity
            FROM inventory_items
            WHERE character_id = ? AND equipped_slot = ?
            ORDER BY id
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(character_id.0)
        .bind(slot.index() as u16)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((equipped_item_id, item_definition_id, equipped_quantity)) = equipped else {
            return Err(RepositoryError::EquipmentSlotEmpty { character_id, slot });
        };

        let inventory_stack = sqlx::query_as::<_, (u64, u32)>(
            r#"
            SELECT id, quantity
            FROM inventory_items
            WHERE character_id = ?
              AND item_definition_id = ?
              AND equipped_slot IS NULL
            ORDER BY id
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(character_id.0)
        .bind(item_definition_id)
        .fetch_optional(&mut *transaction)
        .await?;

        if let Some((inventory_item_id, quantity)) = inventory_stack {
            sqlx::query("UPDATE inventory_items SET quantity = ? WHERE id = ?")
                .bind(quantity.saturating_add(equipped_quantity))
                .bind(inventory_item_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query("DELETE FROM inventory_items WHERE id = ?")
                .bind(equipped_item_id)
                .execute(&mut *transaction)
                .await?;
        } else {
            sqlx::query("UPDATE inventory_items SET equipped_slot = NULL WHERE id = ?")
                .bind(equipped_item_id)
                .execute(&mut *transaction)
                .await?;
        }

        transaction.commit().await?;
        Ok(ItemDefinitionId(item_definition_id))
    }

    async fn ensure_development_account(&self, account_id: AccountId) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO accounts (id, username, password_hash)
            VALUES (?, ?, ?)
            ON DUPLICATE KEY UPDATE id = id
            "#,
        )
        .bind(account_id.0)
        .bind(format!("dev_{}", account_id.0))
        .bind("development-account-no-password")
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn default_character_id(
        &self,
        account_id: AccountId,
    ) -> Result<Option<CharacterId>, sqlx::Error> {
        let character_id = sqlx::query_scalar::<_, u64>(
            r#"
            SELECT id
            FROM characters
            WHERE account_id = ? AND slot = 0 AND deleted_at IS NULL
            "#,
        )
        .bind(account_id.0)
        .fetch_optional(&self.pool)
        .await?;

        Ok(character_id.map(CharacterId))
    }
}

fn development_character_name(account_id: AccountId) -> String {
    format!("P_{}", account_id.0)
}

async fn lock_character(
    transaction: &mut Transaction<'_, MySql>,
    character_id: CharacterId,
) -> Result<(), sqlx::Error> {
    sqlx::query_scalar::<_, u64>(
        r#"
        SELECT id
        FROM characters
        WHERE id = ? AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(character_id.0)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_character(
    transaction: &mut Transaction<'_, MySql>,
    character: &NewCharacter,
) -> Result<MySqlQueryResult, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO characters (account_id, slot, name)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(character.account_id.0)
    .bind(character.slot)
    .bind(&character.name)
    .execute(&mut **transaction)
    .await
}

fn validate_character_name(name: &str) -> Result<(), RepositoryError> {
    let valid_length = (3..=24).contains(&name.len());
    let valid_characters = name
        .bytes()
        .all(|character| character.is_ascii_alphanumeric() || character == b'_');

    if valid_length && valid_characters {
        Ok(())
    } else {
        Err(RepositoryError::InvalidCharacterName)
    }
}

fn validate_character_slot(slot: u8) -> Result<(), RepositoryError> {
    if slot < CHARACTER_SLOT_COUNT {
        Ok(())
    } else {
        Err(RepositoryError::InvalidCharacterSlot)
    }
}

fn validate_action_bar_slot(slot: u8) -> Result<(), RepositoryError> {
    if (slot as usize) < ACTION_BAR_SLOT_COUNT {
        Ok(())
    } else {
        Err(RepositoryError::InvalidActionBarSlot)
    }
}

fn persisted_action_bar_values(binding: Option<ActionBarBinding>) -> (u8, u32) {
    match binding {
        None => (0, 0),
        Some(ActionBarBinding::Spell(spell_id)) => (1, spell_id.into()),
        Some(ActionBarBinding::Item(item_id)) => (2, item_id.0),
        Some(ActionBarBinding::Skill(skill_id)) => (3, skill_id.0.into()),
    }
}

fn persisted_action_bar_binding(binding_kind: u8, binding_id: u32) -> Option<ActionBarBinding> {
    match binding_kind {
        1 => u16::try_from(binding_id).ok().map(ActionBarBinding::Spell),
        2 => Some(ActionBarBinding::Item(ItemDefinitionId(binding_id))),
        3 => u16::try_from(binding_id)
            .ok()
            .map(SkillId)
            .map(ActionBarBinding::Skill),
        _ => None,
    }
}

#[derive(Debug)]
pub(super) enum RepositoryError {
    Database(sqlx::Error),
    UsernameTaken,
    InvalidCharacterName,
    InvalidCharacterSlot,
    InvalidActionBarSlot,
    RevisionConflict(CharacterId),
    CreatedCharacterMissing(CharacterId),
    InsufficientInventoryItem {
        character_id: CharacterId,
        item_id: ItemDefinitionId,
    },
    EquipmentSlotOccupied {
        character_id: CharacterId,
        slot: EquipmentSlot,
    },
    EquipmentSlotEmpty {
        character_id: CharacterId,
        slot: EquipmentSlot,
    },
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "{error}"),
            Self::UsernameTaken => formatter.write_str("that username is already in use"),
            Self::InvalidCharacterName => formatter.write_str(
                "character names must contain 3-24 ASCII letters, numbers, or underscores",
            ),
            Self::InvalidCharacterSlot => write!(
                formatter,
                "character slot must be between 0 and {}",
                CHARACTER_SLOT_COUNT - 1
            ),
            Self::InvalidActionBarSlot => write!(
                formatter,
                "action bar slot must be between 0 and {}",
                ACTION_BAR_SLOT_COUNT - 1
            ),
            Self::RevisionConflict(character_id) => write!(
                formatter,
                "character {} changed after it was loaded",
                character_id.0
            ),
            Self::CreatedCharacterMissing(character_id) => write!(
                formatter,
                "newly created character {} could not be loaded",
                character_id.0
            ),
            Self::InsufficientInventoryItem {
                character_id,
                item_id,
            } => write!(
                formatter,
                "character {} does not have enough of item {}",
                character_id.0, item_id.0
            ),
            Self::EquipmentSlotOccupied { character_id, slot } => write!(
                formatter,
                "character {} already has an item in {}",
                character_id.0,
                slot.name()
            ),
            Self::EquipmentSlotEmpty { character_id, slot } => write!(
                formatter,
                "character {} has no item in {}",
                character_id.0,
                slot.name()
            ),
        }
    }
}

impl Error for RepositoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for RepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_name_validation_accepts_safe_names() {
        assert!(validate_character_name("Manuel_01").is_ok());
    }

    #[test]
    fn character_name_validation_rejects_sql_and_whitespace_characters() {
        assert!(validate_character_name("a").is_err());
        assert!(validate_character_name("name with spaces").is_err());
        assert!(validate_character_name("name'; DROP TABLE characters").is_err());
    }

    #[test]
    fn character_slots_are_bounded() {
        assert!(validate_character_slot(0).is_ok());
        assert!(validate_character_slot(8).is_ok());
        assert!(validate_character_slot(9).is_err());
    }

    #[test]
    fn development_character_names_fit_database_validation() {
        let shortest = development_character_name(AccountId(1));
        let longest = development_character_name(AccountId(u64::MAX));

        assert!(validate_character_name(&shortest).is_ok());
        assert!(validate_character_name(&longest).is_ok());
        assert!(longest.len() <= 24);
    }

    #[test]
    fn action_bar_bindings_round_trip_through_database_values() {
        let bindings = [
            None,
            Some(ActionBarBinding::Spell(4)),
            Some(ActionBarBinding::Item(
                crate::shared::gameplay::items::RED_HERB,
            )),
            Some(ActionBarBinding::Skill(
                crate::shared::gameplay::skills::SkillId(301),
            )),
        ];

        for binding in bindings {
            let (kind, id) = persisted_action_bar_values(binding);
            assert_eq!(persisted_action_bar_binding(kind, id), binding);
        }
        assert!(validate_action_bar_slot(9).is_ok());
        assert!(validate_action_bar_slot(10).is_err());
    }
}
