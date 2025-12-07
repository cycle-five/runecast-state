//! Connection state management.
//!
//! Tracks WebSocket connections and their associated metadata.
//! Handles reconnection with grace period.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Default grace period for reconnection (60 seconds).
pub const DEFAULT_RECONNECT_GRACE_PERIOD: Duration = Duration::from_secs(60);

/// Default heartbeat interval (30 seconds).
pub const DEFAULT_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// Default heartbeat timeout (45 seconds).
pub const DEFAULT_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(45);

/// Connection state for a single player.
#[derive(Debug, Clone)]
pub struct Connection {
    /// Player's database ID
    pub player_id: i64,

    /// Discord user ID (string to preserve precision)
    pub user_id: String,

    /// Display name
    pub username: String,

    /// Avatar URL
    pub avatar_url: Option<String>,

    /// Current connection status
    pub status: ConnectionStatus,

    /// When this connection was established
    pub connected_at: Instant,

    /// Last activity timestamp
    pub last_activity: Instant,

    /// Last heartbeat received
    pub last_heartbeat: Instant,

    /// Sequence number for message ordering
    pub send_seq: u64,

    /// Last acknowledged sequence from client
    pub ack_seq: u64,

    /// Messages pending acknowledgment (for replay on reconnect)
    pub pending_messages: Vec<PendingMessage>,

    /// Session token for reconnection
    pub session_token: String,

    /// Whether this connection is using envelope protocol
    pub uses_envelope: bool,
}

/// Connection status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Actively connected
    Connected,

    /// Disconnected, within grace period for reconnection
    Disconnected {
        since: Instant,
        grace_until: Instant,
    },

    /// Permanently disconnected (grace period expired)
    Expired,
}

impl ConnectionStatus {
    /// Check if currently connected.
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    /// Check if within reconnection grace period.
    pub fn is_reconnectable(&self) -> bool {
        match self {
            Self::Disconnected { grace_until, .. } => Instant::now() < *grace_until,
            _ => false,
        }
    }

    /// Check if connection has expired.
    pub fn is_expired(&self) -> bool {
        match self {
            Self::Expired => true,
            Self::Disconnected { grace_until, .. } => Instant::now() >= *grace_until,
            _ => false,
        }
    }
}

/// A message pending acknowledgment.
#[derive(Debug, Clone)]
pub struct PendingMessage {
    pub seq: u64,
    pub message: serde_json::Value,
    pub sent_at: Instant,
}

impl Connection {
    /// Create a new connection.
    pub fn new(
        player_id: i64,
        user_id: String,
        username: String,
        avatar_url: Option<String>,
        session_token: String,
    ) -> Self {
        let now = Instant::now();
        Self {
            player_id,
            user_id,
            username,
            avatar_url,
            status: ConnectionStatus::Connected,
            connected_at: now,
            last_activity: now,
            last_heartbeat: now,
            send_seq: 0,
            ack_seq: 0,
            pending_messages: Vec::new(),
            session_token,
            uses_envelope: false,
        }
    }

    /// Mark as disconnected with grace period.
    pub fn disconnect(&mut self) {
        self.disconnect_with_grace(DEFAULT_RECONNECT_GRACE_PERIOD);
    }

    /// Mark as disconnected with custom grace period.
    pub fn disconnect_with_grace(&mut self, grace_period: Duration) {
        let now = Instant::now();
        self.status = ConnectionStatus::Disconnected {
            since: now,
            grace_until: now + grace_period,
        };
    }

    /// Reconnect (restore Connected status).
    pub fn reconnect(&mut self) -> Result<Vec<PendingMessage>, &'static str> {
        match &self.status {
            ConnectionStatus::Connected => {
                // Already connected, just update activity
                self.last_activity = Instant::now();
                Ok(vec![])
            }
            ConnectionStatus::Disconnected { grace_until, .. } => {
                if Instant::now() < *grace_until {
                    self.status = ConnectionStatus::Connected;
                    self.last_activity = Instant::now();
                    self.last_heartbeat = Instant::now();
                    // Return pending messages for replay
                    Ok(self.pending_messages.clone())
                } else {
                    Err("Grace period expired")
                }
            }
            ConnectionStatus::Expired => Err("Connection expired"),
        }
    }

    /// Mark as expired.
    pub fn expire(&mut self) {
        self.status = ConnectionStatus::Expired;
        self.pending_messages.clear();
    }

    /// Record activity (any message received).
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Record heartbeat.
    pub fn heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
        self.last_activity = Instant::now();
    }

    /// Process acknowledgment from client.
    pub fn acknowledge(&mut self, ack: u64) {
        self.ack_seq = ack;
        // Remove acknowledged messages
        self.pending_messages.retain(|m| m.seq > ack);
    }

    /// Get next sequence number and record pending message.
    pub fn send(&mut self, message: serde_json::Value) -> u64 {
        self.send_seq += 1;
        self.pending_messages.push(PendingMessage {
            seq: self.send_seq,
            message,
            sent_at: Instant::now(),
        });
        self.send_seq
    }

    /// Check if heartbeat has timed out.
    pub fn is_heartbeat_timeout(&self) -> bool {
        self.status.is_connected()
            && self.last_heartbeat.elapsed() > DEFAULT_HEARTBEAT_TIMEOUT
    }

    /// Get time since last activity.
    pub fn idle_time(&self) -> Duration {
        self.last_activity.elapsed()
    }

    /// Get messages that need to be replayed on reconnect.
    pub fn messages_since(&self, seq: u64) -> Vec<&PendingMessage> {
        self.pending_messages.iter().filter(|m| m.seq > seq).collect()
    }
}

/// Connection manager - tracks all active connections.
#[derive(Debug, Default)]
pub struct ConnectionManager {
    /// Connections by player ID
    connections: HashMap<i64, Connection>,

