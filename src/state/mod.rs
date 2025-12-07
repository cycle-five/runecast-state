//! State management module for RuneCast.
//!
//! This module provides the core state types and managers:
//!
//! - `player` - Player state machine (where is each player?)
//! - `connection` - WebSocket connection tracking and reconnection
//! - `lobby` - Lobby membership and configuration
//! - `game` - Active game sessions
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                           AppState                                       │
//! │                                                                          │
//! │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐          │
//! │  │ ConnectionManager│  │  LobbyManager   │  │   GameManager   │          │
//! │  │                 │  │                 │  │                 │          │
//! │  │ player_id →     │  │ lobby_id →      │  │ game_id →       │          │
//! │  │   Connection    │  │   Lobby         │  │   Game          │          │
//! │  │                 │  │                 │  │                 │          │
//! │  │ session →       │  │ channel_id →    │  │ player_id →     │          │
//! │  │   player_id     │  │   lobby_id      │  │   game_id       │          │
//! │  │                 │  │                 │  │                 │          │
//! │  │                 │  │ player_id →     │  │                 │          │
//! │  │                 │  │   lobby_id      │  │                 │          │
//! │  └─────────────────┘  └─────────────────┘  └─────────────────┘          │
//! │                                                                          │
//! │  ┌─────────────────────────────────────────────────────────────────┐    │
//! │  │                     PlayerState (per player)                     │    │
//! │  │                                                                  │    │
//! │  │  Disconnected ──▶ Connected ──▶ InLobby ──▶ InGame/Spectating   │    │
//! │  │       ▲              ▲             ▲              │              │    │
//! │  │       └──────────────┴─────────────┴──────────────┘              │    │
//! │  └─────────────────────────────────────────────────────────────────┘    │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use runecast_state::state::{
//!     connection::ConnectionManager,
//!     lobby::LobbyManager,
//!     game::GameManager,
//!     player::{PlayerState, PlayerEvent},
//! };
//!
//! // Create managers
//! let mut connections = ConnectionManager::new();
//! let mut lobbies = LobbyManager::new();
//! let mut games = GameManager::new();
//!
//! // Track player state
//! let mut player_state = PlayerState::new();
//! player_state.apply_mut(PlayerEvent::Connect)?;
//! player_state.apply_mut(PlayerEvent::JoinLobby { lobby_id: "lobby-1".into() })?;
//! ```

pub mod connection;
pub mod game;
pub mod lobby;
pub mod player;

// Re-export commonly used types
pub use connection::{Connection, ConnectionManager, ConnectionStatus, PendingMessage};
pub use game::{
    Game, GameError, GameManager, GamePlayer, GameStatus, Grid, GridCell, Multiplier, Position,
    Spectator, TimerVoteState, GRID_SIZE,
};
pub use lobby::{Lobby, LobbyError, LobbyManager, LobbyMember, LobbyType, MAX_LOBBY_PLAYERS};
pub use player::{InvalidTransition, PlayerEvent, PlayerLocation, PlayerState};

/// Combined application state.
///
/// This is an optional convenience struct that combines all managers.
/// You can also use the individual managers directly.
#[derive(Debug, Default)]
pub struct AppState {
    pub connections: ConnectionManager,
    pub lobbies: LobbyManager,
    pub games: GameManager,
    /// Individual player state machines
    player_states: std::collections::HashMap<i64, PlayerState>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get player state, creating if needed.
    pub fn player_state(&mut self, player_id: i64) -> &mut PlayerState {
        self.player_states
            .entry(player_id)
            .or_insert_with(PlayerState::new)
    }

    /// Get player state if exists.
    pub fn get_player_state(&self, player_id: i64) -> Option<&PlayerState> {
        self.player_states.get(&player_id)
    }

    /// Remove player state.
    pub fn remove_player_state(&mut self, player_id: i64) -> Option<PlayerState> {
        self.player_states.remove(&player_id)
    }

    /// Apply a player event, updating all relevant state.
    pub fn apply_player_event(
        &mut self,
        player_id: i64,
        event: PlayerEvent,
    ) -> Result<(), InvalidTransition> {
        let state = self.player_state(player_id);
        state.apply_mut(event)
    }

    /// Cleanup stale connections and remove expired players.
    pub fn cleanup(&mut self) -> CleanupResult {
        let expired_connections = self.connections.expire_stale();
        let empty_lobbies = self.lobbies.cleanup_empty();
        let finished_games = self.games.cleanup_finished();

        // Mark disconnected players
        for player_id in &expired_connections {
            if let Some(state) = self.player_states.get_mut(player_id) {
                let _ = state.apply_mut(PlayerEvent::Disconnect);
            }
        }

        CleanupResult {
            expired_connections,
            empty_lobbies,
            finished_games,
        }
    }
}

/// Result of cleanup operation.
#[derive(Debug, Default)]
pub struct CleanupResult {
    pub expired_connections: Vec<i64>,
    pub empty_lobbies: Vec<String>,
    pub finished_games: Vec<String>,
}

impl CleanupResult {
    pub fn is_empty(&self) -> bool {
        self.expired_connections.is_empty()
            && self.empty_lobbies.is_empty()
            && self.finished_games.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_basic() {
        let mut state = AppState::new();

        // Create player state
        let ps = state.player_state(1);
        assert!(!ps.is_connected());

        // Apply event
        state
            .apply_player_event(1, PlayerEvent::Connect)
            .unwrap();
        assert!(state.get_player_state(1).unwrap().is_connected());
    }
}
