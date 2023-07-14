use bracket_lib::prelude::*;
use specs::prelude::*;

mod rect;
use rect::Rect;

mod map;
use map::*;

mod player;
use player::*;

mod components;
use components::*;

mod visibility_system;
use visibility_system::*;

mod monster_ai_system;
use monster_ai_system::*;

mod map_indexing_system;
use map_indexing_system::*;

mod melee_combat_system;
use melee_combat_system::*;

mod damage_system;
use damage_system::*;

mod gui;
use gui::*;

mod gamelog;
use gamelog::*;

mod spawner;

mod inventory_systems;
use inventory_systems::*;

#[derive(PartialEq, Copy, Clone)]
pub enum RunState {
    AwaitingInput,
    PreRun,
    PlayerTurn,
    MonsterTurn,
    ShowInventory,
}

pub struct State {
    ecs: World,
}

impl State {
    fn run_systems(&mut self) {
        let mut vis = VisibilitySystem {};
        vis.run_now(&self.ecs);
        let mut mob = MonsterAI {};
        mob.run_now(&self.ecs);
        let mut mapindex = MapIndexingSystem {};
        mapindex.run_now(&self.ecs);
        let mut melee = MeleeCombatSystem {};
        melee.run_now(&self.ecs);
        let mut damage = DamageSystem {};
        damage.run_now(&self.ecs);
        let mut pickup = ItemCollectionSystem {};
        pickup.run_now(&self.ecs);
        let mut potions = PotionUseSystem {};
        potions.run_now(&self.ecs);
        self.ecs.maintain();
    }
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut BTerm) {
        ctx.cls();
        draw_map(&self.ecs, ctx);

        {
            let positions = self.ecs.read_storage::<Position>();
            let renderables = self.ecs.read_storage::<Renderable>();
            let map = self.ecs.fetch::<Map>();

            for (pos, render) in (&positions, &renderables).join() {
                let idx = map.xy_idx(pos.x, pos.y);
                if map.visible_tiles[idx] {
                    ctx.set(pos.x, pos.y, render.fg, render.bg, render.glyph)
                };
            }

            draw_ui(&self.ecs, ctx);
        }

        let mut newrunstate = *self.ecs.fetch::<RunState>();
        newrunstate = match newrunstate {
            RunState::PreRun => {
                self.run_systems();
                self.ecs.maintain();
                RunState::AwaitingInput
            }
            RunState::AwaitingInput => player_input(self, ctx),
            RunState::PlayerTurn => {
                self.run_systems();
                self.ecs.maintain();
                RunState::MonsterTurn
            }
            RunState::MonsterTurn => {
                self.run_systems();
                self.ecs.maintain();
                RunState::AwaitingInput
            }
            RunState::ShowInventory => {
                let result = gui::show_inventory(self, ctx);
                match result.0 {
                    gui::ItemMenuResult::Cancel => RunState::AwaitingInput,
                    gui::ItemMenuResult::NoResponse => newrunstate,
                    gui::ItemMenuResult::Selected => {
                        let item_entity = result.1.unwrap();
                        let mut intent = self.ecs.write_storage::<WantsToDrinkPotion>();
                        intent
                            .insert(
                                *self.ecs.fetch::<Entity>(),
                                WantsToDrinkPotion {
                                    potion: item_entity,
                                },
                            )
                            .expect("Unable to insert intent");
                        RunState::PlayerTurn
                    }
                }
            }
        };
        {
            let mut runwriter = self.ecs.write_resource::<RunState>();
            *runwriter = newrunstate;
        }

        damage_system::delete_the_dead(&mut self.ecs);
    }
}

fn main() -> BError {
    let mut context = BTermBuilder::simple80x50()
        .with_title("My Roguelike")
        .build()?;
    context.with_post_scanlines(true);

    let mut gs = State { ecs: World::new() };
    gs.ecs.register::<Position>();
    gs.ecs.register::<Renderable>();
    gs.ecs.register::<Player>();
    gs.ecs.register::<Viewshed>();
    gs.ecs.register::<Monster>();
    gs.ecs.register::<Name>();
    gs.ecs.register::<BlocksTile>();
    gs.ecs.register::<CombatStats>();
    gs.ecs.register::<WantsToMelee>();
    gs.ecs.register::<SufferDamage>();
    gs.ecs.register::<Item>();
    gs.ecs.register::<Potion>();
    gs.ecs.register::<InBackpack>();
    gs.ecs.register::<WantsToPickupItem>();
    gs.ecs.register::<WantsToDrinkPotion>();

    let map = Map::new_map_rooms_and_corridors();

    let (player_x, player_y) = map.rooms[0].center();
    let player_entity = spawner::player(&mut gs.ecs, player_x, player_y);

    gs.ecs.insert(RandomNumberGenerator::new());
    for room in map.rooms.iter().skip(1) {
        spawner::spawn_room(&mut gs.ecs, room);
    }

    gs.ecs.insert(RunState::PreRun);
    gs.ecs.insert(map);
    gs.ecs.insert(player_entity);
    gs.ecs.insert(Point::new(player_x, player_y));
    gs.ecs.insert(gamelog::GameLog {
        entries: vec!["Welcome to Rusty Roguelike".to_string()],
    });

    main_loop(context, gs)
}