    /// Session token to player ID mapping
    sessions: HashMap<String, i64>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new connection.
    pub fn add(&mut self, conn: Connection) {
        self.sessions
            .insert(conn.session_token.clone(), conn.player_id);
        self.connections.insert(conn.player_id, conn);
    }

    /// Get a connection by player ID.
    pub fn get(&self, player_id: i64) -> Option<&Connection> {
        self.connections.get(&player_id)
    }

    /// Get a mutable connection by player ID.
    pub fn get_mut(&mut self, player_id: i64) -> Option<&mut Connection> {
        self.connections.get_mut(&player_id)
    }

    /// Get a connection by session token.
    pub fn get_by_session(&self, token: &str) -> Option<&Connection> {
        self.sessions
            .get(token)
            .and_then(|pid| self.connections.get(pid))
    }

    /// Get a mutable connection by session token.
    pub fn get_by_session_mut(&mut self, token: &str) -> Option<&mut Connection> {
        self.sessions
            .get(token)
            .copied()
            .and_then(move |pid| self.connections.get_mut(&pid))
    }

    /// Remove a connection.
    pub fn remove(&mut self, player_id: i64) -> Option<Connection> {
        if let Some(conn) = self.connections.remove(&player_id) {
            self.sessions.remove(&conn.session_token);
            Some(conn)
        } else {
            None
        }
    }

    /// Mark a connection as disconnected.
    pub fn disconnect(&mut self, player_id: i64) {
        if let Some(conn) = self.connections.get_mut(&player_id) {
            conn.disconnect();
        }
    }

    /// Check for and expire timed-out connections.
    /// Returns list of expired player IDs.
    pub fn expire_stale(&mut self) -> Vec<i64> {
        let mut expired = Vec::new();

        for (player_id, conn) in &mut self.connections {
            if conn.status.is_expired() || conn.is_heartbeat_timeout() {
                conn.expire();
                expired.push(*player_id);
            }
        }

        // Remove expired
        for pid in &expired {
            if let Some(conn) = self.connections.remove(pid) {
                self.sessions.remove(&conn.session_token);
            }
        }

        expired
    }

    /// Get all connected player IDs.
    pub fn connected_players(&self) -> Vec<i64> {
        self.connections
            .iter()
            .filter(|(_, c)| c.status.is_connected())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get all player IDs (including disconnected within grace).
    pub fn all_players(&self) -> Vec<i64> {
        self.connections.keys().copied().collect()
    }

    /// Count connected players.
    pub fn connected_count(&self) -> usize {
        self.connections
            .values()
            .filter(|c| c.status.is_connected())
            .count()
    }

    /// Count total tracked players.
    pub fn total_count(&self) -> usize {
        self.connections.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_connection(player_id: i64) -> Connection {
        Connection::new(
            player_id,
            format!("{}", player_id * 1000),
            format!("Player{}", player_id),
            None,
            format!("session-{}", player_id),
        )
    }

    #[test]
    fn test_connection_new() {
        let conn = make_connection(1);
        assert!(conn.status.is_connected());
        assert_eq!(conn.send_seq, 0);
        assert_eq!(conn.ack_seq, 0);
    }

    #[test]
    fn test_connection_disconnect_reconnect() {
        let mut conn = make_connection(1);

        // Disconnect
        conn.disconnect();
        assert!(!conn.status.is_connected());
        assert!(conn.status.is_reconnectable());

        // Reconnect
        let pending = conn.reconnect().unwrap();
        assert!(conn.status.is_connected());
        assert!(pending.is_empty());
    }

    #[test]
    fn test_connection_expire() {
        let mut conn = make_connection(1);

        // Disconnect with zero grace
        conn.disconnect_with_grace(Duration::ZERO);

        // Should be expired
        assert!(conn.status.is_expired());
        assert!(conn.reconnect().is_err());
    }

    #[test]
    fn test_sequence_numbers() {
        let mut conn = make_connection(1);

        let seq1 = conn.send(serde_json::json!({"type": "test1"}));
        let seq2 = conn.send(serde_json::json!({"type": "test2"}));
        let seq3 = conn.send(serde_json::json!({"type": "test3"}));

        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);
        assert_eq!(seq3, 3);
        assert_eq!(conn.pending_messages.len(), 3);

        // Acknowledge first two
        conn.acknowledge(2);
        assert_eq!(conn.pending_messages.len(), 1);
        assert_eq!(conn.pending_messages[0].seq, 3);
    }

    #[test]
    fn test_reconnect_replay() {
        let mut conn = make_connection(1);

        conn.send(serde_json::json!({"type": "test1"}));
        conn.send(serde_json::json!({"type": "test2"}));

        // Disconnect
        conn.disconnect();

        // Reconnect - should get pending messages
        let pending = conn.reconnect().unwrap();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_manager_basic() {
        let mut manager = ConnectionManager::new();

        manager.add(make_connection(1));
        manager.add(make_connection(2));

        assert_eq!(manager.connected_count(), 2);
        assert!(manager.get(1).is_some());
        assert!(manager.get(3).is_none());
    }

    #[test]
    fn test_manager_session_lookup() {
        let mut manager = ConnectionManager::new();

        manager.add(make_connection(1));

        assert!(manager.get_by_session("session-1").is_some());
        assert!(manager.get_by_session("invalid").is_none());
    }

    #[test]
    fn test_manager_disconnect_remove() {
        let mut manager = ConnectionManager::new();

        manager.add(make_connection(1));
        manager.disconnect(1);

        // Still tracked
        assert!(manager.get(1).is_some());
        assert_eq!(manager.connected_count(), 0);

        // Remove
        manager.remove(1);
        assert!(manager.get(1).is_none());
    }
}
