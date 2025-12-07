//! Game state management.
//!
//! Tracks active game sessions including grid, players, turns, and scoring.

use std::collections::{HashMap, HashSet};

/// Grid dimensions.
pub const GRID_SIZE: usize = 5;

/// Maximum rounds per game.
pub const DEFAULT_MAX_ROUNDS: u8 = 5;

/// Game state machine states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameStatus {
    /// Game created but not started
    #[default]
    Idle,
    /// Countdown before game starts
    Starting,
    /// Game in progress
    InProgress,
    /// Game completed normally
    Finished,
    /// Game cancelled (player left, etc)
    Cancelled,
}

impl GameStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Starting => "starting",
            Self::InProgress => "in_progress",
            Self::Finished => "finished",
            Self::Cancelled => "cancelled",
        }
    }

    /// Check if game is active (can receive actions).
    pub fn is_active(&self) -> bool {
        matches!(self, Self::InProgress)
    }

    /// Check if game is terminal (cannot change).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Finished | Self::Cancelled)
    }
}

/// Tile multiplier types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Multiplier {
    DoubleLetter,
    TripleLetter,
    DoubleWord,
}

impl Multiplier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DoubleLetter => "double_letter",
            Self::TripleLetter => "triple_letter",
            Self::DoubleWord => "double_word",
        }
    }
}

/// A single grid cell.
#[derive(Debug, Clone)]
pub struct GridCell {
    pub letter: char,
    pub value: u8,
    pub multiplier: Option<Multiplier>,
    pub has_gem: bool,
}

impl GridCell {
    pub fn new(letter: char) -> Self {
        Self {
            letter: letter.to_ascii_uppercase(),
            value: letter_value(letter),
            multiplier: None,
            has_gem: false,
        }
    }

    pub fn with_multiplier(mut self, multiplier: Multiplier) -> Self {
        self.multiplier = Some(multiplier);
        self
    }

    pub fn with_gem(mut self) -> Self {
        self.has_gem = true;
        self
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "letter": self.letter.to_string(),
            "value": self.value
        });
        if let Some(m) = &self.multiplier {
            obj["multiplier"] = serde_json::json!(m.as_str());
        }
        if self.has_gem {
            obj["has_gem"] = serde_json::json!(true);
        }
        obj
    }
}

/// Get point value for a letter.
pub fn letter_value(letter: char) -> u8 {
    match letter.to_ascii_uppercase() {
        'A' | 'E' | 'I' | 'O' | 'U' | 'L' | 'N' | 'S' | 'T' | 'R' => 1,
        'D' | 'G' => 2,
        'B' | 'C' | 'M' | 'P' => 3,
        'F' | 'H' | 'V' | 'W' | 'Y' => 4,
        'K' => 5,
        'J' | 'X' => 8,
        'Q' | 'Z' => 10,
        _ => 1,
    }
}

/// 5x5 game grid.
pub type Grid = [[GridCell; GRID_SIZE]; GRID_SIZE];

/// Grid position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub row: usize,
    pub col: usize,
}

impl Position {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }

    /// Check if position is valid (within grid bounds).
    pub fn is_valid(&self) -> bool {
        self.row < GRID_SIZE && self.col < GRID_SIZE
    }

    /// Check if two positions are adjacent (including diagonals).
    pub fn is_adjacent_to(&self, other: &Position) -> bool {
        let row_diff = (self.row as i32 - other.row as i32).abs();
        let col_diff = (self.col as i32 - other.col as i32).abs();
        row_diff <= 1 && col_diff <= 1 && (row_diff != 0 || col_diff != 0)
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({"row": self.row, "col": self.col})
    }
}

/// A player in the game.
#[derive(Debug, Clone)]
pub struct GamePlayer {
    pub player_id: i64,
    pub user_id: String,
    pub username: String,
    pub avatar_url: Option<String>,
    pub score: i32,
    pub gems: i32,
    pub turn_order: u8,
    pub is_connected: bool,
    pub words_played: Vec<String>,
}

impl GamePlayer {
    pub fn new(
        player_id: i64,
        user_id: String,
        username: String,
        avatar_url: Option<String>,
        turn_order: u8,
    ) -> Self {
        Self {
            player_id,
            user_id,
            username,
            avatar_url,
            score: 0,
            gems: 0,
            turn_order,
            is_connected: true,
            words_played: Vec::new(),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "user_id": self.user_id,
            "username": self.username,
            "avatar_url": self.avatar_url,
            "score": self.score,
            "gems": self.gems,
            "turn_order": self.turn_order,
            "is_connected": self.is_connected
        })
    }
}

