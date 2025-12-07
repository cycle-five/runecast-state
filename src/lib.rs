//! RuneCast State Library
//!
//! This crate provides state management for RuneCast game logic.
//!
//! # Overview
//!
//! The state module provides:
//!
//! - **Player State Machine** - Tracks where each player is (disconnected, connected,
//!   in lobby, playing, spectating) with validated transitions.
//!
//! - **Connection Management** - Tracks WebSocket connections, handles reconnection
//!   with grace periods, and manages message sequencing.
//!
//! - **Lobby Management** - Channel and custom lobbies, membership, ready states.
//!
//! - **Game Management** - Active game sessions with grid, turns, scoring.
//!
//! # Design Principles
//!
//! 1. **State machines validate transitions** - Invalid state changes are rejected
//!    at compile time or runtime with clear errors.
//!
//! 2. **Managers provide indexed access** - Look up by ID, by player, by session, etc.
//!
//! 3. **No networking** - This crate is pure state, no WebSocket or HTTP.
//!
//! 4. **Serialization-ready** - All types can be converted to JSON for clients.
//!
//! # Example
//!
//! ```rust
//! use runecast_state::state::{
//!     AppState, PlayerEvent,
//!     connection::Connection,
//!     lobby::{Lobby, LobbyMember},
//! };
//!
//! let mut app = AppState::new();
//!
//! // Track a new connection
//! let conn = Connection::new(1, "12345".to_string(), "Alice".to_string(), None, "session-abc".to_string());
//! app.connections.add(conn);
//!
//! // Update player state machine
//! app.apply_player_event(1, PlayerEvent::Connect).unwrap();
//!
//! // Create/join a lobby
//! let lobby_id = {
//!     let lobby = app.lobbies.find_or_create_channel("channel-1".to_string(), None);
//!     lobby.id.clone()
//! };
//! let member = LobbyMember::new(1, "12345".to_string(), "Alice".to_string(), None);
//! app.lobbies.add_player(&lobby_id, member).unwrap();
//!
//! app.apply_player_event(1, PlayerEvent::JoinLobby { lobby_id }).unwrap();
//! ```

pub mod state;

// Re-export everything from state module at crate root
pub use state::*;
