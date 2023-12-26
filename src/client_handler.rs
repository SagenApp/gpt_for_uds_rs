use tokio::net::UnixStream;
use std::error::Error;
use std::io::Cursor;
use futures::StreamExt;
use byteorder::{BigEndian, ReadBytesExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::gpt_client::{GptEngine, GptMessage, stream_gpt_response};

fn parse_json_to_vec(json_string: &str) -> Result<Vec<GptMessage>, serde_json::Error> {
    serde_json::from_str(json_string)
}

pub async fn handle_client(mut stream: UnixStream, gpt_engine: GptEngine) -> Result<(), Box<dyn Error>> {
    let message = read_string_from_stream(&mut stream).await?;

    let message_history = parse_json_to_vec(&message).map_err(|e| e.to_string())?;

    let mut response_stream = stream_gpt_response(gpt_engine, message_history).await?;

    while let Some(response) = response_stream.next().await {
        match response {
            Ok(response) => {
                write_string_to_stream(&mut stream, response).await?;
            }
            Err(e) => {
                eprintln!("Error generating response: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/**
 * Writes a string to the stream. The first 4 bytes of the stream are
 * interpreted as a u32 representing the length of the message to follow.
 */
async fn write_string_to_stream(stream: &mut UnixStream, response: String) -> Result<(), Box<dyn Error>> {
    let response_size = response.len() as u32;
    let mut buffer = Cursor::new(Vec::new());
    buffer.write_u32(response_size).await?;
    buffer.get_mut().extend_from_slice(response.as_bytes());

    stream.write_all(buffer.get_ref()).await?;
    stream.flush().await?;
    Ok(())
}

/**
 * Reads a string from the stream. The first 4 bytes of the stream are
 * interpreted as a u32 representing the length of the message to follow.
 */
async fn read_string_from_stream(mut stream: &mut UnixStream) -> Result<String, Box<dyn Error>> {
    let msg_length = read_message_length(&mut stream).await?;

    println!("Reading {} bytes...", msg_length);

    let mut msg_buf = vec![0; msg_length];
    stream.read_exact(&mut msg_buf).await?;

    let message = String::from_utf8_lossy(&msg_buf);
    println!("Received: {}", message);
    Ok(message.into_owned())
}

/**
 * Reads the first 4 bytes of the stream and interprets them as a u32
 * representing the length of the message to follow.
 */
async fn read_message_length(stream: &mut UnixStream) -> Result<usize, Box<dyn Error>> {
    let mut length_buf = [0u8; 4];
    stream.read_exact(&mut length_buf).await?;

    let msg_length = ReadBytesExt::read_u32::<BigEndian>(&mut Cursor::new(&length_buf))? as usize;
    Ok(msg_length)
}
