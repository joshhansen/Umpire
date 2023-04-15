use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use async_trait::async_trait;

use crate::{
    game::{
        city::{City, CityID},
        error::GameError,
        map::Tile,
        obs::{Obs, ObsTracker},
        unit::{
            orders::{Orders, OrdersResult},
            Unit, UnitID, UnitType,
        },
    },
    util::{Dims, Direction, Location, Wrap2d},
};

use super::{
    action::{
        Actionable, AiPlayerAction, NextCityAction, NextUnitAction, PlayerAction,
        PlayerActionOutcome,
    },
    ai::{fX, TrainingFocus},
    move_::Move,
    obs::LocatedObsLite,
    player::PlayerNum,
    Game, PlayerSecret, ProductionCleared, ProposedActionResult, ProposedOrdersResult,
    ProposedResult, TurnNum, TurnPhase, TurnStart, UmpireResult, UnitDisbanded,
};

pub use super::traits::IGame;

#[async_trait]
impl IGame for Game {
    async fn num_players(&self) -> PlayerNum {
        self.num_players()
    }

    async fn is_player_turn(&self, secret: PlayerSecret) -> UmpireResult<bool> {
        self.is_player_turn(secret)
    }

    async fn begin_turn(
        &mut self,
        player_secret: PlayerSecret,
        clear_after_unit_production: bool,
    ) -> UmpireResult<TurnStart> {
        self.begin_turn(player_secret, clear_after_unit_production)
    }

    async fn turn_is_done(&self, player: PlayerNum, turn: TurnNum) -> UmpireResult<bool> {
        self.turn_is_done(player, turn)
    }

    async fn current_turn_is_done(&self) -> bool {
        self.current_turn_is_done()
    }

    async fn victor(&self) -> Option<PlayerNum> {
        self.victor()
    }

    async fn end_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<()> {
        self.end_turn(player_secret)
    }

    async fn force_end_turn(&mut self, player_secret: PlayerSecret) -> UmpireResult<()> {
        self.force_end_turn(player_secret)
    }

    async fn end_then_begin_turn(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
        clear_after_unit_production: bool,
    ) -> UmpireResult<TurnStart> {
        self.end_then_begin_turn(
            player_secret,
            next_player_secret,
            clear_after_unit_production,
        )
    }

    async fn force_end_then_begin_turn(
        &mut self,
        player_secret: PlayerSecret,
        next_player_secret: PlayerSecret,
        clear_after_unit_production: bool,
    ) -> UmpireResult<TurnStart> {
        self.force_end_then_begin_turn(
            player_secret,
            next_player_secret,
            clear_after_unit_production,
        )
    }

