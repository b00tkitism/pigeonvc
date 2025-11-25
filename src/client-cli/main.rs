use pigeonvc::client::{Client, ClientCallbacks};
use tokio;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::new("127.0.0.1:8897".to_string()).await?;

    client
        .set_callbacks(ClientCallbacks {
            on_joined: Some(|users| {
                println!("Online users: {:?}", users);
            }),
            on_talk: Some(|audio| {
                println!("Received audio frame ({} bytes)", audio.len());
            }),
            on_error: Some(|err| {
                eprintln!("Client error: {err}");
            }),
        })
        .await;

    if client.validate_server().await? {
        println!("Server OK.");
    }

    client.join("mahdi2".to_string()).await?;

    client.start_keepalive().await;
    client.start_listener().await;

    // send audio loop
    loop {
        let frame = vec![1, 2, 3, 4]; // test
        client.send_audio(&frame).await?;
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}
