use crate::ai::PlayerAI;
use crate::element::{OptionalReadableElement, ReadableHeaderElement, WritableHeaderElement};
use crate::reader::RecordingHeaderReader;
use crate::unit::Unit;
use crate::unit_type::CompactUnitType;
use crate::GameVariant::DefinitiveEdition;
use crate::{Error, ObjectID, PlayerID, Result};
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use genie_dat::tech_tree::TechTree;
use genie_dat::CivilizationID;
use genie_scx::VictoryConditions;
use genie_support::ReadSkipExt;
use genie_support::{read_opt_u32, ReadStringsExt};
use std::convert::TryInto;
use std::io::{Read, Write};

// TODO! reset to warn before merging
#[allow(unused_macro_rules)]
macro_rules! assert_marker {
    ($val:expr, $marker:expr) => {
        let found = $val.read_u8()?;
        if found != $marker {
            let mut skipped: u64 = 1;
            while $val.read_u8()? != $marker {
                skipped += 1;
            }

            return Err(Error::HeaderError(
                $val.position() as u64 - skipped,
                Box::new(Error::MissingMarker(
                    $val.version(),
                    $marker as u128,
                    found as u128,
                    file!(),
                    line!(),
                    skipped,
                )),
            ));
        }
    };

    ($found:ident eq $marker:expr, $version:expr) => {
        if $found != $marker {
            return Err(Error::MissingMarker(
                $version,
                $marker as u128,
                $found as u128,
                file!(),
                line!(),
                0,
            ));
        }
    };
}

#[derive(Debug, Default, Clone)]
pub struct Player {
    player_type: u8,
    relations: Vec<u8>,
    diplomacy: [u32; 9],
    allied_los: bool,
    allied_victory: bool,
    name: String,
    pub attributes: Vec<f32>,
    initial_view: (f32, f32),
    saved_views: Vec<(f32, f32)>,
    spawn_location: (u16, u16),
    culture_id: u8,
    pub civilization_id: CivilizationID,
    game_status: u8,
    resigned: bool,
    pub userpatch_data: Option<UserPatchData>,
    pub tech_state: PlayerTech,
    pub history_info: HistoryInfo,
    pub tech_tree: Option<TechTree>,
    pub gaia: Option<GaiaData>,
    pub unit_types: Vec<Option<CompactUnitType>>,
    pub visible_map: VisibleMap,
    pub visible_resources: VisibleResources,
    pub units: Vec<Unit>,
    pub sleeping_units: Vec<Unit>,
    pub doppelganger_units: Vec<Unit>,
    pub victory: VictoryConditions,
}

impl Player {
    /// Return the name of this player.
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn read_info<R: Read>(&mut self, input: &mut RecordingHeaderReader<R>) -> Result<()> {
        self.victory = VictoryConditions::read_from(input, true)?;
        Ok(())
    }
}

impl ReadableHeaderElement for Player {
    #[allow(clippy::cognitive_complexity)]
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        let mut player = Self::default();

        player.player_type = input.read_u8()?;
        if input.version() >= 10.55 {
            assert_marker!(input, 11);
        }
        player.relations = vec![0; input.num_players() as usize];
        input.read_exact(&mut player.relations)?;
        input.read_u32_into::<LE>(&mut player.diplomacy)?;
        player.allied_los = input.read_u32::<LE>()? != 0;
        player.allied_victory = input.read_u8()? != 0;
        player.name = input
            .read_u16_length_prefixed_str()?
            .unwrap_or_else(String::new);

        if input.version() >= 10.55 {
            assert_marker!(input, 22);
        }
        let num_attributes = input.read_u32::<LE>()?;
        if input.version() >= 10.55 {
            assert_marker!(input, 33);
        }
        player.attributes = vec![0.0; num_attributes.try_into().unwrap()];
        input.read_f32_into::<LE>(&mut player.attributes)?;

        if input.variant() >= DefinitiveEdition {
            input.skip((num_attributes * 4) as u64)?;
        }

        if input.version() >= 10.55 {
            assert_marker!(input, 11);
        }

