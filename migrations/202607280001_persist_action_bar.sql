CREATE TABLE character_action_bar_slots (
    character_id BIGINT UNSIGNED NOT NULL,
    slot_index TINYINT UNSIGNED NOT NULL,
    binding_kind TINYINT UNSIGNED NOT NULL,
    binding_id INT UNSIGNED NOT NULL DEFAULT 0,
    updated_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
        ON UPDATE CURRENT_TIMESTAMP(6),

    PRIMARY KEY (character_id, slot_index),
    CONSTRAINT character_action_bar_slots_character_id_foreign
        FOREIGN KEY (character_id) REFERENCES characters (id)
        ON DELETE CASCADE,
    CONSTRAINT character_action_bar_slots_slot_in_range CHECK (slot_index < 10),
    CONSTRAINT character_action_bar_slots_binding_kind_valid CHECK (binding_kind <= 3)
) ENGINE = InnoDB
  DEFAULT CHARACTER SET = utf8mb4
  COLLATE = utf8mb4_unicode_ci;
