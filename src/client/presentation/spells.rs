use std::{collections::HashMap, time::Duration};

use crate::shared::states::ClientState;
use bevy::{
    animation::{animate_targets, RepeatAnimation},
    light::CascadeShadowConfigBuilder,
    prelude::*,
};
use bevy_asset_loader::prelude::*;

#[derive(AssetCollection, Resource, Debug)]
struct SpellAssets {
    #[asset(path = "models/blue_aura/scene.glb#Scene0")]
    // glb: Handle<Gltf>,
    scene: Handle<WorldAsset>,
}

#[derive(Resource, Debug)]
pub struct SpellAnimation {
    pub animations: Vec<AnimationNodeIndex>,
    #[allow(dead_code)]
    pub graph: Handle<AnimationGraph>,
}

#[derive(Resource, Debug)]
pub struct SpellAnimations(HashMap<SpellId, SpellAnimation>);

#[derive(Event)]
pub(crate) struct CastSpell {
    pub(crate) spell_id: u16,
    pub(crate) translation: Vec3,
}

#[derive(Component, Debug, PartialEq, Eq, Hash)]
struct SpellId(u16);

pub struct SpellAnimationsPlugin;

impl Plugin for SpellAnimationsPlugin {
    fn build(&self, app: &mut App) {
        // add things to your app here
        app.add_systems(
            Startup,
            (
                setup_spell,
                //setup_blue_aura
            ),
        )
        .add_loading_state(LoadingState::new(ClientState::Setup).load_collection::<SpellAssets>())
        .add_systems(
            Update,
            (
                setup_scene_once_loaded.before(animate_targets),
                // remove_spell_shadows.before(animate_targets)
            ),
        )
        .add_observer(on_cast_spell);

        pub fn setup_spell(
            mut commands: Commands,
            asset_server: Res<AssetServer>,
            mut graphs: ResMut<Assets<AnimationGraph>>,
        ) {
            let mut spell_animations: HashMap<SpellId, SpellAnimation> = HashMap::new();
            //let mut spell_animations: Vec<SpellAnimation> = vec![];

            let mut graph = AnimationGraph::new();
            let animations = graph
                .add_clips(
                    [GltfAssetLabel::Animation(0).from_asset("models/light_beam.glb")]
                        .into_iter()
                        .map(|path| asset_server.load(path)),
                    1.0,
                    graph.root,
                )
                .collect();

            let graph = graphs.add(graph);

            /* commands.insert_resource(SpellAnimation {
                animations,
                graph: graph.clone(),
            });*/
            spell_animations.insert(
                SpellId(1),
                SpellAnimation {
                    animations,
                    graph: graph.clone(),
                },
            );
            /*spell_animations.push(SpellAnimation {
                animations,
                graph: graph.clone(),
            });*/

            // Fox
            commands.spawn((
                WorldAssetRoot(
                    asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/light_beam.glb")),
                ),
                Transform {
                    translation: Vec3::new(10.0, -1.0, 10.0),
                    scale: Vec3::splat(1.0),
                    //rotation,
                    ..Default::default()
                },
                /*SceneBundle {
                    scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/light_beam.glb")),
                    transform: Transform {
                        translation: Vec3::new(10.0, -1.0, 10.0),
                        scale: Vec3::splat(1.0),
                        //rotation,
                        ..Default::default()
                    },
                    ..default()

                },    */
                Name::new("Cast"),
            ));

            let mut graph2 = AnimationGraph::new();
            let animations2 = graph2
                .add_clips(
                    [GltfAssetLabel::Animation(0).from_asset("models/blue_aura/scene.glb")]
                        .into_iter()
                        .map(|path| asset_server.load(path)),
                    1.0,
                    graph2.root,
                )
                .collect();

            let graph2 = graphs.add(graph2);

            spell_animations.insert(
                SpellId(2),
                SpellAnimation {
                    animations: animations2,
                    graph: graph2.clone(),
                },
            );

            /*spell_animations.push(SpellAnimation {
                animations: animations2,
                graph: graph2.clone(),
            });*/

            /*commands.insert_resource(SpellAnimation {
                animations: animations2,
                graph: graph2.clone(),
            });*/

            // Fox
            commands.spawn((
                WorldAssetRoot(
                    asset_server
                        .load(GltfAssetLabel::Scene(0).from_asset("models/blue_aura/scene.glb")),
                ),
                Transform {
                    translation: Vec3::new(5.0, -1.0, 5.0),
                    scale: Vec3::splat(1.0),
                    //rotation,
                    ..Default::default()
                },
                /*SceneBundle {
                    scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/blue_aura/scene.glb")),
                    transform: Transform {
                        translation: Vec3::new(5.0, -1.0, 5.0),
                        scale: Vec3::splat(1.0),
                        //rotation,
                        ..Default::default()
                    },
                    ..default()

                },      */
                Name::new("Cast"),
            ));

            let mut graph3 = AnimationGraph::new();
            let animations3 = graph3
                .add_clips(
                    [GltfAssetLabel::Animation(0).from_asset("models/yellow_aura/scene.glb")]
                        .into_iter()
                        .map(|path| asset_server.load(path)),
                    1.0,
                    graph3.root,
                )
                .collect();

            let graph3 = graphs.add(graph3);

            spell_animations.insert(
                SpellId(3),
                SpellAnimation {
                    animations: animations3,
                    graph: graph3.clone(),
                },
            );
            /*spell_animations.push(SpellAnimation {
                animations: animations3,
                graph: graph3.clone(),
            });*/

            // Fox
            commands.spawn((
                WorldAssetRoot(
                    asset_server
                        .load(GltfAssetLabel::Scene(0).from_asset("models/yellow_aura/scene.glb")),
                ),
                Transform {
                    translation: Vec3::new(5.0, -1.0, 10.0),
                    scale: Vec3::splat(1.0),
                    //rotation,
                    ..Default::default()
                },
                /*SceneBundle {
                scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/yellow_aura/scene.glb")),
                transform: Transform {
                    translation: Vec3::new(5.0, -1.0, 10.0),
                    scale: Vec3::splat(1.0),
                    //rotation,
                    ..Default::default()
                },
                ..default()

                },   */
                Name::new("Cast"),
            ));

            let mut graph4 = AnimationGraph::new();
            let animations4 = graph4
                .add_clips(
                    [GltfAssetLabel::Animation(0).from_asset("models/magical_orb/scene.glb")]
                        .into_iter()
                        .map(|path| asset_server.load(path)),
                    1.0,
                    graph4.root,
                )
                .collect();

            let graph4 = graphs.add(graph4);

            spell_animations.insert(
                SpellId(4),
                SpellAnimation {
                    animations: animations4,
                    graph: graph4.clone(),
                },
            );
            /*spell_animations.push(SpellAnimation {
                animations: animations4,
                graph: graph4.clone(),
            });*/
            // println!("graphs {} ", graphs);

            commands.spawn((
                WorldAssetRoot(
                    asset_server
                        .load(GltfAssetLabel::Scene(0).from_asset("models/magical_orb/scene.glb")),
                ),
                Transform {
                    translation: Vec3::new(8.0, 2.0, 7.0),
                    scale: Vec3::splat(1.0),
                    //rotation,
                    ..Default::default()
                },
                /*SceneBundle {
                    scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/magical_orb/scene.glb")),
                    transform: Transform {
                        translation: Vec3::new(8.0, 2.0, 7.0),
                        scale: Vec3::splat(1.0),
                        //rotation,
                        ..Default::default()
                    },
                    ..default()

                },  */
                Name::new("Cast"),
            ));

            commands.insert_resource(SpellAnimations(spell_animations));
        }

        pub fn _remove_spell_shadows(
            mut commands: Commands,
            children: Query<&Children>,
            mut spells: Query<Entity, Added<SpellId>>,
        ) {
            for (spell_entity) in &spells {
                for entity in children.iter_descendants(spell_entity) {
                    info!("quitar sombras de   {entity:?}");

                    commands.entity(entity);
                    /*if let Ok(mut animation_player) = animation_players.get_mut(entity) {
                        animation_player.play(animations.0[animation_index.value].clone_weak()).repeat();
                        *done += 1;
                    }*/
                }
            }
        }

        // Once the scene is loaded, start the animation
        pub fn setup_scene_once_loaded(
            mut commands: Commands,
            animations: Res<SpellAnimations>,
            // animation: Res<SpellAnimation>,
            parents_query: Query<&ChildOf>,
            spells_query: Query<&SpellId>,
            mut animation_players_query: Query<
                (Entity, &mut AnimationPlayer),
                Added<AnimationPlayer>,
            >,
        ) {
            for (entity, mut player) in animation_players_query.iter_mut() {
                let top_entity = get_top_parent(entity, &parents_query);

                // If the top parent has an SpellId component then add the corresponding animation.
                if let Ok(spell_id) = spells_query.get(top_entity) {
                    //info!("linking spell_id to {top_entity:?} for {entity:?}  for {spell_id:?}");

                    //info!("animations  {animations:?}");

                    let mut transitions = AnimationTransitions::new();

                    transitions
                        .play(
                            &mut player,
                            animations.0[spell_id].animations[0],
                            Duration::ZERO,
                        )
                        .repeat();

                    commands
                        .entity(entity)
                        .insert(AnimationGraphHandle(animations.0[spell_id].graph.clone()))
                        .insert(transitions);
                }
            }
            /*for (i, (entity, mut player)) in animation_players_query.iter_mut().enumerate() {


                println!("entity {} ", entity);


                let mut transitions = AnimationTransitions::new();

                println!("animation {:?} ", animations);


                transitions
                    .play(&mut player, animations.0[i].animations[0], Duration::ZERO)
                    .repeat();

                commands
                    .entity(entity)
                    .insert(animations.0[i].graph.clone())
                    .insert(transitions);


            }*/
            /*for (entity, mut player) in &mut players {

                println!("entity {} ", entity);
                let mut transitions = AnimationTransitions::new();

                println!("animation {:?} ", animation);


                transitions
                    .play(&mut player, animations.0[].animations[0], Duration::ZERO)
                    .repeat();

                commands
                    .entity(entity)
                    .insert(animation.graph.clone())
                    .insert(transitions);
                /*transitions
                    .play(&mut player, animation.animations[0], Duration::ZERO)
                    .repeat();

                commands
                    .entity(entity)
                    .insert(animation.graph.clone())
                    .insert(transitions);

                // Make sure to start the animation via the `AnimationTransitions`
                // component. The `AnimationTransitions` component wants to manage all
                // the animations and will get confused if the animations are started
                // directly via the `AnimationPlayer`.

                for(spell_animation ) in &animations.0 {

                    let mut transitions = AnimationTransitions::new();

                    println!("animation {:?} ", spell_animation);

                    transitions
                        .play(&mut player, spell_animation.animations[0], Duration::ZERO)
                        .repeat();

                    commands
                        .entity(entity)
                        .insert(animation.graph.clone())
                        .insert(transitions);

                }*/

            }*/
        }