/// A spectator.
#[derive(Debug, Clone)]
pub struct Spectator {
    pub player_id: i64,
    pub user_id: String,
    pub username: String,
    pub avatar_url: Option<String>,
}

impl Spectator {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "user_id": self.user_id,
            "username": self.username,
            "avatar_url": self.avatar_url
        })
    }
}

/// Timer vote state.
#[derive(Debug, Clone, Default)]
pub enum TimerVoteState {
    #[default]
    Idle,
    VoteInProgress {
        initiator_id: i64,
        voters: HashSet<i64>,
        votes_needed: u32,
        expires_at: chrono::DateTime<chrono::Utc>,
    },
    TimerActive {
        target_player_id: i64,
        expires_at: chrono::DateTime<chrono::Utc>,
    },
    Cooldown {
        expires_at: chrono::DateTime<chrono::Utc>,
    },
}

impl TimerVoteState {
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Idle => serde_json::json!({"status": "idle"}),
            Self::VoteInProgress {
                voters,
                votes_needed,
                ..
            } => serde_json::json!({
                "status": "vote_in_progress",
                "current_votes": voters.len(),
                "votes_needed": votes_needed
            }),
            Self::TimerActive {
                target_player_id,
                expires_at,
            } => {
                let remaining = (*expires_at - chrono::Utc::now())
                    .num_seconds()
                    .max(0);
                serde_json::json!({
                    "status": "timer_active",
                    "target_player_id": target_player_id,
                    "seconds_remaining": remaining
                })
            }
            Self::Cooldown { expires_at } => {
                let remaining = (*expires_at - chrono::Utc::now())
                    .num_seconds()
                    .max(0);
                serde_json::json!({
                    "status": "cooldown",
                    "seconds_remaining": remaining
                })
            }
        }
    }
}

/// Game session state.
#[derive(Debug, Clone)]
pub struct Game {
    /// Unique game ID
    pub id: String,

    /// Parent lobby ID
    pub lobby_id: String,

    /// Current status
    pub status: GameStatus,

    /// The game grid
    pub grid: Grid,

    /// Players indexed by player_id
    players: HashMap<i64, GamePlayer>,

    /// Turn order (player_ids in order)
    turn_order: Vec<i64>,

    /// Current turn index
    pub current_turn_index: usize,

    /// Current round (1-indexed)
    pub round: u8,

    /// Maximum rounds
    pub max_rounds: u8,

    /// Words already used
    pub used_words: HashSet<String>,

    /// Spectators
    spectators: HashMap<i64, Spectator>,

    /// Timer vote state
    pub timer_vote: TimerVoteState,

    /// When game was created
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// When game started (status -> InProgress)
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,

