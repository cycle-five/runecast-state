//! Player state machine.
//!
//! Tracks where a player is in the system and validates transitions.
//!
//! # State Diagram
//!
//! ```text
//! ┌──────────────┐
//! │ Disconnected │◀──────────────────────────────────┐
//! └──────┬───────┘                                   │
//!        │ connect                                   │ disconnect
//!        ▼                                           │
//! ┌──────────────┐    join_lobby    ┌───────────┐   │
//! │  Connected   │─────────────────▶│  InLobby  │───┤
//! └──────────────┘                  └─────┬─────┘   │
//!        ▲                                │         │
//!        │ leave_lobby                    │         │
//!        │                                │         │
//!        │         ┌──────────────────────┼─────────┤
//!        │         │                      │         │
//!        │         │ start_game           │ spectate│
//!        │         ▼                      ▼         │
//!        │   ┌───────────┐         ┌───────────┐   │
//!        │   │  InGame   │◀───────▶│ Spectating│───┤
//!        │   │ (playing) │ join as │           │   │
//!        │   └─────┬─────┘ player  └─────┬─────┘   │
//!        │         │                     │         │
//!        │         │ game_end            │ leave   │
//!        │         ▼                     │         │
//!        │   ┌───────────┐               │         │
//!        └───│  InLobby  │◀──────────────┘         │
//!            └───────────┘                         │
//!                  │                               │
//!                  └───────────────────────────────┘
//! ```

use std::fmt;

/// Player's current location/state in the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerLocation {
    /// Not connected to any WebSocket
    Disconnected,

    /// Connected but not in any lobby
    Connected,

    /// In a lobby, not in a game
    InLobby { lobby_id: String },

    /// Playing in a game (also implicitly in the game's lobby)
    InGame { lobby_id: String, game_id: String },

    /// Spectating a game (also implicitly in the game's lobby)
    Spectating { lobby_id: String, game_id: String },
}

impl Default for PlayerLocation {
    fn default() -> Self {
        Self::Disconnected
    }
}

impl PlayerLocation {
    /// Check if player is connected (any state except Disconnected).
    pub fn is_connected(&self) -> bool {
        !matches!(self, Self::Disconnected)
    }

    /// Check if player is in a lobby.
    pub fn is_in_lobby(&self) -> bool {
        matches!(
            self,
            Self::InLobby { .. } | Self::InGame { .. } | Self::Spectating { .. }
        )
    }

    /// Check if player is in a game (playing or spectating).
    pub fn is_in_game(&self) -> bool {
        matches!(self, Self::InGame { .. } | Self::Spectating { .. })
    }

    /// Check if player is actively playing (not spectating).
    pub fn is_playing(&self) -> bool {
        matches!(self, Self::InGame { .. })
    }

    /// Check if player is spectating.
    pub fn is_spectating(&self) -> bool {
        matches!(self, Self::Spectating { .. })
    }

    /// Get the lobby ID if in a lobby.
    pub fn lobby_id(&self) -> Option<&str> {
        match self {
            Self::InLobby { lobby_id }
            | Self::InGame { lobby_id, .. }
            | Self::Spectating { lobby_id, .. } => Some(lobby_id),
            _ => None,
        }
    }

    /// Get the game ID if in a game.
    pub fn game_id(&self) -> Option<&str> {
        match self {
            Self::InGame { game_id, .. } | Self::Spectating { game_id, .. } => Some(game_id),
            _ => None,
        }
    }
}

impl fmt::Display for PlayerLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => write!(f, "Disconnected"),
            Self::Connected => write!(f, "Connected"),
            Self::InLobby { lobby_id } => write!(f, "InLobby({})", lobby_id),
            Self::InGame { lobby_id, game_id } => {
                write!(f, "InGame({}, {})", lobby_id, game_id)
            }
            Self::Spectating { lobby_id, game_id } => {
                write!(f, "Spectating({}, {})", lobby_id, game_id)
            }
        }
    }
}

/// State transition events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerEvent {
    Connect,
    Disconnect,
    JoinLobby { lobby_id: String },
    LeaveLobby,
    StartGame { game_id: String },
    JoinGame { game_id: String },
    SpectateGame { game_id: String },
    LeaveGame,
    BecomePlayer,
    BecomeSpectator,
}

/// Error when a state transition is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidTransition {
    pub from: PlayerLocation,
    pub event: PlayerEvent,
    pub reason: &'static str,
}

impl fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Invalid transition from {} via {:?}: {}",
            self.from, self.event, self.reason
        )
    }
}

impl std::error::Error for InvalidTransition {}

/// Player state machine.
///
/// Encapsulates valid state transitions and enforces invariants.
#[derive(Debug, Clone, Default)]
pub struct PlayerState {
    location: PlayerLocation,
}

impl PlayerState {
    /// Create a new disconnected player state.
    pub fn new() -> Self {
        Self {
            location: PlayerLocation::Disconnected,
        }
    }

