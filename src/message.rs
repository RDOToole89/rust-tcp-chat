// src/message.rs
use serde::{Deserialize, Serialize};

/// The "high-level" type of a chat message: message, join, leave, list, etc.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")] // so we get JSON like "message", "join" ...
pub enum ChatMessageType {
    Message,
    Join,
    Leave,
    List,
    Unknown,
}

/// Convert &str into ChatMessageType at runtime
impl From<&str> for ChatMessageType {
    fn from(s: &str) -> Self {
        match s {
            "message" => ChatMessageType::Message,
            "join" => ChatMessageType::Join,
            "leave" => ChatMessageType::Leave,
            "list" => ChatMessageType::List,
            _ => ChatMessageType::Unknown,
        }
    }
}

/// A generic message structure for client-server communication
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    /// Indicates what kind of message we have
    //  (Or you could store it as a String if you prefer: `pub message_type: String`)
    pub message_type: ChatMessageType,

    /// Which user sent it (None for system or events that don't specify a user)
    pub username: Option<String>,

    /// The text or content
    pub content: String,
}
