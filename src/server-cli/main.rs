use std::sync::Arc;

use pigeonvc2::server::Server;

#[tokio::main]
async fn main() {
    let srv = match Server::new("0.0.0.0:8897".to_string()).await {
        Err(e) => {
            println!("{e}");
            return;
        }
        Ok(s) => Arc::new(s),
    };

    srv.add_room("Lobby");
    srv.add_room("Gaming");
    srv.add_room("Music");

    let srv_clone = srv.clone();
    tokio::spawn(async move { srv_clone.listen().await });

    let srv_clone = srv.clone();
    tokio::spawn(async move { srv_clone.routine().await });

    // Prevent main from exiting
    tokio::signal::ctrl_c().await.unwrap();
}
