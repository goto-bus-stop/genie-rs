use crate::{ObjectID, Result};
use crate::unit_type::CompactUnitType;
use std::convert::TryInto;
use std::io::{Read, Write};
use genie_dat::{CivilizationID, TechTree};
use byteorder::{LE, ReadBytesExt, WriteBytesExt};

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
}

impl Player {
    pub fn read_from(mut input: impl Read, version: f32, num_players: u8) -> Result<Self> {
        let mut player = Self::default();

        player.player_type = input.read_u8()?;
        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }
        player.relations = vec![0; usize::from(num_players)];
        for r in player.relations.iter_mut() {
            *r = input.read_u8()?;
        }
        for r in player.diplomacy.iter_mut() {
            *r = input.read_u32::<LE>()?;
        }
        player.allied_los = input.read_u32::<LE>()? != 0;
        player.allied_victory = input.read_u8()? != 0;
        let name_len = input.read_u16::<LE>()?;
        player.name = genie_support::read_str(&mut input, usize::from(name_len))?.unwrap_or_else(String::new);
        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 22);
        }
        let num_attributes = input.read_u32::<LE>()?;
        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 33);
        }
        player.attributes = vec![0.0; num_attributes.try_into().unwrap()];
        for v in player.attributes.iter_mut() {
            *v = input.read_f32::<LE>()?;
        }
        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }
        player.initial_view = (input.read_f32::<LE>()?, input.read_f32::<LE>()?);
        let num_saved_views = input.read_i32::<LE>()?;
        // saved view count can be negative
        player.saved_views = vec![(0.0, 0.0); num_saved_views.try_into().unwrap_or(0)];
        for sv in player.saved_views.iter_mut() {
            *sv = (input.read_f32::<LE>()?, input.read_f32::<LE>()?);
        }
        player.spawn_location = (input.read_u16::<LE>()?, input.read_u16::<LE>()?);
        player.culture_id = input.read_u8()?;
        player.civilization_id = input.read_u8()?.into();
        player.game_status = input.read_u8()?;
        player.resigned = input.read_u8()? != 0;
        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }
        let color = input.read_u8()?;
        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }
        let pathing_attempt_cap = input.read_u32::<LE>()?;
        let pathing_delay_cap = input.read_u32::<LE>()?;

        // Unit counts
        let counts = if version >= 11.65 {
            (900, 100, 900, 100)
        } else if version >= 11.51 {
            (850, 100, 850, 100)
        } else {
            (750, 100, 750, 100)
        };
        let mut object_categories_count = vec![0; counts.0];
        for count in object_categories_count.iter_mut() {
            *count = input.read_u16::<LE>()?;
        }
        let mut object_groups_count = vec![0; counts.1];
        for count in object_groups_count.iter_mut() {
            *count = input.read_u16::<LE>()?;
        }

        let mut built_object_categories_count = vec![0; counts.2];
        for count in built_object_categories_count.iter_mut() {
            *count = input.read_u16::<LE>()?;
        }
        let mut built_object_groups_count = vec![0; counts.3];
        for count in built_object_groups_count.iter_mut() {
            *count = input.read_u16::<LE>()?;
        }

        let total_units_count = input.read_u16::<LE>()?;
        let total_buildings_count = input.read_u16::<LE>()?;
        let built_units_count = input.read_u16::<LE>()?;
        let built_buildings_count = input.read_u16::<LE>()?;

        // formations
        let line_ratio = input.read_u32::<LE>()?;
        let column_ratio = input.read_u32::<LE>()?;
        let min_column_distance = input.read_u32::<LE>()?;
        let column_to_line_distance = input.read_u32::<LE>()?;
        let auto_formations = input.read_u32::<LE>()?;
        let formations_influence_distance = input.read_f32::<LE>()?;
        let break_auto_formations_by_speed = if version >= 10.81 {
            input.read_f32::<LE>()?
        } else {
            0.0
        };

        // escrow
        let pending_debits = (
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
        );
        let escrow_amounts = (
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
        );
        let escrow_percents = (
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
            input.read_f32::<LE>()?,
        );

        // view scrolling
        if version >= 10.51 {
            let scroll_vector = (input.read_f32::<LE>()?, input.read_f32::<LE>()?);
            let scroll_end = (input.read_f32::<LE>()?, input.read_f32::<LE>()?);
            let scroll_start = (input.read_f32::<LE>()?, input.read_f32::<LE>()?);
            let scroll_total_distance = input.read_f32::<LE>()?;
            let scroll_distance = input.read_f32::<LE>()?;
        }

        // AI state
        if version >= 11.45 {
            let easiest_reaction_percent = input.read_f32::<LE>()?;
            let easier_reaction_percent = input.read_f32::<LE>()?;
            let task_ungrouped_soldiers = input.read_u8()? != 0;
        }

        // selected units
        if version >= 11.72 {
            let num_selections = input.read_u32::<LE>()?;
            let selection = if num_selections > 0 {
                let object_id: ObjectID = input.read_u32::<LE>()?.into();
                let object_properties = input.read_u32::<LE>()?;
                let mut selected_ids = vec![ObjectID(0); num_selections.try_into().unwrap()];
                for id in selected_ids.iter_mut() {
                    *id = input.read_u32::<LE>()?.into();
                }
                Some((
                    object_id,
                    object_properties,
                    selected_ids,
                ))
            } else {
                None
            };
        }

        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
            assert_eq!(input.read_u8()?, 11);
        }

        let ty = input.read_u8()?;
        let update_count = input.read_u32::<LE>()?;
        let update_count_need_help = input.read_u32::<LE>()?;

        // ai attack data
        if version >= 10.02 {
            let alerted_enemy_count = input.read_u32::<LE>()?;
            let regular_attack_count = input.read_u32::<LE>()?;
            let regular_attack_mode = input.read_u8()?;
            let regular_attack_location = (
                input.read_f32::<LE>()?,
                input.read_f32::<LE>()?,
            );
            let town_attack_count = input.read_u32::<LE>()?;
            let town_attack_mode = input.read_u8()?;
            let town_attack_location = (
                input.read_f32::<LE>()?,
                input.read_f32::<LE>()?,
            );
        }

        let fog_update = input.read_u32::<LE>()?;
        let update_time = input.read_f32::<LE>()?;

        // if is userpatch
        player.userpatch_data = Some(UserPatchData::read_from(&mut input)?);

        player.tech_state = PlayerTech::read_from(&mut input)?;

        let update_history_count = input.read_u32::<LE>()?;
        player.history_info = HistoryInfo::read_from(&mut input, version)?;

        if version >= 5.30 {
            let ruin_held_time = input.read_u32::<LE>()?;
            let artifact_held_time = input.read_u32::<LE>()?;
        }

        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }

        // diplomacy
        if version >= 9.13 {
            let mut diplomacy = [0; 9];
            let mut intelligence = [0; 9];
            let mut trade = [0; 9];
            let mut offer = vec![];
            for i in 0..9 {
                diplomacy[i] = input.read_u8()?;
                intelligence[i] = input.read_u8()?;
                trade[i] = input.read_u8()?;

                offer.push(DiplomacyOffer::read_from(&mut input)?);
            }
            let fealty = input.read_u16::<LE>()?;
        }

        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }

        // off-map trade
        if version >= 9.17 {
            let mut off_map_trade_route_explored = [0; 20];
            for v in off_map_trade_route_explored.iter_mut() {
                *v = input.read_u8()?;
            }
        }

        if version >= 9.18 {
            let mut off_map_trade_route_being_explored = [0; 20];
            for v in off_map_trade_route_being_explored.iter_mut() {
                *v = input.read_u8()?;
            }
        }

        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }

        // market trading
        if version >= 9.22 {
            let max_trade_amount = input.read_u32::<LE>()?;
            let old_max_trade_amount = input.read_u32::<LE>()?;
            let max_trade_limit = input.read_u32::<LE>()?;
            let current_wood_limit = input.read_u32::<LE>()?;
            let current_food_limit = input.read_u32::<LE>()?;
            let current_stone_limit = input.read_u32::<LE>()?;
            let current_ore_limit = input.read_u32::<LE>()?;
            let commodity_volume_delta = input.read_i32::<LE>()?;
            let trade_vig_rate = input.read_f32::<LE>()?;
            let trade_refresh_timer = input.read_u32::<LE>()?;
            let trade_refresh_rate = input.read_u32::<LE>()?;
        }

        let prod_queue_enabled = if version >= 9.67 {
            input.read_u8()? != 0
        } else {
            true
        };

        // ai dodging ability
        if version >= 9.90 {
            let chance_to_dodge_missiles = input.read_u8()?;
            let chance_for_archers_to_maintain_distance = input.read_u8()?;
        }

        let open_gates_for_pathing_count = if version >= 11.42 {
            input.read_u32::<LE>()?
        } else {
            0
        };
        let farm_queue_count = if version >= 11.57 {
            input.read_u32::<LE>()?
        } else {
            0
        };
        let nomad_build_lock = if version >= 11.75 {
            input.read_u32::<LE>()? != 0
        } else {
            false
        };

        if version >= 9.30 {
            let old_kills = input.read_u32::<LE>()?;
            let old_razings = input.read_u32::<LE>()?;
            let battle_mode = input.read_u32::<LE>()?;
            let razings_mode = input.read_u32::<LE>()?;
            let total_kills = input.read_u32::<LE>()?;
            let total_razings = input.read_u32::<LE>()?;
        }

        if version >= 9.31 {
            let old_hit_points = input.read_u32::<LE>()?;
            let total_hit_points = input.read_u32::<LE>()?;
        }

        if version >= 9.32 {
            let mut old_player_kills = [0; 9];
            for v in old_player_kills.iter_mut() {
                *v = input.read_u32::<LE>()?;
            }
        }

        player.tech_tree = if version >= 9.38 {
            Some(TechTree::read_from(&mut input)?)
        } else {
            None
        };

        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }

        let player_ai = if player.player_type == 3 && input.read_u32::<LE>()? == 1 {
            todo!();
            Some(0)
        } else {
            None
        };

        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }

        player.gaia = if player.player_type == 2 {
            Some(GaiaData::read_from(&mut input)?)
        } else {
            None
        };

        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }

        let num_object_types = input.read_u32::<LE>()?;
        let mut available_object_types = vec![false; num_object_types.try_into().unwrap()];
        for available in available_object_types.iter_mut() {
            *available = input.read_u32::<LE>()? != 0;
        }

        if version >= 10.55 {
            assert_eq!(input.read_u8()?, 11);
        }

        let mut object_types = Vec::with_capacity(available_object_types.len());
        for available in available_object_types {
            object_types.push(if !available {
                None
            } else {
                if version >= 10.55 {
                    assert_eq!(input.read_u8()?, 22);
                }
                let ty = CompactUnitType::read_from(&mut input, version)?;
                if version >= 10.55 {
                    assert_eq!(input.read_u8()?, 33);
                }
                Some(ty)
            });
        }

        Ok(player)
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

