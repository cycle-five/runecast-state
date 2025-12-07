//! Lobby state management.
//!
//! A lobby is a persistent container for players that can spawn games.
//! Players must be in a lobby to play together.

use std::collections::HashMap;

/// Maximum players per lobby.
pub const MAX_LOBBY_PLAYERS: usize = 6;

/// Lobby types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LobbyType {
    /// Tied to a Discord channel
    Channel,
    /// Custom lobby with shareable code
    Custom,
}

impl Default for LobbyType {
    fn default() -> Self {
        Self::Channel
    }
}

/// A player's state within a lobby.
#[derive(Debug, Clone)]
pub struct LobbyMember {
    /// Database player ID
    pub player_id: i64,

    /// Discord user ID
    pub user_id: String,

    /// Display name
    pub username: String,

    /// Avatar URL
    pub avatar_url: Option<String>,

    /// Whether player is ready to start
    pub is_ready: bool,

    /// Whether player is currently connected
    pub is_connected: bool,

    /// When player joined this lobby
    pub joined_at: chrono::DateTime<chrono::Utc>,
}

impl LobbyMember {
    pub fn new(
        player_id: i64,
        user_id: String,
        username: String,
        avatar_url: Option<String>,
    ) -> Self {
        Self {
            player_id,
            user_id,
            username,
            avatar_url,
            is_ready: false,
            is_connected: true,
            joined_at: chrono::Utc::now(),
        }
    }
}

/// Lobby state.
#[derive(Debug, Clone)]
pub struct Lobby {
    /// Unique lobby ID
    pub id: String,

    /// Lobby type
    pub lobby_type: LobbyType,

    /// Shareable code (for custom lobbies)
    pub code: Option<String>,

    /// Discord channel ID (for channel lobbies)
    pub channel_id: Option<String>,

    /// Discord guild ID
    pub guild_id: Option<String>,

    /// Members indexed by player_id
    members: HashMap<i64, LobbyMember>,

    /// Current host player ID
    pub host_id: Option<i64>,

    /// Maximum players allowed
    pub max_players: usize,

    /// Active game ID (if any)
    pub active_game_id: Option<String>,

    /// When lobby was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Lobby {
    /// Create a new channel lobby.
    pub fn new_channel(channel_id: String, guild_id: Option<String>) -> Self {
        let id = format!("channel-{}", channel_id);
        Self {
            id,
            lobby_type: LobbyType::Channel,
            code: None,
            channel_id: Some(channel_id),
            guild_id,
            members: HashMap::new(),
            host_id: None,
            max_players: MAX_LOBBY_PLAYERS,
            active_game_id: None,
            created_at: chrono::Utc::now(),
        }
    }

    /// Create a new custom lobby with code.
    pub fn new_custom(code: String) -> Self {
        let id = format!("custom-{}", code);
        Self {
            id,
            lobby_type: LobbyType::Custom,
            code: Some(code),
            channel_id: None,
            guild_id: None,
            members: HashMap::new(),
            host_id: None,
            max_players: MAX_LOBBY_PLAYERS,
            active_game_id: None,
            created_at: chrono::Utc::now(),
        }
    }

    /// Add a member to the lobby.
    pub fn add_member(&mut self, member: LobbyMember) -> Result<(), LobbyError> {
        if self.is_full() {
            return Err(LobbyError::Full);
        }

        if self.members.contains_key(&member.player_id) {
            return Err(LobbyError::AlreadyMember);
        }

        // First member becomes host (for custom lobbies)
        if self.host_id.is_none() && self.lobby_type == LobbyType::Custom {
            self.host_id = Some(member.player_id);
        }

        self.members.insert(member.player_id, member);
        Ok(())
    }

    /// Remove a member from the lobby.
    pub fn remove_member(&mut self, player_id: i64) -> Option<LobbyMember> {
        let member = self.members.remove(&player_id)?;

        // If host left, assign new host
        if self.host_id == Some(player_id) {
            self.host_id = self.members.keys().next().copied();
        }

        Some(member)
    }

    /// Get a member by player ID.
    pub fn get_member(&self, player_id: i64) -> Option<&LobbyMember> {
        self.members.get(&player_id)
    }

    /// Get a mutable member by player ID.
    pub fn get_member_mut(&mut self, player_id: i64) -> Option<&mut LobbyMember> {
        self.members.get_mut(&player_id)
    }

    /// Check if player is a member.
    pub fn has_member(&self, player_id: i64) -> bool {
        self.members.contains_key(&player_id)
    }

