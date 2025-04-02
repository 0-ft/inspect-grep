use std::fmt;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use clap::ValueEnum;

#[derive(Debug, Serialize, Deserialize, Clone, ValueEnum, PartialEq)]
pub enum ChatMessageRole {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "tool")]
    Tool
}

impl std::fmt::Display for ChatMessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatMessageRole::System => write!(f, "system"),
            ChatMessageRole::User => write!(f, "user"),
            ChatMessageRole::Assistant => write!(f, "assistant"),
            ChatMessageRole::Tool => write!(f, "tool"),
        }
    }
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

#[derive(Debug)]
pub struct EvalSample {
    pub id: String,
    pub epoch: i64,
    pub messages: Vec<Option<ChatMessage>>,
}

// A struct that wraps a predicate function for filtering messages
pub struct FilteredEvalSampleDeserializer<F>
where
    F: Fn(&ChatMessage) -> bool,
{
    message_filter: F,
}

impl<F> FilteredEvalSampleDeserializer<F>
where
    F: Fn(&ChatMessage) -> bool,
{
    pub fn new(message_filter: F) -> Self {
        Self { message_filter }
    }
}

impl<'de, F> DeserializeSeed<'de> for FilteredEvalSampleDeserializer<F>
where
    F: Fn(&ChatMessage) -> bool,
{
    type Value = EvalSample;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Create a visitor that will filter messages during deserialization
        struct EvalSampleVisitor<F>(F);

        impl<'de, F> Visitor<'de> for EvalSampleVisitor<F>
        where
            F: Fn(&ChatMessage) -> bool,
        {
            type Value = EvalSample;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct EvalSample")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut id = None;
                let mut epoch = None;
                let mut messages = Vec::new();

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "id" => {
                            id = Some(map.next_value()?);
                        }
                        "epoch" => {
                            epoch = Some(map.next_value()?);
                        }
                        "messages" => {
                            // Use a custom visitor for the messages sequence
                            messages = map.next_value_seed(FilteredMessagesDeserializer(&self.0))?;
                        }
                        _ => {
                            // Skip unknown fields
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }

                let id = id.ok_or_else(|| de::Error::missing_field("id"))?;
                let epoch = epoch.ok_or_else(|| de::Error::missing_field("epoch"))?;

                Ok(EvalSample {
                    id,
                    epoch,
                    messages,
                })
            }
        }

        deserializer.deserialize_map(EvalSampleVisitor(self.message_filter))
    }
}

// The struct that will handle filtering messages during deserialization
struct FilteredMessagesDeserializer<'a, F>(&'a F)
where
    F: Fn(&ChatMessage) -> bool;

impl<'de, 'a, F> DeserializeSeed<'de> for FilteredMessagesDeserializer<'a, F>
where
    F: Fn(&ChatMessage) -> bool,
{
    type Value = Vec<Option<ChatMessage>>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MessagesVisitor<'a, F>(&'a F)
        where
            F: Fn(&ChatMessage) -> bool;

        impl<'de, 'a, F> Visitor<'de> for MessagesVisitor<'a, F>
        where
            F: Fn(&ChatMessage) -> bool,
        {
            type Value = Vec<Option<ChatMessage>>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a sequence of messages")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut messages = Vec::new();
                while let Some(message) = seq.next_element()? {
                    // Apply the filter predicate directly to the parsed ChatMessage
                    messages.push((self.0)(&message).then(||message));
                }

                Ok(messages)
            }
        }

        deserializer.deserialize_seq(MessagesVisitor(self.0))
    }
}
// Example usage:
pub fn deserialize_sample_filtered<R: std::io::Read>(
    reader: R,
    filter: impl Fn(&ChatMessage) -> bool,
) -> Result<EvalSample, serde_json::Error> {
    let deserializer = FilteredEvalSampleDeserializer::new(filter);
    let mut json_deserializer = serde_json::Deserializer::from_reader(reader);
    deserializer.deserialize(&mut json_deserializer)
}