        player.initial_view = (input.read_f32::<LE>()?, input.read_f32::<LE>()?);
        if input.version() >= 11.62 {
            let num_saved_views = input.read_i32::<LE>()?;
            // saved view count can be negative
            player.saved_views = vec![(0.0, 0.0); num_saved_views.try_into().unwrap_or(0)];
            for sv in player.saved_views.iter_mut() {
                *sv = (input.read_f32::<LE>()?, input.read_f32::<LE>()?);
            }
        }
        player.spawn_location = (input.read_u16::<LE>()?, input.read_u16::<LE>()?);
        player.culture_id = input.read_u8()?;
        player.civilization_id = input.read_u8()?.into();
        player.game_status = input.read_u8()?;
        player.resigned = input.read_u8()? != 0;
        if input.version() >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }
        let _color = input.read_u8()?;
        if input.version() >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }
        let _pathing_attempt_cap = input.read_u32::<LE>()?;
        let _pathing_delay_cap = input.read_u32::<LE>()?;

        // Unit counts
        let counts = if input.version() >= 11.65 {
            (900, 100, 900, 100)
        } else if input.version() >= 11.51 {
            (850, 100, 850, 100)
        } else {
            (750, 100, 750, 100)
        };

        let mut object_categories_count = vec![0; counts.0];
        input.read_u16_into::<LE>(&mut object_categories_count)?;
        let mut object_groups_count = vec![0; counts.1];
        input.read_u16_into::<LE>(&mut object_groups_count)?;

        let mut built_object_categories_count = vec![0; counts.2];
        input.read_u16_into::<LE>(&mut built_object_categories_count)?;
        let mut built_object_groups_count = vec![0; counts.3];
        input.read_u16_into::<LE>(&mut built_object_groups_count)?;

        let _total_units_count = input.read_u16::<LE>()?;
        let _total_buildings_count = input.read_u16::<LE>()?;
        let _built_units_count = input.read_u16::<LE>()?;
        let _built_buildings_count = input.read_u16::<LE>()?;

        // formations
        let _line_ratio = input.read_u32::<LE>()?;
        let _column_ratio = input.read_u32::<LE>()?;
        let _min_column_distance = input.read_u32::<LE>()?;
        let _column_to_line_distance = input.read_u32::<LE>()?;
        let _auto_formations = input.read_u32::<LE>()?;
        let _formations_influence_distance = input.read_f32::<LE>()?;
        let _break_auto_formations_by_speed = if input.version() >= 10.81 {
            input.read_f32::<LE>()?
        } else {
            0.0
        };

        // escrow
        let _pending_debits = (
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
        );
        let _escrow_amounts = (
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
        );
        let _escrow_percents = (
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
        );

        // view scrolling
        if input.version() >= 10.51 {
            let _scroll_vector = (input.read_f32::<LE>()?, input.read_f32::<LE>()?);
            let _scroll_end = (input.read_f32::<LE>()?, input.read_f32::<LE>()?);
            let _scroll_start = (input.read_f32::<LE>()?, input.read_f32::<LE>()?);
            let _scroll_total_distance = input.read_f32::<LE>()?;
            let _scroll_distance = input.read_f32::<LE>()?;
        }

        // AI state
        if input.version() >= 11.45 {
            let _easiest_reaction_percent = input.read_f32::<LE>()?;
            let _easier_reaction_percent = input.read_f32::<LE>()?;
            let _task_ungrouped_soldiers = input.read_u8()? != 0;
        }

        // selected units
        if input.version() >= 11.72 {
            let num_selections = input.read_u32::<LE>()?;
            let _selection = if num_selections > 0 {
                let object_id: ObjectID = input.read_u32::<LE>()?.into();
                let object_properties = input.read_u32::<LE>()?;
                let mut selected_ids = vec![ObjectID(0); num_selections.try_into().unwrap()];
                for id in selected_ids.iter_mut() {
                    *id = input.read_u32::<LE>()?.into();
                }
                Some((object_id, object_properties, selected_ids))
            } else {
                None
            };
        }

        if input.variant() == DefinitiveEdition {
            let mut pre_skipped = 32435;
            if input.version() >= 13.0 {
                pre_skipped += 8;
            }

            if input.version() >= 13.07 {
                pre_skipped += 1;
            }

            if input.version() >= 13.13 {
                pre_skipped += 5;
            }

            if input.version() >= 13.34 {
                pre_skipped += 4;
            }

            input.skip(pre_skipped)?;
            // I am crying
            loop {
                let first = input.read_u8()?;
                if first != 11 {
                    continue;
                }
                let second = input.read_u8()?;
                if second != 11 {
                    continue;
                }

                break;
            }
        } else if input.version() >= 10.55 {
            assert_marker!(input, 11);
            assert_marker!(input, 11);
        }

        let _ty = input.read_u8()?;

        let _update_count = input.read_u32::<LE>()?;
        let _update_count_need_help = input.read_u32::<LE>()?;

        // ai attack data
        if input.version() >= 10.02 {
            let _alerted_enemy_count = input.read_u32::<LE>()?;
            let _regular_attack_count = input.read_u32::<LE>()?;
            let _regular_attack_mode = input.read_u8()?;
            let _regular_attack_location = (input.read_f32::<LE>()?, input.read_f32::<LE>()?);
            let _town_attack_count = input.read_u32::<LE>()?;
            let _town_attack_mode = input.read_u8()?;
            let _town_attack_location = (input.read_f32::<LE>()?, input.read_f32::<LE>()?);
        }

        let _fog_update = input.read_u32::<LE>()?;
        let _update_time = input.read_f32::<LE>()?;
        // if is userpatch
        if genie_support::f32_eq!(input.version(), 11.97) {
            player.userpatch_data = Some(UserPatchData::read_from(input)?);
        }

        player.tech_state = PlayerTech::read_from(input)?;

        let _update_history_count = input.read_u32::<LE>()?;

        assert_marker!(input, 11);

        player.history_info = HistoryInfo::read_from(input)?;

        if input.version() >= 5.30 {
            let _ruin_held_time = input.read_u32::<LE>()?;
            let _artifact_held_time = input.read_u32::<LE>()?;
        }

        if input.version() >= 10.55 {
            assert_marker!(input, 11);
        }

        // diplomacy
        if input.version() >= 9.13 {
            let mut diplomacy = [0; 9];
            let mut intelligence = [0; 9];
            let mut trade = [0; 9];
            let mut offer = vec![];
            for i in 0..9 {
                diplomacy[i] = input.read_u8()?;
                intelligence[i] = input.read_u8()?;
                trade[i] = input.read_u8()?;

                offer.push(DiplomacyOffer::read_from(input)?);
            }
            let _fealty = input.read_u16::<LE>()?;
        }

        if input.version() >= 10.55 {
            assert_marker!(input, 11);
        }

        // off-map trade
        if input.version() >= 9.17 {
            let mut off_map_trade_route_explored = [0; 20];
            input.read_exact(&mut off_map_trade_route_explored)?;
        }

        if input.version() >= 9.18 {
            let mut off_map_trade_route_being_explored = [0; 20];
            input.read_exact(&mut off_map_trade_route_being_explored)?;
        }

        if input.version() >= 10.55 {
            assert_marker!(input, 11);
        }

        // market trading
        if input.version() >= 9.22 {
            let _max_trade_amount = input.read_u32::<LE>()?;
            let _old_max_trade_amount = input.read_u32::<LE>()?;
            let _max_trade_limit = input.read_u32::<LE>()?;
            let _current_wood_limit = input.read_u32::<LE>()?;
            let _current_food_limit = input.read_u32::<LE>()?;
            let _current_stone_limit = input.read_u32::<LE>()?;
            let _current_ore_limit = input.read_u32::<LE>()?;
            let _commodity_volume_delta = input.read_i32::<LE>()?;
            let _trade_vig_rate = input.read_f32::<LE>()?;
            let _trade_refresh_timer = input.read_u32::<LE>()?;
            let _trade_refresh_rate = input.read_u32::<LE>()?;
        }

        let _prod_queue_enabled = if input.version() >= 9.67 {
            input.read_u8()? != 0
        } else {
            true
        };

        // ai dodging ability
        if input.version() >= 9.90 {
            let _chance_to_dodge_missiles = input.read_u8()?;
            let _chance_for_archers_to_maintain_distance = input.read_u8()?;
        }

        let _open_gates_for_pathing_count = if input.version() >= 11.42 {
            input.read_u32::<LE>()?
        } else {
            0
        };
        let _farm_queue_count = if input.version() >= 11.57 {
            input.read_u32::<LE>()?
        } else {
            0
        };
        let _nomad_build_lock = if input.version() >= 11.75 {
            input.read_u32::<LE>()? != 0
        } else {
            false
        };

        if input.version() >= 9.30 {
            let _old_kills = input.read_u32::<LE>()?;
            let _old_razings = input.read_u32::<LE>()?;
            let _battle_mode = input.read_u32::<LE>()?;
            let _razings_mode = input.read_u32::<LE>()?;
            let _total_kills = input.read_u32::<LE>()?;
            let _total_razings = input.read_u32::<LE>()?;
        }

        if input.version() >= 9.31 {
            let _old_hit_points = input.read_u32::<LE>()?;
            let _total_hit_points = input.read_u32::<LE>()?;
        }

        if input.version() >= 9.32 {
            let mut old_player_kills = [0; 9];
            input.read_u32_into::<LE>(&mut old_player_kills)?;
        }

        if input.variant() >= DefinitiveEdition {
            input.skip(11)?;
        }

        player.tech_tree = if input.version() >= 9.38 {
            Some(TechTree::read_from(&mut *input)?)
        } else {
            None
        };

        if input.variant() >= DefinitiveEdition {
            // Just, random 4 zero bytes :)
            input.skip(4)?;

            if player.player_type != 2 {
                input.skip(4)?;
            }
        }

        if input.version() >= 10.55 {
            assert_marker!(input, 11);
        }

        let _player_ai = if player.player_type == 3 && input.read_u32::<LE>()? == 1 {
            Some(PlayerAI::read_from(input)?)
        } else {
            None
        };

        if input.version() >= 10.55 {
            assert_marker!(input, 11);
        }

        player.gaia = if player.player_type == 2 {
            Some(GaiaData::read_from(input)?)
        } else {
            None
        };

        if input.version() >= 10.55 {
            assert_marker!(input, 11);
        }

        let num_unit_types = input.read_u32::<LE>()?;
        let mut available_unit_types = vec![false; num_unit_types.try_into().unwrap()];
        for available in available_unit_types.iter_mut() {
            *available = input.read_u32::<LE>()? != 0;
        }

        if input.version() >= 10.55 {
            assert_marker!(input, 11);
        }

        player.unit_types.reserve(available_unit_types.len());
        for available in available_unit_types {
            player.unit_types.push(if !available {
                None
            } else {
                if input.version() >= 10.55 {
                    assert_marker!(input, 22);
                }
                let ty = CompactUnitType::read_from(input)?;
                if input.version() >= 10.55 {
                    assert_marker!(input, 33);
                }
                Some(ty)
            });
        }

        if input.version() >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }

        player.visible_map = VisibleMap::read_from(input)?;
        player.visible_resources = VisibleResources::read_from(input)?;

        if input.version() >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }

        player.units = {
            let _list_size = input.read_u32::<LE>()?;
            let _grow_size = input.read_u32::<LE>()?;
            let mut units = vec![];
            while let Some(unit) = Unit::read_from(input)? {
                units.push(unit);
            }
            units
        };

        if input.version() >= 10.55 {
            assert_marker!(input, 11);
        }

        player.sleeping_units = {
            let _list_size = input.read_u32::<LE>()?;
            let _grow_size = input.read_u32::<LE>()?;
            let mut units = vec![];
            while let Some(unit) = Unit::read_from(input)? {
                units.push(unit);
            }
            units
        };

        if input.version() >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }

        player.doppelganger_units = {
            let _list_size = input.read_u32::<LE>()?;
            let _grow_size = input.read_u32::<LE>()?;
            let mut units = vec![];
            while let Some(unit) = Unit::read_from(input)? {
                units.push(unit);
            }
            units
        };

        if input.version() >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }

        Ok(player)
    }
}

