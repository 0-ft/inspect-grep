use std::str::FromStr;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, ValueEnum)]
pub enum ChatMessageRole {
    System,
    User,
    Assistant,
    Tool
}

// impl FromStr for ChatMessageRole {
//     type Err = anyhow::Error;

//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         Ok(match s {
//             "system" => ChatMessageRole::System,
//             "user" => ChatMessageRole::User,
//             "assistant" => ChatMessageRole::Assistant,
//             "tool" => ChatMessageRole::Tool,
//             _ => anyhow::bail!("Invalid chat message role: {}", s),
//         })
//     }
// }

// impl ToString for ChatMessageRole {
//     fn to_string(&self) -> String {
//         match self {
//             ChatMessageRole::System => "system".to_string(),
//             ChatMessageRole::User => "user".to_string(),
//             ChatMessageRole::Assistant => "assistant".to_string(),
//             ChatMessageRole::Tool => "tool".to_string(),
//         }
//     }
// }


#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatMessageRole,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EvalSample {
    id: serde_json::Value,
    epoch: i64,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvalDataset {
    pub name: String,
    pub sample_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvalLogConfig {
    pub epochs: u32,
    pub message_limit: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvalLogHeader {
    pub eval: EvalSpec,
    pub dataset: EvalDataset,
    pub config: EvalLogConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvalSpec {
    pub run_id: String,
    pub task: String,
}
