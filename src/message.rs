use serde::{Deserialize, Serialize}; // For JSON serialization and deserialization

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub message_type: String, // The type of message (e.g., "message", "join", "leave").
    pub username: Option<String>, // The username of the sender (None for system messages).
    pub content: String,      // The message content.
}
