ALTER TABLE characters
    MODIFY COLUMN position_x FLOAT NOT NULL DEFAULT -12,
    MODIFY COLUMN position_y FLOAT NOT NULL DEFAULT 1,
    MODIFY COLUMN position_z FLOAT NOT NULL DEFAULT 16;

-- The original defaults placed characters inside the fixed wall at the map
-- origin. Existing records still at that exact default have never been able
-- to move, so relocate them to the new safe starting position.
UPDATE characters
SET position_x = -12,
    position_y = 1,
    position_z = 16
WHERE map_name = 'starting_map'
  AND position_x = 0
  AND position_y = 1
  AND position_z = 0;
