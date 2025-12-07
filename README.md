# runecast-state

State management for RuneCast game logic.

## Overview

This crate owns **data structures and state machines**. It knows nothing about WebSockets, HTTP, or message formats - just pure state.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           AppState                                       │
│                                                                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐          │
│  │ConnectionManager│  │  LobbyManager   │  │   GameManager   │          │
│  │                 │  │                 │  │                 │          │
│  │ Tracks WS       │  │ Tracks lobbies  │  │ Tracks active   │          │
│  │ connections,    │  │ and membership  │  │ games, turns,   │          │
│  │ sessions,       │  │                 │  │ scoring         │          │
│  │ reconnection    │  │                 │  │                 │          │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘          │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                 PlayerState (per player)                         │    │
│  │  Disconnected ──▶ Connected ──▶ InLobby ──▶ InGame/Spectating   │    │
│  └─────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────┘
```

## Design Goals

1. **State machines validate transitions** - Can't join a game without being in a lobby
2. **Indexed access** - Look up by player_id, session_token, lobby_code, etc.
3. **No networking** - Pure data structures, easily testable
4. **Serialization-ready** - All types can convert to JSON for clients

## Module Structure

```
src/state/
├── mod.rs        # AppState combining all managers, re-exports
├── player.rs     # PlayerLocation state machine
├── connection.rs # Connection tracking with reconnection support  
├── lobby.rs      # Lobby membership and configuration
└── game.rs       # Active game sessions
```

## Player State Machine

Tracks where each player is in the system:

```
┌──────────────┐
│ Disconnected │◀──────────────────────────────────┐
└──────┬───────┘                                   │
       │ connect                                   │ disconnect
       ▼                                           │
┌──────────────┐    join_lobby    ┌───────────┐   │
│  Connected   │─────────────────▶│  InLobby  │───┤
└──────────────┘                  └─────┬─────┘   │
       ▲                                │         │
       │                     start_game │ spectate│
       │                                ▼         ▼
       │                          ┌───────────────────┐
       │                          │ InGame/Spectating │
       │                          └─────────┬─────────┘
       │                                    │
       └────────────────────────────────────┘
```

```rust
use runecast_state::state::{PlayerState, PlayerEvent, PlayerLocation};

let mut state = PlayerState::new();
assert!(!state.is_connected());

// Valid transition
state.apply_mut(PlayerEvent::Connect)?;
assert!(state.is_connected());

// Invalid transition - can't join lobby without connecting first
let disconnected = PlayerState::new();
let result = disconnected.apply(PlayerEvent::JoinLobby { 
    lobby_id: "lobby-1".into() 
});
assert!(result.is_err()); // InvalidTransition
```

## Connection Management

Tracks WebSocket connections with reconnection support:

```rust
use runecast_state::state::connection::{Connection, ConnectionManager};

let mut manager = ConnectionManager::new();

// New connection
let conn = Connection::new(
    player_id,
    user_id,
    username,
    avatar_url,
    session_token,
);
manager.add(conn);

// Lookup by session (for reconnection)
if let Some(conn) = manager.get_by_session_mut(&token) {
    let pending = conn.reconnect()?; // Returns messages to replay
}

// Track message sequences
let seq = conn.send(message_json); // Increments seq, stores for replay
conn.acknowledge(client_ack);       // Removes acknowledged from pending

// Periodic cleanup
let expired = manager.expire_stale(); // Returns expired player IDs
```

## Lobby Management

Channel lobbies (Discord) and custom lobbies (shareable code):

```rust
use runecast_state::state::lobby::{Lobby, LobbyManager, LobbyMember};

let mut manager = LobbyManager::new();

// Find or create channel lobby
let lobby = manager.find_or_create_channel(channel_id, guild_id);

// Add player
let member = LobbyMember::new(player_id, user_id, username, avatar_url);
manager.add_player(&lobby.id, member)?;

// Lookup
let lobby = manager.get_for_player(player_id);
let lobby = manager.get_by_code("ABC123"); // Case-insensitive

// Ready state
lobby.set_ready(player_id, true)?;
if lobby.all_ready() {
    // Start game
}
```

## Game Management

Active game sessions with grid, turns, and scoring:

```rust
use runecast_state::state::game::{Game, GameManager, GamePlayer, Position};

let mut manager = GameManager::new();

// Create game
let mut game = Game::new(game_id, lobby_id, grid);
game.add_player(GamePlayer::new(player_id, user_id, username, None, 0))?;
game.start()?;

// Turn management
assert!(game.is_player_turn(player_id));
let (next_player, round) = game.advance_turn();

// Word tracking
game.use_word("HELLO");
assert!(game.is_word_used("hello")); // Case-insensitive

// Grid access
let cell = game.get_cell(Position::new(2, 3));
let word = game.extract_word(&path);
```

## Combined AppState

Convenience struct combining all managers:

```rust
use runecast_state::state::AppState;

let mut app = AppState::new();

// Access individual managers
app.connections.add(conn);
app.lobbies.find_or_create_channel(channel_id, None);
app.games.add(game);

// Player state machine
app.apply_player_event(player_id, PlayerEvent::Connect)?;
app.apply_player_event(player_id, PlayerEvent::JoinLobby { lobby_id })?;

// Periodic cleanup
let result = app.cleanup();
for player_id in result.expired_connections {
    // Handle disconnected player
}
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
runecast-state = { path = "../runecast-state" }
```

## Why a Separate Crate?

1. **Testability** - State logic can be tested without WebSocket mocking
2. **Clarity** - Clear boundary between "what data exists" and "what to do with it"
3. **Reusability** - Could be used by CLI tools, test harnesses, etc.
4. **No async** - Synchronous code, no runtime requirements
