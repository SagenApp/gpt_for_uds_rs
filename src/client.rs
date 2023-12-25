use std::io::{self, Write};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[derive(Clone, Deserialize, Serialize)]
pub enum GptActor {
    System,
    User,
    Assistant
}

#[derive(Clone, Deserialize, Serialize)]
pub struct GptMessage {
    pub actor: GptActor,
    pub message: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut conversation: Vec<GptMessage> = Vec::new();
    let system_message = GptMessage {
        actor: GptActor::System,
        message: "You are a code expert, answer all the users questions to the best of your ability. Try to find bugs in your own statements while going through it.".to_string(),
    };
    conversation.push(system_message.clone());

    println!("\nSystem: {}\n", system_message.message);

    loop {
        handle_user_question(&mut conversation).await?;
    }
}

async fn handle_user_question(conversation: &mut Vec<GptMessage>) -> Result<(), Box<dyn std::error::Error>> {
    print!("User: ");
    io::stdout().flush().unwrap();
    let from_user = read_line_from_user()?;
    conversation.push(GptMessage {
        actor: GptActor::User,
        message: from_user,
    });

    let json_conversation = serialize_vec_to_json(&conversation)?;

    let mut stream = UnixStream::connect("/tmp/rust_uds_gpt4.sock").await?;

    // Write the length of the string as a 4 byte u32
    let len = json_conversation.len() as u32;
    stream.write_u32(len).await?;

    // Write the string
    stream.write_all(json_conversation.as_bytes()).await?;

    print!("\nAssistant: ");
    io::stdout().flush().unwrap();

    // Read responses until the connection is closed
    loop {
        // Read the length of the response
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {
                let len = u32::from_be_bytes(len_buf);

                // Read the response
                let mut response_buf = vec![0u8; len as usize];
                stream.read_exact(&mut response_buf).await?;

                let response = String::from_utf8(response_buf).unwrap();
                print!("{}", response);
                io::stdout().flush().unwrap();
            }
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                // Connection closed
                break;
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    println!("\n");

    Ok(())
}

fn serialize_vec_to_json(conversation: &Vec<GptMessage>) -> Result<String, String> {
    serde_json::to_string(conversation).map_err(|err| err.to_string())
}

fn read_line_from_user() -> Result<String, String> {
    let mut input_string = String::new();
    io::stdin().read_line(&mut input_string).map_err(|err| err.to_string())?;
    return Ok(input_string);
}