#[derive(Debug, Default, Clone)]
pub struct VisibleMap {
    pub width: u32,
    pub height: u32,
    pub explored_tiles_count: u32,
    pub player_id: PlayerID,
    pub tiles: Vec<i16>,
}

impl ReadableHeaderElement for VisibleMap {
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        let mut map = Self::default();
        map.width = input.read_u32::<LE>()?;
        map.height = input.read_u32::<LE>()?;
        if input.version() >= 6.70 {
            map.explored_tiles_count = input.read_u32::<LE>()?;
        }
        map.player_id = input.read_u16::<LE>()?.try_into().unwrap();
        if input.variant() >= DefinitiveEdition {
            map.tiles = vec![0; (map.width * map.height) as usize];
            input.read_i16_into::<LE>(&mut map.tiles)?;
        } else {
            let mut tiles = vec![0i8; (map.width * map.height) as usize];
            input.read_i8_into(&mut tiles)?;
            map.tiles.extend(tiles.iter().map(|x| *x as i16));
        }
        Ok(map)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct VisibleResource {
    pub object_id: ObjectID,
    pub distance: u8,
    pub zone: i8,
    pub location: (u8, u8),
}

impl ReadableHeaderElement for VisibleResource {
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        let mut vis = Self::default();
        vis.object_id = input.read_u32::<LE>()?.into();
        vis.distance = input.read_u8()?;
        vis.zone = input.read_i8()?;
        vis.location = (input.read_u8()?, input.read_u8()?);
        Ok(vis)
    }
}