    async fn player_unit_legal_one_step_destinations(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<HashSet<Location>> {
        self.player_unit_legal_one_step_destinations(player_secret, unit_id)
    }

    async fn player_unit_legal_directions(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<Vec<Direction>> {
        self.player_unit_legal_directions(player_secret, unit_id)
            .map(|dirs| dirs.collect())
    }

    async fn player_tile(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<Cow<Tile>>> {
        self.player_tile(player_secret, loc)
            .map(|tile| tile.map(|tile| Cow::Borrowed(tile)))
    }

    async fn player_obs(&self, player_secret: PlayerSecret, loc: Location) -> UmpireResult<Obs> {
        self.player_obs(player_secret, loc).map(|obs| obs.clone())
    }

    async fn player_observations(&self, player_secret: PlayerSecret) -> UmpireResult<ObsTracker> {
        self.player_observations(player_secret)
            .map(|tracker| tracker.clone())
    }

    async fn player_cities(&self, player_secret: PlayerSecret) -> UmpireResult<Vec<City>> {
        self.player_cities(player_secret)
            .map(|cities| cities.cloned().collect())
    }

    async fn player_cities_with_production_target(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<City>> {
        self.player_cities_with_production_target(player_secret)
            .map(|cities| cities.cloned().collect())
    }

    async fn player_city_count(&self, player_secret: PlayerSecret) -> UmpireResult<usize> {
        self.player_city_count(player_secret)
    }

    async fn player_cities_producing_or_not_ignored(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<usize> {
        self.player_cities_producing_or_not_ignored(player_secret)
    }

    async fn player_units(&self, player_secret: PlayerSecret) -> UmpireResult<Vec<Unit>> {
        self.player_units(player_secret)
            .map(|units| units.cloned().collect())
    }

    async fn player_unit_type_counts(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<HashMap<UnitType, usize>> {
        self.player_unit_type_counts(player_secret)
            .map(|counts| counts.clone())
    }

    async fn player_city_by_loc(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<City>> {
        self.player_city_by_loc(player_secret, loc)
            .map(|city| city.cloned())
    }

    async fn player_city_by_id(
        &self,
        player_secret: PlayerSecret,
        city_id: CityID,
    ) -> UmpireResult<Option<City>> {
        self.player_city_by_id(player_secret, city_id)
            .map(|city| city.cloned())
    }

    async fn player_unit_by_id(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Unit>> {
        self.player_unit_by_id(player_secret, id)
            .map(|unit| unit.cloned())
    }

    async fn player_unit_loc(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Location>> {
        self.player_unit_loc(player_secret, id)
            .map(|loc| loc.clone())
    }

    async fn player_toplevel_unit_by_loc(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Option<Unit>> {
        self.player_toplevel_unit_by_loc(player_secret, loc)
            .map(|unit| unit.cloned())
    }

    async fn player_production_set_requests(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Location>> {
        self.player_production_set_requests(player_secret)
            .map(|rqsts| rqsts.collect())
    }

    async fn player_unit_orders_requests(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<UnitID>> {
        self.player_unit_orders_requests(player_secret)
            .map(|rqsts| rqsts.collect())
    }

    async fn player_units_with_orders_requests(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<Unit>> {
        self.player_units_with_orders_requests(player_secret)
            .map(|units| units.cloned().collect())
    }

    async fn player_units_with_pending_orders(
        &self,
        player_secret: PlayerSecret,
    ) -> UmpireResult<Vec<UnitID>> {
        Game::player_units_with_pending_orders(self, player_secret).map(|units| units.collect())
    }

    async fn move_toplevel_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        Game::move_toplevel_unit_by_id(self, player_secret, unit_id, dest)
    }

    async fn move_toplevel_unit_by_id_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        Game::move_toplevel_unit_by_id_avoiding_combat(self, player_secret, unit_id, dest)
    }

    async fn move_toplevel_unit_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move> {
        Game::move_toplevel_unit_by_loc(self, player_secret, src, dest)
    }

    async fn move_toplevel_unit_by_loc_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        src: Location,
        dest: Location,
    ) -> UmpireResult<Move> {
        Game::move_toplevel_unit_by_loc_avoiding_combat(self, player_secret, src, dest)
    }

    async fn move_unit_by_id_in_direction(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        direction: Direction,
    ) -> UmpireResult<Move> {
        Game::move_unit_by_id_in_direction(self, player_secret, unit_id, direction)
    }

    async fn move_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        Game::move_unit_by_id(self, player_secret, unit_id, dest)
    }

    async fn propose_move_unit_by_id(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError> {
        Game::propose_move_unit_by_id(self, player_secret, id, dest)
    }

    async fn move_unit_by_id_avoiding_combat(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> UmpireResult<Move> {
        self.move_unit_by_id_avoiding_combat(player_secret, id, dest)
    }

    async fn propose_move_unit_by_id_avoiding_combat(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        dest: Location,
    ) -> ProposedResult<Move, GameError> {
        self.propose_move_unit_by_id_avoiding_combat(player_secret, id, dest)
    }

    async fn disband_unit_by_id(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> UmpireResult<UnitDisbanded> {
        self.disband_unit_by_id(player_secret, unit_id)
    }

    async fn set_production_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>> {
        self.set_production_by_loc(player_secret, loc, production)
    }

    async fn set_production_by_id(
        &mut self,
        player_secret: PlayerSecret,
        city_id: CityID,
        production: UnitType,
    ) -> UmpireResult<Option<UnitType>> {
        self.set_production_by_id(player_secret, city_id, production)
    }

    async fn clear_production(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
        ignore_cleared_production: bool,
    ) -> UmpireResult<ProductionCleared> {
        self.clear_production(player_secret, loc, ignore_cleared_production)
    }

    async fn clear_productions(
        &mut self,
        player_secret: PlayerSecret,
        ignore_cleared_productions: bool,
    ) -> UmpireResult<Vec<ProductionCleared>> {
        self.clear_productions(player_secret, ignore_cleared_productions)
            .map(|prods_cleared| prods_cleared.collect())
    }

    async fn turn(&self) -> TurnNum {
        self.turn()
    }

    async fn turn_phase(&self) -> TurnPhase {
        self.turn_phase()
    }

    async fn current_player(&self) -> PlayerNum {
        self.current_player()
    }

    async fn dims(&self) -> Dims {
        self.dims()
    }

    async fn wrapping(&self) -> Wrap2d {
        self.wrapping()
    }

    async fn valid_productions(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Vec<UnitType>> {
        self.valid_productions(player_secret, loc)
            .map(|prods| prods.collect())
    }

    async fn valid_productions_conservative(
        &self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<Vec<UnitType>> {
        self.valid_productions_conservative(player_secret, loc)
            .map(|prods| prods.collect())
    }

    async fn order_unit_sentry(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        self.order_unit_sentry(player_secret, unit_id)
    }

    async fn order_unit_skip(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        self.order_unit_skip(player_secret, unit_id)
    }

    async fn order_unit_go_to(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> OrdersResult {
        self.order_unit_go_to(player_secret, unit_id, dest)
    }

    async fn propose_order_unit_go_to(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
        dest: Location,
    ) -> ProposedOrdersResult {
        self.propose_order_unit_go_to(player_secret, unit_id, dest)
    }

    async fn order_unit_explore(
        &mut self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> OrdersResult {
        self.order_unit_explore(player_secret, unit_id)
    }

    async fn propose_order_unit_explore(
        &self,
        player_secret: PlayerSecret,
        unit_id: UnitID,
    ) -> ProposedOrdersResult {
        self.propose_order_unit_explore(player_secret, unit_id)
    }

    async fn activate_unit_by_loc(
        &mut self,
        player_secret: PlayerSecret,
        loc: Location,
    ) -> UmpireResult<LocatedObsLite> {
        self.activate_unit_by_loc(player_secret, loc)
    }

    async fn set_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> UmpireResult<Option<Orders>> {
        self.set_orders(player_secret, id, orders)
    }

    async fn clear_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
    ) -> UmpireResult<Option<Orders>> {
        self.clear_orders(player_secret, id)
    }

    /// Simulate setting the orders of unit with ID `id` to `orders` and then following them out.
    async fn propose_set_and_follow_orders(
        &self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> ProposedOrdersResult {
        self.propose_set_and_follow_orders(player_secret, id, orders)
    }

    async fn set_and_follow_orders(
        &mut self,
        player_secret: PlayerSecret,
        id: UnitID,
        orders: Orders,
    ) -> OrdersResult {
        self.set_and_follow_orders(player_secret, id, orders)
    }

    async fn current_player_score(&self) -> f64 {
        self.current_player_score()
    }

    async fn player_score(&self, player_secret: PlayerSecret) -> UmpireResult<f64> {
        self.player_score(player_secret)
    }

    async fn player_score_by_idx(&self, player: PlayerNum) -> UmpireResult<f64> {
        self.player_score_by_idx(player)
    }

    async fn player_scores(&self) -> Vec<f64> {
        self.player_scores()
    }

    async fn player_features(
        &self,
        player_secret: PlayerSecret,
        focus: TrainingFocus,
    ) -> UmpireResult<Vec<fX>> {
        self.player_features(player_secret, focus)
    }

    async fn take_simple_action(
        &mut self,
        player_secret: PlayerSecret,
        action: AiPlayerAction,
    ) -> UmpireResult<PlayerActionOutcome> {
        let action = action.to_action(self, player_secret)?;
        self.take_action(player_secret, action)
    }

    async fn take_next_city_action(
        &mut self,
        player_secret: PlayerSecret,
        action: NextCityAction,
    ) -> UmpireResult<PlayerActionOutcome> {
        let action = action.to_action(self, player_secret)?;
        self.take_action(player_secret, action)
    }

    async fn take_next_unit_action(
        &mut self,
        player_secret: PlayerSecret,
        action: NextUnitAction,
    ) -> UmpireResult<PlayerActionOutcome> {
        let action = action.to_action(self, player_secret)?;
        self.take_action(player_secret, action)
    }

    async fn take_action(
        &mut self,
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> UmpireResult<PlayerActionOutcome> {
        self.take_action(player_secret, action)
    }

    async fn propose_action(
        &self,
        player_secret: PlayerSecret,
        action: PlayerAction,
    ) -> ProposedActionResult {
        self.propose_action(player_secret, action)
    }

    fn clone_underlying_game_state(&self) -> Result<Game, String> {
        Ok(self.clone())
    }
}
