CREATE TABLE accounts (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    username VARCHAR(32) NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
        ON UPDATE CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    UNIQUE KEY accounts_username_unique (username)
) ENGINE = InnoDB
  DEFAULT CHARACTER SET = utf8mb4
  COLLATE = utf8mb4_unicode_ci;

CREATE TABLE characters (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    account_id BIGINT UNSIGNED NOT NULL,
    slot TINYINT UNSIGNED NOT NULL,
    name VARCHAR(24) NOT NULL,
    class_id SMALLINT UNSIGNED NOT NULL DEFAULT 0,

    base_level SMALLINT UNSIGNED NOT NULL DEFAULT 1,
    base_experience BIGINT UNSIGNED NOT NULL DEFAULT 0,
    job_level SMALLINT UNSIGNED NOT NULL DEFAULT 1,
    job_experience BIGINT UNSIGNED NOT NULL DEFAULT 0,

    strength SMALLINT UNSIGNED NOT NULL DEFAULT 1,
    agility SMALLINT UNSIGNED NOT NULL DEFAULT 1,
    vitality SMALLINT UNSIGNED NOT NULL DEFAULT 1,
    intelligence SMALLINT UNSIGNED NOT NULL DEFAULT 1,
    dexterity SMALLINT UNSIGNED NOT NULL DEFAULT 1,
    luck SMALLINT UNSIGNED NOT NULL DEFAULT 1,
    status_points INT UNSIGNED NOT NULL DEFAULT 0,
    skill_points INT UNSIGNED NOT NULL DEFAULT 0,

    hp INT UNSIGNED NOT NULL DEFAULT 40,
    max_hp INT UNSIGNED NOT NULL DEFAULT 40,
    sp INT UNSIGNED NOT NULL DEFAULT 10,
    max_sp INT UNSIGNED NOT NULL DEFAULT 10,
    zeny BIGINT UNSIGNED NOT NULL DEFAULT 0,

    map_name VARCHAR(64) NOT NULL DEFAULT 'starting_map',
    position_x FLOAT NOT NULL DEFAULT 0,
    position_y FLOAT NOT NULL DEFAULT 1,
    position_z FLOAT NOT NULL DEFAULT 0,
    facing TINYINT UNSIGNED NOT NULL DEFAULT 0,

    revision BIGINT UNSIGNED NOT NULL DEFAULT 0,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
        ON UPDATE CURRENT_TIMESTAMP(6),
    deleted_at TIMESTAMP(6) NULL,

    PRIMARY KEY (id),
    UNIQUE KEY characters_name_unique (name),
    UNIQUE KEY characters_account_slot_unique (account_id, slot),
    KEY characters_account_id_index (account_id),
    CONSTRAINT characters_account_id_foreign
        FOREIGN KEY (account_id) REFERENCES accounts (id),
    CONSTRAINT characters_slot_in_range CHECK (slot < 9)
) ENGINE = InnoDB
  DEFAULT CHARACTER SET = utf8mb4
  COLLATE = utf8mb4_unicode_ci;

CREATE TABLE inventory_items (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    character_id BIGINT UNSIGNED NOT NULL,
    item_definition_id INT UNSIGNED NOT NULL,
    quantity INT UNSIGNED NOT NULL DEFAULT 1,
    equipped_slot SMALLINT UNSIGNED NULL,
    refinement_level TINYINT UNSIGNED NOT NULL DEFAULT 0,
    metadata JSON NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
        ON UPDATE CURRENT_TIMESTAMP(6),

    PRIMARY KEY (id),
    KEY inventory_items_character_id_index (character_id),
    CONSTRAINT inventory_items_character_id_foreign
        FOREIGN KEY (character_id) REFERENCES characters (id)
        ON DELETE CASCADE,
    CONSTRAINT inventory_items_quantity_positive CHECK (quantity > 0)
) ENGINE = InnoDB
  DEFAULT CHARACTER SET = utf8mb4
  COLLATE = utf8mb4_unicode_ci;

CREATE TABLE character_skills (
    character_id BIGINT UNSIGNED NOT NULL,
    skill_id INT UNSIGNED NOT NULL,
    skill_level SMALLINT UNSIGNED NOT NULL DEFAULT 1,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
        ON UPDATE CURRENT_TIMESTAMP(6),

    PRIMARY KEY (character_id, skill_id),
    CONSTRAINT character_skills_character_id_foreign
        FOREIGN KEY (character_id) REFERENCES characters (id)
        ON DELETE CASCADE,
    CONSTRAINT character_skills_level_positive CHECK (skill_level > 0)
) ENGINE = InnoDB
  DEFAULT CHARACTER SET = utf8mb4
  COLLATE = utf8mb4_unicode_ci;