impl GaiaData {
    pub fn read_from(mut input: impl Read) -> Result<Self> {
        let mut gaia = Self::default();
        gaia.update_time = input.read_u32::<LE>()?;
        gaia.update_nature = input.read_u32::<LE>()?;
        for creature in gaia.creatures.iter_mut() {
            *creature = GaiaCreature::read_from(&mut input)?;
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
        gaia.wolf_current_villager = match input.read_i32::<LE>()? {
            -1 => None,
            id => Some(id.try_into().unwrap()),
        };
        gaia.wolf_villager_count = input.read_u32::<LE>()?;
        for wolf in gaia.wolves.iter_mut() {
            *wolf = GaiaWolfInfo::read_from(&mut input)?;
        }
        gaia.current_wolf = match input.read_i32::<LE>()? {
            -1 => None,
            id => Some(id.try_into().unwrap()),
        };
        for v in gaia.wolf_counts.iter_mut() {
            *v = input.read_u32::<LE>()?;
        }
        Ok(gaia)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GaiaCreature {
    pub growth_rate: f32,
    pub remainder: f32,
    pub max: u32,
}

impl GaiaCreature {
    pub fn read_from(mut input: impl Read) -> Result<Self> {
        let mut creature = Self::default();
        creature.growth_rate = input.read_f32::<LE>()?;
        creature.remainder = input.read_f32::<LE>()?;
        creature.max = input.read_u32::<LE>()?;
        Ok(creature)
    }

    pub fn write_to(&self, mut output: impl Write) -> Result<()> {
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

impl GaiaWolfInfo {
    pub fn read_from(mut input: impl Read) -> Result<Self> {
        let mut wolf = Self::default();
        wolf.id = input.read_u32::<LE>()?;
        wolf.distance = input.read_f32::<LE>()?;
        Ok(wolf)
    }

    pub fn write_to(&self, mut output: impl Write) -> Result<()> {
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

impl DiplomacyOffer {
    pub fn read_from(mut input: impl Read) -> Result<Self> {
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
        offer.message = genie_support::read_str(&mut input, usize::from(message_len))?;
        offer.status = input.read_u8()?;
        Ok(offer)
    }
}

#[derive(Debug, Default, Clone)]
pub struct HistoryInfo {
    entries: Vec<HistoryEntry>,
    events: Vec<HistoryEvent>,
}

impl HistoryInfo {
    pub fn read_from(mut input: impl Read, version: f32) -> Result<Self> {
        let _padding = input.read_u8()?;
        let num_entries = input.read_u32::<LE>()?;
        let _num_events = input.read_u32::<LE>()?;
        let entries_capacity = input.read_u32::<LE>()?;
        let mut entries = Vec::with_capacity(entries_capacity.try_into().unwrap());
        for _ in 0..num_entries {
            entries.push(HistoryEntry::read_from(&mut input, version)?);
        }

        let _padding = input.read_u8()?;

        let num_events = input.read_u32::<LE>()?;
        let mut events = Vec::with_capacity(num_events.try_into().unwrap());
        for _ in 0..num_events {
            events.push(HistoryEvent::read_from(&mut input)?);
        }

        let razings = input.read_i32::<LE>()?;
        let hit_points_razed = input.read_i32::<LE>()?;
        let razed_by_others = input.read_i32::<LE>()?;
        let hit_points_razed_by_others = input.read_i32::<LE>()?;
        let kills = input.read_i32::<LE>()?;
        let hit_points_killed = input.read_i32::<LE>()?;
        let killed_by_others = input.read_i32::<LE>()?;
        let hit_points_killed_by_others = input.read_i32::<LE>()?;
        let razings_weight = input.read_i32::<LE>()?;
        let kills_weight = input.read_i32::<LE>()?;
        let razings_percent = input.read_i32::<LE>()?;
        let kills_percent = input.read_i32::<LE>()?;
        let razing_mode = input.read_i32::<LE>()?;
        let battle_mode = input.read_i32::<LE>()?;
        let update_count = input.read_i32::<LE>()?;
        let old_current_units_created = input.read_i32::<LE>()?;
        let old_current_buildings_built = input.read_i32::<LE>()?;
        let mut old_kills = [0; 8];
        for v in old_kills.iter_mut() {
            *v = input.read_u16::<LE>()?;
        }
        let mut old_kill_bvs = [0; 8];
        for v in old_kill_bvs.iter_mut() {
            *v = input.read_u32::<LE>()?;
        }
        let mut old_razings = [0; 8];
        for v in old_razings.iter_mut() {
            *v = input.read_u16::<LE>()?;
        }
        let mut old_razing_bvs = [0; 8];
        for v in old_razing_bvs.iter_mut() {
            *v = input.read_u32::<LE>()?;
        }
        let running_average_bv_percent = input.read_i32::<LE>()?;
        let running_total_bv_kills = input.read_i32::<LE>()?;
        let running_total_bv_razings = input.read_i32::<LE>()?;
        let running_total_kills = input.read_i16::<LE>()?;
        let running_total_razings = input.read_i16::<LE>()?;

        let _padding = input.read_u8()?;

        Ok(Self {
            entries,
            events,
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct HistoryEvent {
    pub event_type: i8,
    pub time_slice: u32,
    pub world_time: u32,
    pub params: (f32, f32, f32),
}

impl HistoryEvent {
    pub fn read_from(mut input: impl Read) -> Result<Self> {
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

impl HistoryEntry {
    pub fn read_from(mut input: impl Read, version: f32) -> Result<Self> {
        let civilian_population = input.read_u16::<LE>()?;
        let military_population = input.read_u16::<LE>()?;
        Ok(HistoryEntry { civilian_population, military_population })
    }
}

#[derive(Debug, Default, Clone)]
pub struct TechState {
    pub progress: f32,
    pub state: i16,
    pub modifiers: (i16, i16, i16),
    pub time_modifier: i16,
}

impl TechState {
    pub fn read_from(mut input: impl Read) -> Result<Self> {
        let mut state = Self::default();
        state.progress = input.read_f32::<LE>()?;
        state.state = input.read_i16::<LE>()?;
        state.modifiers = (
            input.read_i16::<LE>()?,
            input.read_i16::<LE>()?,
            input.read_i16::<LE>()?,
        );
        state.time_modifier = input.read_i16::<LE>()?;
        Ok(state)
    }
}

#[derive(Debug, Default, Clone)]
pub struct PlayerTech {
    pub tech_states: Vec<TechState>,
}

impl PlayerTech {
    pub fn read_from(mut input: impl Read) -> Result<Self> {
        let num_techs = input.read_u16::<LE>()?;
        let mut tech_states = Vec::with_capacity(usize::from(num_techs));
        for _ in 0..num_techs {
            tech_states.push(TechState::read_from(&mut input)?);
        }
        Ok(Self { tech_states })
    }
}

#[derive(Debug, Clone)]
pub struct UserPatchData {
}

impl UserPatchData {
    pub fn read_from(mut input: impl Read) -> Result<Self> {
        {
            let mut bytes = vec![0; 4080];
            input.read_exact(&mut bytes)?;
        }

        let mut category_priorities = vec![0; 900];
        let mut group_priorities = vec![0; 100];

        for val in category_priorities.iter_mut() {
            *val = input.read_u16::<LE>()?;
        }

        for val in group_priorities.iter_mut() {
            *val = input.read_u16::<LE>()?;
        }

        {
            let mut bytes = vec![0; 2096];
            input.read_exact(&mut bytes)?;
        }

        Ok(Self {
        })
    }
}