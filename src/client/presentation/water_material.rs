use bevy::{
    image::{
        ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler,
        ImageSamplerDescriptor,
    },
    light::NotShadowCaster,
    pbr::{Material, MaterialPlugin},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

use crate::shared::{constants::WATER_RENDER_LEVEL, states::ClientState};

const WATER_SHADER_PATH: &str = "shaders/stylized_water.wgsl";
const WATER_NORMALS_PATH: &str = "textures/water_normals.png";
const WATER_SURFACE_SIZE: f32 = 320.0;
const WATER_OPACITY: f32 = 0.78;

#[derive(Component)]
struct WaterSurface;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct WaterMaterial {
    #[uniform(0)]
    shallow_color: LinearRgba,
    #[uniform(1)]
    deep_color: LinearRgba,
    /// x: world-space UV scale, y: wave strength, z/w: layer speeds.
    #[uniform(2)]
    settings: Vec4,
    #[texture(3)]
    #[sampler(4)]
    normals: Handle<Image>,
}

impl Material for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        WATER_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_shadows() -> bool {
        false
    }
}

pub(crate) struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<WaterMaterial>::default())
            .add_systems(OnEnter(ClientState::InGame), spawn_water_surface);
    }
}

fn spawn_water_surface(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    asset_server: Res<AssetServer>,
    existing_water: Query<(), With<WaterSurface>>,
) {
    if !existing_water.is_empty() {
        return;
    }

    let normals = asset_server.load_with_settings::<Image, ImageLoaderSettings>(
        WATER_NORMALS_PATH,
        |settings| {
            settings.is_srgb = false;
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                ..default()
            });
        },
    );

    let material = materials.add(WaterMaterial {
        shallow_color: LinearRgba::new(0.08, 0.52, 0.62, WATER_OPACITY),
        deep_color: LinearRgba::new(0.015, 0.12, 0.24, WATER_OPACITY),
        settings: Vec4::new(0.055, 0.32, 0.018, -0.011),
        normals,
    });

    commands.spawn((
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(WATER_SURFACE_SIZE, WATER_SURFACE_SIZE),
            ),
        ),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, WATER_RENDER_LEVEL, 0.0),
        WaterSurface,
        NotShadowCaster,
        Name::new("Water Surface"),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::constants::WATER_LEVEL;

    #[test]
    fn water_plane_covers_the_authored_map_bounds() {
        assert!(WATER_SURFACE_SIZE >= 301.0);
    }

    #[test]
    fn water_plane_does_not_occlude_dry_zero_height_terrain() {
        assert!(WATER_RENDER_LEVEL < WATER_LEVEL);
    }

    #[test]
    fn water_opacity_is_partially_transparent() {
        assert!(WATER_OPACITY > 0.0 && WATER_OPACITY < 1.0);
    }
}