#[derive(Debug, Default, Clone)]
pub struct VisibleResources {
    lists: Vec<Vec<VisibleResource>>,
}

impl ReadableHeaderElement for VisibleResources {
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        let num_lists = input.read_u32::<LE>()?;
        let mut sizes = vec![];
        for _ in 0..num_lists {
            let _capacity = input.read_u32::<LE>()?;
            sizes.push(input.read_u32::<LE>()?);
        }
        let mut lists = Vec::with_capacity(sizes.len());
        for size in sizes {
            let mut list = Vec::with_capacity(size.try_into().unwrap());
            for _ in 0..size {
                list.push(VisibleResource::read_from(input)?);
            }
            lists.push(list);
        }
        Ok(Self { lists })
    }
}

#[derive(Debug, Default, Clone)]
pub struct GaiaData {
    update_time: u32,
    update_nature: u32,
    creatures: [GaiaCreature; 5],
    next_wolf_attack_update_time: u32,
    wolf_attack_update_interval: u32,
    wolf_attack_stop_time: u32,
    min_villager_distance: f32,
    tc_positions: [(f32, f32); 9],
    wolf_current_player: u32,
    wolf_current_villagers: [u32; 10],
    wolf_current_villager: Option<ObjectID>,
    wolf_villager_count: u32,
    wolves: [GaiaWolfInfo; 25],
    current_wolf: Option<ObjectID>,
    wolf_counts: [u32; 10],
}