    /// Create a player state at a specific location (for restoring state).
    pub fn at(location: PlayerLocation) -> Self {
        Self { location }
    }

    /// Get current location.
    pub fn location(&self) -> &PlayerLocation {
        &self.location
    }

    /// Apply an event, returning the new state or an error.
    pub fn apply(&self, event: PlayerEvent) -> Result<Self, InvalidTransition> {
        let new_location = self.transition(&event)?;
        Ok(Self {
            location: new_location,
        })
    }

    /// Apply an event in place, returning error if invalid.
    pub fn apply_mut(&mut self, event: PlayerEvent) -> Result<(), InvalidTransition> {
        self.location = self.transition(&event)?;
        Ok(())
    }

    /// Calculate the new location for an event.
    fn transition(&self, event: &PlayerEvent) -> Result<PlayerLocation, InvalidTransition> {
        use PlayerEvent::*;
        use PlayerLocation::*;

        let invalid = |reason: &'static str| InvalidTransition {
            from: self.location.clone(),
            event: event.clone(),
            reason,
        };

        match (&self.location, event) {
            // Connect: Disconnected -> Connected
            (Disconnected, Connect) => Ok(Connected),
            (_, Connect) => Err(invalid("Already connected")),

            // Disconnect: Any -> Disconnected
            (Disconnected, Disconnect) => Err(invalid("Already disconnected")),
            (_, Disconnect) => Ok(Disconnected),

            // JoinLobby: Connected -> InLobby
            (Connected, JoinLobby { lobby_id }) => Ok(InLobby {
                lobby_id: lobby_id.clone(),
            }),
            (InLobby { .. }, JoinLobby { .. }) => Err(invalid("Already in a lobby")),
            (InGame { .. }, JoinLobby { .. }) => Err(invalid("Must leave game first")),
            (Spectating { .. }, JoinLobby { .. }) => Err(invalid("Must leave game first")),
            (Disconnected, JoinLobby { .. }) => Err(invalid("Must connect first")),

            // LeaveLobby: InLobby -> Connected
            (InLobby { .. }, LeaveLobby) => Ok(Connected),
            (InGame { .. }, LeaveLobby) => Err(invalid("Must leave game first")),
            (Spectating { .. }, LeaveLobby) => Err(invalid("Must leave game first")),
            (_, LeaveLobby) => Err(invalid("Not in a lobby")),

            // StartGame: InLobby -> InGame
            (InLobby { lobby_id }, StartGame { game_id }) => Ok(InGame {
                lobby_id: lobby_id.clone(),
                game_id: game_id.clone(),
            }),
            (InGame { .. }, StartGame { .. }) => Err(invalid("Already in a game")),
            (_, StartGame { .. }) => Err(invalid("Must be in a lobby to start a game")),

            // JoinGame: InLobby -> InGame (mid-game join)
            (InLobby { lobby_id }, JoinGame { game_id }) => Ok(InGame {
                lobby_id: lobby_id.clone(),
                game_id: game_id.clone(),
            }),
            (Spectating { lobby_id, .. }, JoinGame { game_id }) => Ok(InGame {
                lobby_id: lobby_id.clone(),
                game_id: game_id.clone(),
            }),
            (InGame { .. }, JoinGame { .. }) => Err(invalid("Already playing")),
            (_, JoinGame { .. }) => Err(invalid("Must be in lobby or spectating")),

            // SpectateGame: InLobby -> Spectating
            (InLobby { lobby_id }, SpectateGame { game_id }) => Ok(Spectating {
                lobby_id: lobby_id.clone(),
                game_id: game_id.clone(),
            }),
            (Connected, SpectateGame { game_id }) => {
                // Allow spectating without being in lobby (for public games)
                // We'll use a placeholder lobby_id
                Ok(Spectating {
                    lobby_id: format!("spectate-{}", game_id),
                    game_id: game_id.clone(),
                })
            }
            (InGame { .. }, SpectateGame { .. }) => Err(invalid("Already in a game")),
            (Spectating { .. }, SpectateGame { .. }) => Err(invalid("Already spectating")),
            (Disconnected, SpectateGame { .. }) => Err(invalid("Must connect first")),

            // LeaveGame: InGame/Spectating -> InLobby
            (InGame { lobby_id, .. }, LeaveGame) => Ok(InLobby {
                lobby_id: lobby_id.clone(),
            }),
            (Spectating { lobby_id, .. }, LeaveGame) => Ok(InLobby {
                lobby_id: lobby_id.clone(),
            }),
            (_, LeaveGame) => Err(invalid("Not in a game")),

            // BecomePlayer: Spectating -> InGame
            (Spectating { lobby_id, game_id }, BecomePlayer) => Ok(InGame {
                lobby_id: lobby_id.clone(),
                game_id: game_id.clone(),
            }),
            (InGame { .. }, BecomePlayer) => Err(invalid("Already a player")),
            (_, BecomePlayer) => Err(invalid("Must be spectating")),

            // BecomeSpectator: InGame -> Spectating
            (InGame { lobby_id, game_id }, BecomeSpectator) => Ok(Spectating {
                lobby_id: lobby_id.clone(),
                game_id: game_id.clone(),
            }),
            (Spectating { .. }, BecomeSpectator) => Err(invalid("Already spectating")),
            (_, BecomeSpectator) => Err(invalid("Must be in a game")),
        }
    }

    // Convenience methods for common checks

    pub fn is_connected(&self) -> bool {
        self.location.is_connected()
    }

    pub fn is_in_lobby(&self) -> bool {
        self.location.is_in_lobby()
    }

    pub fn is_in_game(&self) -> bool {
        self.location.is_in_game()
    }

    pub fn is_playing(&self) -> bool {
        self.location.is_playing()
    }

    pub fn is_spectating(&self) -> bool {
        self.location.is_spectating()
    }

    pub fn lobby_id(&self) -> Option<&str> {
        self.location.lobby_id()
    }

    pub fn game_id(&self) -> Option<&str> {
        self.location.game_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let state = PlayerState::new();
        assert!(!state.is_connected());
        assert_eq!(*state.location(), PlayerLocation::Disconnected);
    }

    #[test]
    fn test_connect_disconnect() {
        let mut state = PlayerState::new();

        // Connect
        state.apply_mut(PlayerEvent::Connect).unwrap();
        assert!(state.is_connected());
        assert!(!state.is_in_lobby());

        // Disconnect
        state.apply_mut(PlayerEvent::Disconnect).unwrap();
        assert!(!state.is_connected());
    }

    #[test]
    fn test_lobby_flow() {
        let mut state = PlayerState::new();

        state.apply_mut(PlayerEvent::Connect).unwrap();
        state
            .apply_mut(PlayerEvent::JoinLobby {
                lobby_id: "lobby-1".to_string(),
            })
            .unwrap();

        assert!(state.is_in_lobby());
        assert_eq!(state.lobby_id(), Some("lobby-1"));

        state.apply_mut(PlayerEvent::LeaveLobby).unwrap();
        assert!(!state.is_in_lobby());
        assert!(state.is_connected());
    }

    #[test]
    fn test_game_flow() {
        let mut state = PlayerState::new();

        state.apply_mut(PlayerEvent::Connect).unwrap();
        state
            .apply_mut(PlayerEvent::JoinLobby {
                lobby_id: "lobby-1".to_string(),
            })
            .unwrap();
        state
            .apply_mut(PlayerEvent::StartGame {
                game_id: "game-1".to_string(),
            })
            .unwrap();

        assert!(state.is_in_game());
        assert!(state.is_playing());
        assert!(!state.is_spectating());
        assert_eq!(state.game_id(), Some("game-1"));
        assert_eq!(state.lobby_id(), Some("lobby-1"));

        state.apply_mut(PlayerEvent::LeaveGame).unwrap();
        assert!(!state.is_in_game());
        assert!(state.is_in_lobby());
    }

    #[test]
    fn test_spectator_flow() {
        let mut state = PlayerState::new();

        state.apply_mut(PlayerEvent::Connect).unwrap();
        state
            .apply_mut(PlayerEvent::JoinLobby {
                lobby_id: "lobby-1".to_string(),
            })
            .unwrap();
        state
            .apply_mut(PlayerEvent::SpectateGame {
                game_id: "game-1".to_string(),
            })
            .unwrap();

        assert!(state.is_in_game());
        assert!(!state.is_playing());
        assert!(state.is_spectating());

        // Become player
        state.apply_mut(PlayerEvent::BecomePlayer).unwrap();
        assert!(state.is_playing());
        assert!(!state.is_spectating());

        // Become spectator
        state.apply_mut(PlayerEvent::BecomeSpectator).unwrap();
        assert!(state.is_spectating());
        assert!(!state.is_playing());
    }

    #[test]
    fn test_invalid_transitions() {
        let state = PlayerState::new();

        // Can't join lobby when disconnected
        let result = state.apply(PlayerEvent::JoinLobby {
            lobby_id: "lobby-1".to_string(),
        });
        assert!(result.is_err());

        // Can't connect twice
        let connected = state.apply(PlayerEvent::Connect).unwrap();
        let result = connected.apply(PlayerEvent::Connect);
        assert!(result.is_err());

        // Can't start game without being in lobby
        let result = connected.apply(PlayerEvent::StartGame {
            game_id: "game-1".to_string(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_display() {
        let loc = PlayerLocation::InGame {
            lobby_id: "lobby-1".to_string(),
            game_id: "game-1".to_string(),
        };
        assert_eq!(format!("{}", loc), "InGame(lobby-1, game-1)");
    }
}