    /// Check if player is the host.
    pub fn is_host(&self, player_id: i64) -> bool {
        self.host_id == Some(player_id)
    }

    /// Set player ready state.
    pub fn set_ready(&mut self, player_id: i64, ready: bool) -> Result<(), LobbyError> {
        let member = self
            .members
            .get_mut(&player_id)
            .ok_or(LobbyError::NotMember)?;
        member.is_ready = ready;
        Ok(())
    }

    /// Set player connection state.
    pub fn set_connected(&mut self, player_id: i64, connected: bool) -> Result<(), LobbyError> {
        let member = self
            .members
            .get_mut(&player_id)
            .ok_or(LobbyError::NotMember)?;
        member.is_connected = connected;
        Ok(())
    }

    /// Get all members.
    pub fn members(&self) -> impl Iterator<Item = &LobbyMember> {
        self.members.values()
    }

    /// Get all member player IDs.
    pub fn member_ids(&self) -> impl Iterator<Item = i64> + '_ {
        self.members.keys().copied()
    }

    /// Get connected member player IDs.
    pub fn connected_member_ids(&self) -> impl Iterator<Item = i64> + '_ {
        self.members
            .iter()
            .filter(|(_, m)| m.is_connected)
            .map(|(id, _)| *id)
    }

    /// Get ready member player IDs.
    pub fn ready_member_ids(&self) -> impl Iterator<Item = i64> + '_ {
        self.members
            .iter()
            .filter(|(_, m)| m.is_ready)
            .map(|(id, _)| *id)
    }

    /// Count members.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Count connected members.
    pub fn connected_count(&self) -> usize {
        self.members.values().filter(|m| m.is_connected).count()
    }

    /// Count ready members.
    pub fn ready_count(&self) -> usize {
        self.members.values().filter(|m| m.is_ready).count()
    }

    /// Check if lobby is full.
    pub fn is_full(&self) -> bool {
        self.members.len() >= self.max_players
    }

    /// Check if lobby is empty.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Check if all members are ready.
    pub fn all_ready(&self) -> bool {
        !self.members.is_empty() && self.members.values().all(|m| m.is_ready)
    }

    /// Check if lobby has an active game.
    pub fn has_active_game(&self) -> bool {
        self.active_game_id.is_some()
    }

    /// Set the active game.
    pub fn set_active_game(&mut self, game_id: Option<String>) {
        self.active_game_id = game_id;
    }

    /// Transfer host to another player.
    pub fn transfer_host(&mut self, new_host_id: i64) -> Result<(), LobbyError> {
        if !self.members.contains_key(&new_host_id) {
            return Err(LobbyError::NotMember);
        }
        self.host_id = Some(new_host_id);
        Ok(())
    }

    /// Convert to JSON for sending to clients.
    pub fn to_json(&self) -> serde_json::Value {
        let members: Vec<serde_json::Value> = self
            .members
            .values()
            .map(|m| {
                serde_json::json!({
                    "user_id": m.user_id,
                    "username": m.username,
                    "avatar_url": m.avatar_url,
                    "is_ready": m.is_ready,
                    "is_connected": m.is_connected
                })
            })
            .collect();

        let host_user_id = self.host_id.and_then(|hid| {
            self.members.get(&hid).map(|m| m.user_id.clone())
        });

        serde_json::json!({
            "lobby_id": self.id,
            "lobby_type": match self.lobby_type {
                LobbyType::Channel => "channel",
                LobbyType::Custom => "custom",
            },
            "lobby_code": self.code,
            "channel_id": self.channel_id,
            "guild_id": self.guild_id,
            "players": members,
            "host_id": host_user_id,
            "max_players": self.max_players,
            "active_game_id": self.active_game_id
        })
    }
}

/// Lobby errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyError {
    Full,
    AlreadyMember,
    NotMember,
    NotHost,
    GameInProgress,
}

impl std::fmt::Display for LobbyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => write!(f, "Lobby is full"),
            Self::AlreadyMember => write!(f, "Already a member of this lobby"),
            Self::NotMember => write!(f, "Not a member of this lobby"),
            Self::NotHost => write!(f, "Not the lobby host"),
            Self::GameInProgress => write!(f, "A game is in progress"),
        }
    }
}

impl std::error::Error for LobbyError {}

/// Lobby manager - tracks all active lobbies.
#[derive(Debug, Default)]
pub struct LobbyManager {
    /// Lobbies by ID
    lobbies: HashMap<String, Lobby>,

    /// Channel ID to lobby ID mapping
    channel_index: HashMap<String, String>,

