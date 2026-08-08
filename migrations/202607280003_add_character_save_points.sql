ALTER TABLE characters
    ADD COLUMN save_map_name VARCHAR(64) NULL AFTER map_name,
    ADD COLUMN save_position_x FLOAT NULL AFTER save_map_name,
    ADD COLUMN save_position_y FLOAT NULL AFTER save_position_x,
    ADD COLUMN save_position_z FLOAT NULL AFTER save_position_y;
