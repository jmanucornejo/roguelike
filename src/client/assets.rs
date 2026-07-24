use bevy::prelude::*;
use bevy_asset_loader::prelude::AssetCollection;

#[derive(AssetCollection, Resource, Debug)]
pub(super) struct SealAssets {
    #[asset(texture_atlas_layout(
        tile_size_x = 64,
        tile_size_y = 67,
        columns = 8,
        rows = 8,
        padding_x = 0,
        padding_y = 0,
        offset_x = 0,
        offset_y = 0
    ))]
    pub(super) layout: Handle<TextureAtlasLayout>,
    #[asset(path = "spritesheets/monsters/seal.png")]
    pub(super) sprite: Handle<Image>,
}

#[derive(AssetCollection, Resource)]
pub(super) struct MyAssets {
    #[asset(texture_atlas_layout(
        tile_size_x = 24,
        tile_size_y = 24,
        columns = 7,
        rows = 1,
        padding_x = 0,
        padding_y = 0,
        offset_x = 0,
        offset_y = 0
    ))]
    pub(super) layout: Handle<TextureAtlasLayout>,
    #[asset(path = "gabe-idle-run.png")]
    pub(super) sprite: Handle<Image>,
}

#[derive(AssetCollection, Resource, Debug)]
pub(super) struct PigAssets {
    #[asset(texture_atlas_layout(
        tile_size_x = 24,
        tile_size_y = 16,
        columns = 1,
        rows = 1,
        padding_x = 0,
        padding_y = 0,
        offset_x = 0,
        offset_y = 0
    ))]
    pub(super) layout: Handle<TextureAtlasLayout>,
    #[asset(path = "pig.png")]
    pub(super) sprite: Handle<Image>,
}

#[derive(AssetCollection, Resource)]
pub(super) struct SkyboxAssets {
    #[asset(path = "skyboxes/Ryfjallet_cubemap.png")]
    pub(super) sprite: Handle<Image>,
}

#[derive(AssetCollection, Resource)]
pub(super) struct ChaskiAssets {
    #[asset(texture_atlas_layout(
        tile_size_x = 128,
        tile_size_y = 128,
        columns = 8,
        rows = 8,
        padding_x = 0,
        padding_y = 0,
        offset_x = 0,
        offset_y = 0
    ))]
    pub(super) layout: Handle<TextureAtlasLayout>,
    #[asset(texture_atlas_layout(
        tile_size_x = 128,
        tile_size_y = 128,
        columns = 8,
        rows = 1,
        padding_x = 0,
        padding_y = 0,
        offset_x = 0,
        offset_y = 0
    ))]
    pub(super) attack_layout: Handle<TextureAtlasLayout>,
    #[asset(path = "spritesheets/chasqui/chasqui.png")]
    pub(super) sprite: Handle<Image>,
    #[asset(path = "spritesheets/chasqui/chasqui_attack-side-down.png")]
    pub(super) attack_side_down: Handle<Image>,
    #[asset(path = "spritesheets/chasqui/chasqui_side-up-attack.png")]
    pub(super) attack_side_up: Handle<Image>,
}