impl ReadableHeaderElement for GaiaData {
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        let mut gaia = Self::default();
        gaia.update_time = input.read_u32::<LE>()?;
        gaia.update_nature = input.read_u32::<LE>()?;
        for creature in gaia.creatures.iter_mut() {
            *creature = GaiaCreature::read_from(input)?;
        }
        gaia.next_wolf_attack_update_time = input.read_u32::<LE>()?;
        gaia.wolf_attack_update_interval = input.read_u32::<LE>()?;
        gaia.wolf_attack_stop_time = input.read_u32::<LE>()?;
        gaia.min_villager_distance = input.read_f32::<LE>()?;
        for pos in gaia.tc_positions.iter_mut() {
            pos.0 = input.read_f32::<LE>()?;
        }
        for pos in gaia.tc_positions.iter_mut() {
            pos.1 = input.read_f32::<LE>()?;
        }
        gaia.wolf_current_player = input.read_u32::<LE>()?;
        for v in gaia.wolf_current_villagers.iter_mut() {
            *v = input.read_u32::<LE>()?;
        }
        gaia.wolf_current_villager = read_opt_u32(input)?;
        gaia.wolf_villager_count = input.read_u32::<LE>()?;
        for wolf in gaia.wolves.iter_mut() {
            *wolf = GaiaWolfInfo::read_from(input)?;
        }
        gaia.current_wolf = read_opt_u32(input)?;
        input.read_u32_into::<LE>(&mut gaia.wolf_counts[..])?;
        Ok(gaia)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GaiaCreature {
    pub growth_rate: f32,
    pub remainder: f32,
    pub max: u32,
}

impl ReadableHeaderElement for GaiaCreature {
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        let mut creature = Self::default();
        creature.growth_rate = input.read_f32::<LE>()?;
        creature.remainder = input.read_f32::<LE>()?;
        creature.max = input.read_u32::<LE>()?;
        Ok(creature)
    }
}

