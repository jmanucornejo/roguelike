use bevy::{pbr::Material, prelude::*, render::render_resource::AsBindGroup, shader::ShaderRef};

pub fn move_water(time: Res<Time>, mut water_materials: ResMut<Assets<WaterMaterial>>) {
    for (_, water) in water_materials.iter_mut() {
        water.time += time.delta_secs();
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct WaterMaterial {
    #[uniform(0)]
    pub time: f32,
}

impl Material for WaterMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/water2.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/water2.wgsl".into()
    }
}
