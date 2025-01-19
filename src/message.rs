// message.rs
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ChatMessageType {
    Message,
    Join,
    Leave,
    Command(CommandType),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CommandType {
    List,
    Quit,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub message_type: ChatMessageType,
    pub username: Option<String>,
    pub content: String,
}
