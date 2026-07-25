use sqlx::{mysql::MySqlQueryResult, MySql, MySqlPool, Transaction};
use std::{error::Error, fmt};

use super::models::{
    AccountId, CharacterRecord, CharacterSnapshot, CharacterSummary, NewCharacter,
};
use crate::shared::gameplay::components::CharacterId;

const CHARACTER_COLUMNS: &str = r#"
    id, account_id, slot, name, class_id,
    base_level, base_experience, job_level, job_experience,
    strength, agility, vitality, intelligence, dexterity, luck,
    status_points, skill_points,
    hp, max_hp, sp, max_sp, zeny,
    map_name, position_x, position_y, position_z, facing, revision
"#;
const CHARACTER_SLOT_COUNT: u8 = 9;

#[derive(Clone)]
pub(super) struct CharacterRepository {
    pool: MySqlPool,
}

impl CharacterRepository {
    pub(super) fn new(pool: MySqlPool) -> Self {
        Self { pool }
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
        let result = sqlx::query(
            r#"
            UPDATE characters
            SET
                base_level = ?,
                base_experience = ?,
                job_level = ?,
                job_experience = ?,
                hp = ?,
                max_hp = ?,
                sp = ?,
                max_sp = ?,
                zeny = ?,
                map_name = ?,
                position_x = ?,
                position_y = ?,
                position_z = ?,
                facing = ?,
                revision = revision + 1
            WHERE id = ? AND revision = ? AND deleted_at IS NULL
            "#,
        )
        .bind(snapshot.base_level)
        .bind(snapshot.base_experience)
        .bind(snapshot.job_level)
        .bind(snapshot.job_experience)
        .bind(snapshot.hp)
        .bind(snapshot.max_hp)
        .bind(snapshot.sp)
        .bind(snapshot.max_sp)
        .bind(snapshot.zeny)
        .bind(snapshot.map_name)
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

#[derive(Debug)]
pub(super) enum RepositoryError {
    Database(sqlx::Error),
    InvalidCharacterName,
    InvalidCharacterSlot,
    RevisionConflict(CharacterId),
    CreatedCharacterMissing(CharacterId),
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "{error}"),
            Self::InvalidCharacterName => formatter.write_str(
                "character names must contain 3-24 ASCII letters, numbers, or underscores",
            ),
            Self::InvalidCharacterSlot => write!(
                formatter,
                "character slot must be between 0 and {}",
                CHARACTER_SLOT_COUNT - 1
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
}