    /// When game ended
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Game {
    /// Create a new game.
    pub fn new(id: String, lobby_id: String, grid: Grid) -> Self {
        Self {
            id,
            lobby_id,
            status: GameStatus::Idle,
            grid,
            players: HashMap::new(),
            turn_order: Vec::new(),
            current_turn_index: 0,
            round: 1,
            max_rounds: DEFAULT_MAX_ROUNDS,
            used_words: HashSet::new(),
            spectators: HashMap::new(),
            timer_vote: TimerVoteState::Idle,
            created_at: chrono::Utc::now(),
            started_at: None,
            ended_at: None,
        }
    }

    /// Add a player to the game.
    pub fn add_player(&mut self, player: GamePlayer) -> Result<(), GameError> {
        if self.status != GameStatus::Idle {
            return Err(GameError::GameStarted);
        }

        if self.players.contains_key(&player.player_id) {
            return Err(GameError::AlreadyPlayer);
        }

        if self.players.len() >= 6 {
            return Err(GameError::TooManyPlayers);
        }

        let player_id = player.player_id;
        self.players.insert(player_id, player);
        self.turn_order.push(player_id);

        Ok(())
    }

    /// Start the game.
    pub fn start(&mut self) -> Result<(), GameError> {
        if self.status != GameStatus::Idle {
            return Err(GameError::InvalidStatus);
        }

        if self.players.is_empty() {
            return Err(GameError::NotEnoughPlayers);
        }

        self.status = GameStatus::InProgress;
        self.started_at = Some(chrono::Utc::now());

        Ok(())
    }

    /// Get current player ID.
    pub fn current_player_id(&self) -> Option<i64> {
        self.turn_order.get(self.current_turn_index).copied()
    }

    /// Get current player.
    pub fn current_player(&self) -> Option<&GamePlayer> {
        self.current_player_id()
            .and_then(|id| self.players.get(&id))
    }

    /// Check if it's a player's turn.
    pub fn is_player_turn(&self, player_id: i64) -> bool {
        self.current_player_id() == Some(player_id)
    }

    /// Advance to next turn.
    pub fn advance_turn(&mut self) -> (i64, u8) {
        self.current_turn_index = (self.current_turn_index + 1) % self.turn_order.len();

        if self.current_turn_index == 0 {
            self.round += 1;
        }

        (self.current_player_id().unwrap_or(0), self.round)
    }

    /// Check if game should end.
    pub fn should_end(&self) -> bool {
        self.round > self.max_rounds
    }

    /// End the game.
    pub fn end(&mut self) -> Result<Vec<(i64, String, i32)>, GameError> {
        if !self.status.is_active() {
            return Err(GameError::InvalidStatus);
        }

        self.status = GameStatus::Finished;
        self.ended_at = Some(chrono::Utc::now());

        // Return final scores sorted by score descending
        let mut scores: Vec<(i64, String, i32)> = self
            .players
            .values()
            .map(|p| (p.player_id, p.user_id.clone(), p.score))
            .collect();
        scores.sort_by(|a, b| b.2.cmp(&a.2));

        Ok(scores)
    }

    /// Cancel the game.
    pub fn cancel(&mut self, reason: &str) {
        self.status = GameStatus::Cancelled;
        self.ended_at = Some(chrono::Utc::now());
        // Could store reason if needed
        let _ = reason;
    }

    /// Get a player.
    pub fn get_player(&self, player_id: i64) -> Option<&GamePlayer> {
        self.players.get(&player_id)
    }

    /// Get a mutable player.
    pub fn get_player_mut(&mut self, player_id: i64) -> Option<&mut GamePlayer> {
        self.players.get_mut(&player_id)
    }

    /// Check if player is in game.
    pub fn has_player(&self, player_id: i64) -> bool {
        self.players.contains_key(&player_id)
    }

    /// Get all players.
    pub fn players(&self) -> impl Iterator<Item = &GamePlayer> {
        self.players.values()
    }

    /// Get player IDs in turn order.
    pub fn player_ids_in_order(&self) -> &[i64] {
        &self.turn_order
    }

    /// Player count.
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Add a spectator.
    pub fn add_spectator(&mut self, spectator: Spectator) -> Result<(), GameError> {
        if self.spectators.contains_key(&spectator.player_id) {
            return Err(GameError::AlreadySpectator);
        }

        self.spectators.insert(spectator.player_id, spectator);
        Ok(())
    }

    /// Remove a spectator.
    pub fn remove_spectator(&mut self, player_id: i64) -> Option<Spectator> {
        self.spectators.remove(&player_id)
    }

    /// Get spectators.
    pub fn spectators(&self) -> impl Iterator<Item = &Spectator> {
        self.spectators.values()
    }

    /// Spectator count.
    pub fn spectator_count(&self) -> usize {
        self.spectators.len()
    }

    /// Check if word has been used.
    pub fn is_word_used(&self, word: &str) -> bool {
        self.used_words.contains(&word.to_uppercase())
    }

    /// Mark word as used.
    pub fn use_word(&mut self, word: &str) {
        self.used_words.insert(word.to_uppercase());
    }

    /// Get cell at position.
    pub fn get_cell(&self, pos: Position) -> Option<&GridCell> {
        if pos.is_valid() {
            Some(&self.grid[pos.row][pos.col])
        } else {
            None
        }
    }

    /// Get mutable cell at position.
    pub fn get_cell_mut(&mut self, pos: Position) -> Option<&mut GridCell> {
        if pos.is_valid() {
            Some(&mut self.grid[pos.row][pos.col])
        } else {
            None
        }
    }

    /// Extract word from path.
    pub fn extract_word(&self, path: &[Position]) -> String {
        path.iter()
            .filter_map(|p| self.get_cell(*p))
            .map(|c| c.letter)
            .collect()
    }

    /// Convert grid to JSON.
    pub fn grid_to_json(&self) -> serde_json::Value {
        let rows: Vec<serde_json::Value> = self
            .grid
            .iter()
            .map(|row| {
                let cells: Vec<serde_json::Value> =
                    row.iter().map(|c| c.to_json()).collect();
                serde_json::Value::Array(cells)
            })
            .collect();
        serde_json::Value::Array(rows)
    }

    /// Convert full game state to JSON snapshot.
    pub fn to_json(&self) -> serde_json::Value {
        let players: Vec<serde_json::Value> =
            self.turn_order
                .iter()
                .filter_map(|id| self.players.get(id))
                .map(|p| p.to_json())
                .collect();

        let spectators: Vec<serde_json::Value> =
            self.spectators.values().map(|s| s.to_json()).collect();

        let current_turn = self.current_player().map(|p| p.user_id.as_str());

        serde_json::json!({
            "game_id": self.id,
            "lobby_id": self.lobby_id,
            "status": self.status.as_str(),
            "grid": self.grid_to_json(),
            "players": players,
            "spectators": spectators,
            "current_turn": current_turn,
            "round": self.round,
            "max_rounds": self.max_rounds,
            "used_words": self.used_words.iter().collect::<Vec<_>>(),
            "timer_vote": self.timer_vote.to_json()
        })
    }
}

/// Game errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameError {
    InvalidStatus,
    AlreadyPlayer,
    NotPlayer,
    AlreadySpectator,
    NotSpectator,
    NotYourTurn,
    GameStarted,
    GameNotActive,
    NotEnoughPlayers,
    TooManyPlayers,
    WordUsed,
    InvalidPath,
    PathTooShort,
}

impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidStatus => write!(f, "Invalid game status for this action"),
            Self::AlreadyPlayer => write!(f, "Already a player in this game"),
            Self::NotPlayer => write!(f, "Not a player in this game"),
            Self::AlreadySpectator => write!(f, "Already spectating this game"),
            Self::NotSpectator => write!(f, "Not spectating this game"),
            Self::NotYourTurn => write!(f, "It's not your turn"),
            Self::GameStarted => write!(f, "Game has already started"),
            Self::GameNotActive => write!(f, "Game is not active"),
            Self::NotEnoughPlayers => write!(f, "Not enough players to start"),
            Self::TooManyPlayers => write!(f, "Too many players"),
            Self::WordUsed => write!(f, "Word has already been used"),
            Self::InvalidPath => write!(f, "Invalid tile path"),
            Self::PathTooShort => write!(f, "Path too short"),
        }
    }
}

impl std::error::Error for GameError {}

/// Game manager - tracks all active games.
#[derive(Debug, Default)]
pub struct GameManager {
    games: HashMap<String, Game>,
    /// Player ID to game ID
    player_index: HashMap<i64, String>,
    /// Spectator ID to game ID
    spectator_index: HashMap<i64, String>,
}

impl GameManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a game.
    pub fn add(&mut self, game: Game) {
        // Index players
        for player_id in game.players.keys() {
            self.player_index.insert(*player_id, game.id.clone());
        }
        self.games.insert(game.id.clone(), game);
    }

    /// Get a game.
    pub fn get(&self, game_id: &str) -> Option<&Game> {
        self.games.get(game_id)
    }

    /// Get a mutable game.
    pub fn get_mut(&mut self, game_id: &str) -> Option<&mut Game> {
        self.games.get_mut(game_id)
    }

    /// Get game for a player.
    pub fn get_for_player(&self, player_id: i64) -> Option<&Game> {
        self.player_index
            .get(&player_id)
            .and_then(|id| self.games.get(id))
    }

    /// Get mutable game for a player.
    pub fn get_for_player_mut(&mut self, player_id: i64) -> Option<&mut Game> {
        let id = self.player_index.get(&player_id)?.clone();
        self.games.get_mut(&id)
    }

    /// Get game for a spectator.
    pub fn get_for_spectator(&self, player_id: i64) -> Option<&Game> {
        self.spectator_index
            .get(&player_id)
            .and_then(|id| self.games.get(id))
    }

    /// Remove a game.
    pub fn remove(&mut self, game_id: &str) -> Option<Game> {
        let game = self.games.remove(game_id)?;

        // Clean up indexes
        for player_id in game.players.keys() {
            self.player_index.remove(player_id);
        }
        for spectator_id in game.spectators.keys() {
            self.spectator_index.remove(spectator_id);
        }

        Some(game)
    }

    /// Clean up finished games.
    pub fn cleanup_finished(&mut self) -> Vec<String> {
        let finished: Vec<String> = self
            .games
            .iter()
            .filter(|(_, g)| g.status.is_terminal())
            .map(|(id, _)| id.clone())
            .collect();

        for id in &finished {
            self.remove(id);
        }

        finished
    }

    /// Count active games.
    pub fn active_count(&self) -> usize {
        self.games.values().filter(|g| g.status.is_active()).count()
    }

    /// Total game count.
    pub fn count(&self) -> usize {
        self.games.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_grid() -> Grid {
        let default_cell = || GridCell::new('A');
        [
            [default_cell(), default_cell(), default_cell(), default_cell(), default_cell()],
            [default_cell(), default_cell(), default_cell(), default_cell(), default_cell()],
            [default_cell(), default_cell(), default_cell(), default_cell(), default_cell()],
            [default_cell(), default_cell(), default_cell(), default_cell(), default_cell()],
            [default_cell(), default_cell(), default_cell(), default_cell(), default_cell()],
        ]
    }

    fn make_player(id: i64, turn_order: u8) -> GamePlayer {
        GamePlayer::new(
            id,
            format!("{}", id * 1000),
            format!("Player{}", id),
            None,
            turn_order,
        )
    }

    #[test]
    fn test_game_new() {
        let game = Game::new("game-1".to_string(), "lobby-1".to_string(), make_grid());
        assert_eq!(game.status, GameStatus::Idle);
        assert_eq!(game.round, 1);
        assert!(game.players.is_empty());
    }

    #[test]
    fn test_game_add_players() {
        let mut game = Game::new("game-1".to_string(), "lobby-1".to_string(), make_grid());

        game.add_player(make_player(1, 0)).unwrap();
        game.add_player(make_player(2, 1)).unwrap();

        assert_eq!(game.player_count(), 2);
        assert!(game.has_player(1));
        assert!(game.has_player(2));
    }

    #[test]
    fn test_game_start() {
        let mut game = Game::new("game-1".to_string(), "lobby-1".to_string(), make_grid());
        game.add_player(make_player(1, 0)).unwrap();
        game.add_player(make_player(2, 1)).unwrap();

        game.start().unwrap();

        assert_eq!(game.status, GameStatus::InProgress);
        assert!(game.current_player_id().is_some());
    }

    #[test]
    fn test_game_turns() {
        let mut game = Game::new("game-1".to_string(), "lobby-1".to_string(), make_grid());
        game.add_player(make_player(1, 0)).unwrap();
        game.add_player(make_player(2, 1)).unwrap();
        game.start().unwrap();

        // Player 1's turn
        assert!(game.is_player_turn(1));
        assert!(!game.is_player_turn(2));

        // Advance
        game.advance_turn();

        // Player 2's turn
        assert!(!game.is_player_turn(1));
        assert!(game.is_player_turn(2));

        // Advance again - back to player 1, new round
        let (_, round) = game.advance_turn();
        assert!(game.is_player_turn(1));
        assert_eq!(round, 2);
    }

    #[test]
    fn test_game_word_tracking() {
        let mut game = Game::new("game-1".to_string(), "lobby-1".to_string(), make_grid());

        assert!(!game.is_word_used("TEST"));
        game.use_word("test");
        assert!(game.is_word_used("TEST"));
        assert!(game.is_word_used("test")); // Case insensitive
    }

    #[test]
    fn test_position_adjacency() {
        let p = Position::new(2, 2);

        // All 8 neighbors
        assert!(p.is_adjacent_to(&Position::new(1, 1)));
        assert!(p.is_adjacent_to(&Position::new(1, 2)));
        assert!(p.is_adjacent_to(&Position::new(1, 3)));
        assert!(p.is_adjacent_to(&Position::new(2, 1)));
        assert!(p.is_adjacent_to(&Position::new(2, 3)));
        assert!(p.is_adjacent_to(&Position::new(3, 1)));
        assert!(p.is_adjacent_to(&Position::new(3, 2)));
        assert!(p.is_adjacent_to(&Position::new(3, 3)));

        // Not adjacent
        assert!(!p.is_adjacent_to(&Position::new(2, 2))); // Same
        assert!(!p.is_adjacent_to(&Position::new(0, 0))); // Too far
        assert!(!p.is_adjacent_to(&Position::new(4, 4))); // Too far
    }

    #[test]
    fn test_letter_values() {
        assert_eq!(letter_value('A'), 1);
        assert_eq!(letter_value('E'), 1);
        assert_eq!(letter_value('D'), 2);
        assert_eq!(letter_value('B'), 3);
        assert_eq!(letter_value('K'), 5);
        assert_eq!(letter_value('X'), 8);
        assert_eq!(letter_value('Z'), 10);
    }
}