        fn get_top_parent(mut curr_entity: Entity, parent_query: &Query<&ChildOf>) -> Entity {
            //Loop up all the way to the top parent
            loop {
                if let Ok(parent) = parent_query.get(curr_entity) {
                    curr_entity = parent.parent();
                } else {
                    break;
                }
            }
            curr_entity
        }

        fn on_cast_spell(
            trigger: On<CastSpell>,
            mut commands: Commands,
            asset_server: Res<AssetServer>,
        ) {
            let spell = trigger.event();
            let Some(scene_path) = spell_scene_path(spell.spell_id) else {
                warn!("Ignoring unknown spell id {}", spell.spell_id);
                return;
            };

            println!("spell.spell_id {} ", spell.spell_id);
            commands.spawn((
                WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(scene_path))),
                Transform {
                    translation: spell.translation,
                    scale: Vec3::splat(1.0),
                    //rotation,
                    ..Default::default()
                },
                /*SceneBundle {
                    scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/blue_aura/scene.glb")),
                    //scene:  spell_assets.scene.clone(),
                    transform: Transform {
                        translation: spell.translation,
                        scale: Vec3::splat(1.0),
                        //rotation,
                        ..Default::default()
                    },

                    ..default()

                },   */
                SpellId(spell.spell_id),
                Name::new("Cast"),
            ));
        }
    }
}

fn spell_scene_path(spell_id: u16) -> Option<&'static str> {
    match spell_id {
        1 => Some("models/light_beam.glb"),
        2 => Some("models/blue_aura/scene.glb"),
        3 => Some("models/yellow_aura/scene.glb"),
        4 => Some("models/magical_orb/scene.glb"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_bar_spells_use_different_scene_assets() {
        let scenes = [
            spell_scene_path(1),
            spell_scene_path(2),
            spell_scene_path(3),
        ];

        assert!(scenes.iter().all(Option::is_some));
        assert_ne!(scenes[0], scenes[1]);
        assert_ne!(scenes[1], scenes[2]);
        assert_eq!(spell_scene_path(99), None);
    }
}