impl WritableHeaderElement for GaiaCreature {
    fn write_to<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_f32::<LE>(self.growth_rate)?;
        output.write_f32::<LE>(self.remainder)?;
        output.write_u32::<LE>(self.max)?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GaiaWolfInfo {
    pub id: u32,
    pub distance: f32,
}

impl ReadableHeaderElement for GaiaWolfInfo {
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        let mut wolf = Self::default();
        wolf.id = input.read_u32::<LE>()?;
        wolf.distance = input.read_f32::<LE>()?;
        Ok(wolf)
    }
}

impl WritableHeaderElement for GaiaWolfInfo {
    fn write_to<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_u32::<LE>(self.id)?;
        output.write_f32::<LE>(self.distance)?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
struct DiplomacyOffer {
    sequence: u8,
    started_by: u8,
    actual_time: u32,
    game_time: u32,
    declare: u8,
    old_diplomacy: u8,
    new_diplomacy: u8,
    old_intelligence: u8,
    new_intelligence: u8,
    old_trade: u8,
    new_trade: u8,
    demand: u8,
    gold: u32,
    message: Option<String>,
    status: u8,
}

impl ReadableHeaderElement for DiplomacyOffer {
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        let mut offer = Self::default();
        offer.sequence = input.read_u8()?;
        offer.started_by = input.read_u8()?;
        offer.actual_time = 0;
        offer.game_time = input.read_u32::<LE>()?;
        offer.declare = input.read_u8()?;
        offer.old_diplomacy = input.read_u8()?;
        offer.new_diplomacy = input.read_u8()?;
        offer.old_intelligence = input.read_u8()?;
        offer.new_intelligence = input.read_u8()?;
        offer.old_trade = input.read_u8()?;
        offer.new_trade = input.read_u8()?;
        offer.demand = input.read_u8()?;
        offer.gold = input.read_u32::<LE>()?;
        let message_len = input.read_u8()?;
        offer.message = input.read_str(usize::from(message_len))?;
        offer.status = input.read_u8()?;
        Ok(offer)
    }
}

#[derive(Debug, Default, Clone)]
pub struct HistoryInfo {
    pub entries: Vec<HistoryEntry>,
    pub events: Vec<HistoryEvent>,
}

impl ReadableHeaderElement for HistoryInfo {
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        let num_entries = input.read_u32::<LE>()?;
        let _num_events = input.read_u32::<LE>()?;
        let entries_capacity = input.read_u32::<LE>()?;
        let mut entries = Vec::with_capacity(entries_capacity.try_into().unwrap());
        for _ in 0..num_entries {
            entries.push(HistoryEntry::read_from(input)?);
        }

        // 22 for gaia, but 0 for players?
        let _value = input.read_u8()?;
        let num_events = input.read_u32::<LE>()?;
        let mut events = Vec::with_capacity(num_events.try_into().unwrap());
        for _ in 0..num_events {
            events.push(HistoryEvent::read_from(input)?);
        }

