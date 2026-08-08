ALTER TABLE characters
    MODIFY COLUMN attribute_points INT UNSIGNED NOT NULL DEFAULT 48;

UPDATE characters
SET attribute_points = 48
WHERE base_level = 1
  AND might = 1
  AND finesse = 1
  AND agility = 1
  AND vitality = 1
  AND intellect = 1
  AND spirit = 1
  AND attribute_points = 0;
