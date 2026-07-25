ALTER TABLE characters
    MODIFY COLUMN position_x FLOAT NOT NULL DEFAULT -10,
    MODIFY COLUMN position_y FLOAT NOT NULL DEFAULT 1,
    MODIFY COLUMN position_z FLOAT NOT NULL DEFAULT 0;

-- The first fallback selected during development was outside the origin wall,
-- but its terrain surface is below the water navigation threshold. Move only
-- records still at that exact fallback to connected, walkable ground.
UPDATE characters
SET position_x = -10,
    position_y = 1,
    position_z = 0
WHERE map_name = 'starting_map'
  AND position_x = -12
  AND position_y = 1
  AND position_z = 16;