        let _razings = input.read_i32::<LE>()?;
        let _hit_points_razed = input.read_i32::<LE>()?;
        let _razed_by_others = input.read_i32::<LE>()?;
        let _hit_points_razed_by_others = input.read_i32::<LE>()?;
        let _kills = input.read_i32::<LE>()?;
        let _hit_points_killed = input.read_i32::<LE>()?;
        let _killed_by_others = input.read_i32::<LE>()?;
        let _hit_points_killed_by_others = input.read_i32::<LE>()?;
        let _razings_weight = input.read_i32::<LE>()?;
        let _kills_weight = input.read_i32::<LE>()?;
        let _razings_percent = input.read_i32::<LE>()?;
        let _kills_percent = input.read_i32::<LE>()?;
        let _razing_mode = input.read_i32::<LE>()?;
        let _battle_mode = input.read_i32::<LE>()?;
        let _update_count = input.read_i32::<LE>()?;
        let _old_current_units_created = input.read_i32::<LE>()?;
        let _old_current_buildings_built = input.read_i32::<LE>()?;
        let mut old_kills = [0; 8];
        input.read_u16_into::<LE>(&mut old_kills[..])?;
        let mut old_kill_bvs = [0; 8];
        input.read_u32_into::<LE>(&mut old_kill_bvs[..])?;
        let mut old_razings = [0; 8];
        input.read_u16_into::<LE>(&mut old_razings[..])?;
        let mut old_razing_bvs = [0; 8];
        input.read_u32_into::<LE>(&mut old_razing_bvs[..])?;
        let _running_average_bv_percent = input.read_i32::<LE>()?;
        let _running_total_bv_kills = input.read_i32::<LE>()?;
        let _running_total_bv_razings = input.read_i32::<LE>()?;
        let _running_total_kills = input.read_i16::<LE>()?;
        let _running_total_razings = input.read_i16::<LE>()?;

        let _padding = input.read_u8()?;

        Ok(Self { entries, events })
    }
}

#[derive(Debug, Default, Clone)]
pub struct HistoryEvent {
    pub event_type: i8,
    pub time_slice: u32,
    pub world_time: u32,
    pub params: (f32, f32, f32),
}

impl ReadableHeaderElement for HistoryEvent {
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        let mut event = Self::default();
        event.event_type = input.read_i8()?;
        event.time_slice = input.read_u32::<LE>()?;
        event.world_time = input.read_u32::<LE>()?;
        event.params = (
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
        );

        Ok(event)
    }
}

#[derive(Debug, Default, Clone)]
pub struct HistoryEntry {
    pub civilian_population: u16,
    pub military_population: u16,
}

impl ReadableHeaderElement for HistoryEntry {
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        let civilian_population = input.read_u16::<LE>()?;
        let military_population = input.read_u16::<LE>()?;

        Ok(HistoryEntry {
            civilian_population,
            military_population,
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct TechState {
    pub progress: f32,
    pub state: i16,
    pub modifiers: (i16, i16, i16),
    pub time_modifier: i16,
}

impl ReadableHeaderElement for TechState {
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        let mut state = Self::default();
        state.progress = input.read_f32::<LE>()?;
        state.state = input.read_i16::<LE>()?;
        state.modifiers = (
            input.read_i16::<LE>()?,
            input.read_i16::<LE>()?,
            input.read_i16::<LE>()?,
        );
        state.time_modifier = input.read_i16::<LE>()?;

        if input.variant() >= DefinitiveEdition {
            input.skip(15)?;
        }

        Ok(state)
    }
}

#[derive(Debug, Default, Clone)]
pub struct PlayerTech {
    pub tech_states: Vec<TechState>,
}

impl ReadableHeaderElement for PlayerTech {
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        let num_techs = input.read_u16::<LE>()?;

        let mut tech_states = Vec::with_capacity(usize::from(num_techs));
        for _ in 0..num_techs {
            tech_states.push(TechState::read_from(input)?);
        }
        Ok(Self { tech_states })
    }
}

#[derive(Debug, Clone)]
pub struct UserPatchData {}

impl ReadableHeaderElement for UserPatchData {
    fn read_from<R: Read>(input: &mut RecordingHeaderReader<R>) -> Result<Self> {
        {
            let mut bytes = vec![0; 4080];
            input.read_exact(&mut bytes)?;
        }

        let mut category_priorities = vec![0; 900];
        let mut group_priorities = vec![0; 100];

        input.read_u16_into::<LE>(&mut category_priorities)?;
        input.read_u16_into::<LE>(&mut group_priorities)?;

        {
            let mut bytes = vec![0; 2096];
            input.read_exact(&mut bytes)?;
        }

        Ok(Self {})
    }
}