    /// Code to lobby ID mapping
    code_index: HashMap<String, String>,

    /// Player ID to lobby ID mapping
    player_index: HashMap<i64, String>,
}

impl LobbyManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a lobby.
    pub fn add(&mut self, lobby: Lobby) {
        if let Some(channel_id) = &lobby.channel_id {
            self.channel_index
                .insert(channel_id.clone(), lobby.id.clone());
        }
        if let Some(code) = &lobby.code {
            self.code_index.insert(code.clone(), lobby.id.clone());
        }
        self.lobbies.insert(lobby.id.clone(), lobby);
    }

    /// Get lobby by ID.
    pub fn get(&self, lobby_id: &str) -> Option<&Lobby> {
        self.lobbies.get(lobby_id)
    }

    /// Get mutable lobby by ID.
    pub fn get_mut(&mut self, lobby_id: &str) -> Option<&mut Lobby> {
        self.lobbies.get_mut(lobby_id)
    }

    /// Get lobby by channel ID.
    pub fn get_by_channel(&self, channel_id: &str) -> Option<&Lobby> {
        self.channel_index
            .get(channel_id)
            .and_then(|id| self.lobbies.get(id))
    }

    /// Get lobby by code.
    pub fn get_by_code(&self, code: &str) -> Option<&Lobby> {
        self.code_index
            .get(&code.to_uppercase())
            .and_then(|id| self.lobbies.get(id))
    }

    /// Get mutable lobby by code.
    pub fn get_by_code_mut(&mut self, code: &str) -> Option<&mut Lobby> {
        let id = self.code_index.get(&code.to_uppercase())?.clone();
        self.lobbies.get_mut(&id)
    }

    /// Get lobby for a player.
    pub fn get_for_player(&self, player_id: i64) -> Option<&Lobby> {
        self.player_index
            .get(&player_id)
            .and_then(|id| self.lobbies.get(id))
    }

    /// Get mutable lobby for a player.
    pub fn get_for_player_mut(&mut self, player_id: i64) -> Option<&mut Lobby> {
        let id = self.player_index.get(&player_id)?.clone();
        self.lobbies.get_mut(&id)
    }

    /// Find or create a channel lobby.
    pub fn find_or_create_channel(
        &mut self,
        channel_id: String,
        guild_id: Option<String>,
    ) -> &mut Lobby {
        if let Some(lobby_id) = self.channel_index.get(&channel_id).cloned() {
            self.lobbies.get_mut(&lobby_id).unwrap()
        } else {
            let lobby = Lobby::new_channel(channel_id, guild_id);
            let lobby_id = lobby.id.clone();
            self.add(lobby);
            self.lobbies.get_mut(&lobby_id).unwrap()
        }
    }

    /// Add player to a lobby.
    pub fn add_player(&mut self, lobby_id: &str, member: LobbyMember) -> Result<(), LobbyError> {
        // Check if already in a lobby
        if self.player_index.contains_key(&member.player_id) {
            return Err(LobbyError::AlreadyMember);
        }

        let lobby = self.lobbies.get_mut(lobby_id).ok_or(LobbyError::NotMember)?;
        let player_id = member.player_id;
        lobby.add_member(member)?;

        self.player_index.insert(player_id, lobby_id.to_string());
        Ok(())
    }

    /// Remove player from their lobby.
    pub fn remove_player(&mut self, player_id: i64) -> Option<(String, LobbyMember)> {
        let lobby_id = self.player_index.remove(&player_id)?;
        let lobby = self.lobbies.get_mut(&lobby_id)?;
        let member = lobby.remove_member(player_id)?;
        Some((lobby_id, member))
    }

    /// Remove a lobby entirely.
    pub fn remove(&mut self, lobby_id: &str) -> Option<Lobby> {
        let lobby = self.lobbies.remove(lobby_id)?;

        // Clean up indexes
        if let Some(channel_id) = &lobby.channel_id {
            self.channel_index.remove(channel_id);
        }
        if let Some(code) = &lobby.code {
            self.code_index.remove(code);
        }
        for member in lobby.members() {
            self.player_index.remove(&member.player_id);
        }

        Some(lobby)
    }

    /// Remove empty lobbies.
    pub fn cleanup_empty(&mut self) -> Vec<String> {
        let empty: Vec<String> = self
            .lobbies
            .iter()
            .filter(|(_, l)| l.is_empty())
            .map(|(id, _)| id.clone())
            .collect();

        for id in &empty {
            self.remove(id);
        }

        empty
    }

    /// Count lobbies.
    pub fn count(&self) -> usize {
        self.lobbies.len()
    }

    /// Get all lobby IDs.
    pub fn lobby_ids(&self) -> impl Iterator<Item = &String> {
        self.lobbies.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lobby_new() {
        let lobby = Lobby::new_channel("channel-123".to_string(), Some("guild-456".to_string()));
        assert_eq!(lobby.id, "channel-channel-123");
        assert!(lobby.is_empty());
        assert!(!lobby.is_full());
    }

    #[test]
    fn test_lobby_members() {
        let mut lobby = Lobby::new_custom("ABC123".to_string());

        let member = LobbyMember::new(1, "1000".to_string(), "Player1".to_string(), None);
        lobby.add_member(member).unwrap();

        assert_eq!(lobby.member_count(), 1);
        assert!(lobby.has_member(1));
        assert!(lobby.is_host(1)); // First member becomes host

        // Add another
        let member2 = LobbyMember::new(2, "2000".to_string(), "Player2".to_string(), None);
        lobby.add_member(member2).unwrap();

        assert_eq!(lobby.member_count(), 2);
        assert!(!lobby.is_host(2));
    }

    #[test]
    fn test_lobby_ready() {
        let mut lobby = Lobby::new_custom("ABC123".to_string());

        lobby
            .add_member(LobbyMember::new(
                1,
                "1000".to_string(),
                "P1".to_string(),
                None,
            ))
            .unwrap();
        lobby
            .add_member(LobbyMember::new(
                2,
                "2000".to_string(),
                "P2".to_string(),
                None,
            ))
            .unwrap();

        assert!(!lobby.all_ready());
        assert_eq!(lobby.ready_count(), 0);

        lobby.set_ready(1, true).unwrap();
        assert_eq!(lobby.ready_count(), 1);
        assert!(!lobby.all_ready());

        lobby.set_ready(2, true).unwrap();
        assert!(lobby.all_ready());
    }

    #[test]
    fn test_lobby_host_transfer() {
        let mut lobby = Lobby::new_custom("ABC123".to_string());

        lobby
            .add_member(LobbyMember::new(
                1,
                "1000".to_string(),
                "P1".to_string(),
                None,
            ))
            .unwrap();
        lobby
            .add_member(LobbyMember::new(
                2,
                "2000".to_string(),
                "P2".to_string(),
                None,
            ))
            .unwrap();

        assert!(lobby.is_host(1));

        // Host leaves
        lobby.remove_member(1);

        // Host transfers to remaining member
        assert!(lobby.is_host(2));
    }

    #[test]
    fn test_lobby_full() {
        let mut lobby = Lobby::new_custom("ABC123".to_string());

        for i in 0..MAX_LOBBY_PLAYERS {
            lobby
                .add_member(LobbyMember::new(
                    i as i64,
                    format!("{}", i * 1000),
                    format!("P{}", i),
                    None,
                ))
                .unwrap();
        }

        assert!(lobby.is_full());

        let result = lobby.add_member(LobbyMember::new(
            100,
            "100000".to_string(),
            "P100".to_string(),
            None,
        ));
        assert!(matches!(result, Err(LobbyError::Full)));
    }

    #[test]
    fn test_manager_basic() {
        let mut manager = LobbyManager::new();

        let lobby = Lobby::new_custom("ABC123".to_string());
        let lobby_id = lobby.id.clone();
        manager.add(lobby);

        assert!(manager.get(&lobby_id).is_some());
        assert!(manager.get_by_code("ABC123").is_some());
        assert!(manager.get_by_code("abc123").is_some()); // Case insensitive
    }

    #[test]
    fn test_manager_player_tracking() {
        let mut manager = LobbyManager::new();

        let lobby = Lobby::new_custom("ABC123".to_string());
        let lobby_id = lobby.id.clone();
        manager.add(lobby);

        let member = LobbyMember::new(1, "1000".to_string(), "P1".to_string(), None);
        manager.add_player(&lobby_id, member).unwrap();

        assert!(manager.get_for_player(1).is_some());
        assert_eq!(manager.get_for_player(1).unwrap().id, lobby_id);
    }

    #[test]
    fn test_manager_find_or_create() {
        let mut manager = LobbyManager::new();

        // First call creates
        let lobby1 = manager.find_or_create_channel("chan-1".to_string(), None);
        let id1 = lobby1.id.clone();

        // Second call finds
        let lobby2 = manager.find_or_create_channel("chan-1".to_string(), None);
        assert_eq!(lobby2.id, id1);
    }
}
