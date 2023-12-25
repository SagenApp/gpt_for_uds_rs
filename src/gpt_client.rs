use std::pin::Pin;
use futures::{Stream, StreamExt};
use chatgpt::client::ChatGPT;
use chatgpt::config::{ChatGPTEngine, ModelConfigurationBuilder};
use chatgpt::prelude::Conversation;
use chatgpt::types::{ChatMessage, ResponseChunk};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub enum GptEngine {
    Gpt35Turbo(String),
    Gpt4(String),
    Gpt4_32k(String),
}

#[derive(Clone, Deserialize, Serialize)]
pub enum GptActor {
    System,
    User,
    Assistant
}

#[derive(Clone, Deserialize, Serialize)]
pub struct GptMessage {
    pub(crate) actor: GptActor,
    pub(crate) message: String,
}

fn get_client_engine(engine: GptEngine) -> ChatGPTEngine {
    match engine {
        GptEngine::Gpt35Turbo(_) => ChatGPTEngine::Gpt35Turbo,
        GptEngine::Gpt4(_) => ChatGPTEngine::Gpt4,
        GptEngine::Gpt4_32k(_) => ChatGPTEngine::Gpt4_32k,
    }
}

fn get_client_actor(actor: GptActor) -> chatgpt::types::Role {
    match actor {
        GptActor::System => chatgpt::types::Role::System,
        GptActor::User => chatgpt::types::Role::User,
        GptActor::Assistant => chatgpt::types::Role::Assistant,
    }
}

pub fn create_gpt_client(engine: GptEngine) -> Result<ChatGPT, String> {
    let (token, client_engine) = match engine {
        GptEngine::Gpt35Turbo(token) => (token, ChatGPTEngine::Gpt35Turbo),
        GptEngine::Gpt4(token) => (token, ChatGPTEngine::Gpt4),
        GptEngine::Gpt4_32k(token) => (token, ChatGPTEngine::Gpt4_32k),
    };

    let client = ChatGPT::new_with_config(
        token,
        ModelConfigurationBuilder::default()
            .temperature(1.0)
            .engine(client_engine)
            .build().map_err(|err| err.to_string())?,
    ).map_err(|err| err.to_string())?;
    Ok(client)
}

fn create_client_conversation(engine: GptEngine, history: Vec<GptMessage>) -> Result<Conversation, String> {
    let map = get_client_messages(history);
    let conversation = Conversation::new_with_history(create_gpt_client(engine)?, map);
    Ok(conversation)
}

pub async fn stream_gpt_response(
    engine: GptEngine,
    mut history: Vec<GptMessage>
) -> Result<Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>, String> {
    let (role, last_message) = match history.pop() {
        Some(message) => (message.actor, message.message),
        None => return Err("No messages in history!".to_string()),
    };
    let mut conversation = create_client_conversation(engine, history).map_err(|err| err.to_string())?;
    let response_stream = conversation.send_role_message_streaming(get_client_actor(role), last_message).await.map_err(|err| err.to_string())?;

    let stream = response_stream.filter_map(|chunk| {
        async move {
            match chunk {
                ResponseChunk::Content { delta, .. } => Some(Ok(delta)),
                _ => None,
            }
        }
    });

    Ok(Box::pin(stream))
}

fn get_client_messages(history: Vec<GptMessage>) -> Vec<ChatMessage> {
    history.iter().map(|message| ChatMessage {
        role: get_client_actor(message.clone().actor),
        content: message.message.clone()
    }).collect()
}
