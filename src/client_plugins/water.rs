
use bevy::{color::palettes::css::BLACK, math::vec4, pbr::{ExtendedMaterial, MaterialExtension}, prelude::*, render::texture::{ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor}};
use crate::*;



pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app     
            .add_plugins((MaterialPlugin::<ExtendedMaterial<StandardMaterial, Water>>::default()))     
            .add_systems(OnEnter(AppState::InGame), (setup_water_mesh));


        fn setup_water_mesh(
            mut meshes: ResMut<Assets<Mesh>>,
            mut commands: Commands,
            mut water_materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, Water>>>,  
            asset_server: Res<AssetServer>
        ) {

            let mesh = Mesh::from(Rectangle::default());


            commands.spawn((MaterialMeshBundle {
                mesh: meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(1.0))),
                material: water_materials.add(ExtendedMaterial {
                    base: StandardMaterial {
                        base_color: BLACK.into(),
                        perceptual_roughness: 0.0,
                        ..default()
                    },
                    extension: Water {
                        normals: asset_server.load_with_settings::<Image, ImageLoaderSettings>(
                            "textures/water_normals.png",
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
                        ),
                        // These water settings are just random values to create some
                        // variety.
                        settings: WaterSettings {
                            octave_vectors: [
                                vec4(0.080, 0.059, 0.073, -0.062),
                                vec4(0.153, 0.138, -0.149, -0.195),
                            ],
                            octave_scales: vec4(1.0, 2.1, 7.9, 14.9) * 5.0,
                            octave_strengths: vec4(0.16, 0.18, 0.093, 0.044),
                        },
                    },
                }),
                transform:Transform::from_scale(Vec3::splat(100.0)),
                ..default()
            }
            ));

            /*commands.spawn((MaterialMeshBundle { 
                mesh: meshes.add(mesh),
                transform: Transform::from_xyz(10.0, 10., 10.0),
                material: materials.add(WaterMaterial {
                   
                }),
                ..default()
            },  Name::new("Water")));*/

         
        
        }
        // Add a new system to load the shader
        /*fn setup_water_shader(
            mut commands: Commands,
            mut pipelines: ResMut<Assets<PipelineDescriptor>>,
            mut shaders: ResMut<Assets<Shader>>,
        ) {
            let shader_handle = shaders.add(Shader::from_wgsl(include_str!("water_shader.wgsl")));

            // Create a pipeline using the shader
            let pipeline_handle = pipelines.add(PipelineDescriptor::default_config(ShaderStages {
                vertex: shader_handle.clone(),
                fragment: Some(shader_handle),
            }));

            commands.insert_resource(WaterMaterial { pipeline: pipeline_handle });
        }*/
      
    }

  
}


#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]

pub struct WaterMaterial {}

impl Material for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/minimal.wgsl".into()
    }
    
}


/// A custom [`ExtendedMaterial`] that creates animated water ripples.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct Water {
    /// The normal map image.
    ///
    /// Note that, like all normal maps, this must not be loaded as sRGB.
    #[texture(100)]
    #[sampler(101)]
    normals: Handle<Image>,

    // Parameters to the water shader.
    #[uniform(102)]
    settings: WaterSettings,
}

/// Parameters to the water shader.
#[derive(ShaderType, Debug, Clone)]
struct WaterSettings {
    /// How much to displace each octave each frame, in the u and v directions.
    /// Two octaves are packed into each `vec4`.
    octave_vectors: [Vec4; 2],
    /// How wide the waves are in each octave.
    octave_scales: Vec4,
    /// How high the waves are in each octave.
    octave_strengths: Vec4,
}

const SHADER_ASSET_PATH: &str = "shaders/water_example.wgsl";

impl MaterialExtension for Water {
    fn deferred_fragment_shader() -> ShaderRef {
        SHADER_ASSET_PATH.into()
    }
}