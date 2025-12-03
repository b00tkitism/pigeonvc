use std::sync::Arc;

use anyhow::Context;
use dashmap::DashMap;
use pigeonvc2::server::Server;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let state = Arc::new(DashMap::new());

    let db = SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename("pigeonvc.db")
            .create_if_missing(true),
    )
    .await
    .context("failed to connect to sqlite")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            hwid        TEXT NOT NULL UNIQUE,
            banned      INTEGER NOT NULL DEFAULT 0,
            created_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
            last_seen   DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )
    .execute(&db)
    .await
    .context("failed to create users table")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS rooms (
            id          INTEGER PRIMARY KEY,
            name        TEXT NOT NULL UNIQUE,
            description TEXT
        );
        "#,
    )
    .execute(&db)
    .await
    .context("failed to create rooms table")?;

    let (room_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM rooms")
        .fetch_one(&db)
        .await
        .context("failed to count rooms")?;

    if room_count == 0 {
        sqlx::query(
            r#"
            INSERT INTO rooms (name, description) VALUES
                ('Lobby',  'Default lobby'),
                ('Gaming', 'Gaming room'),
                ('Music',  'Music room');
            "#,
        )
        .execute(&db)
        .await
        .context("failed to insert default rooms")?;
    }

    let join_fn = {
        let db = db.clone();
        let state = state.clone();
        move |hwid: String| {
            let db = db.clone();
            let state = state.clone();
            async move {
                if let Some(_) = state.get(&hwid) {
                    anyhow::bail!("user with hwid `{hwid}` is already joined");
                }
                if let Some((banned,)) =
                    sqlx::query_as::<_, (i64,)>("SELECT banned FROM users WHERE hwid = ?")
                        .bind(&hwid)
                        .fetch_optional(&db)
                        .await
                        .context("failed to query user by hwid")?
                {
                    if banned != 0 {
                        anyhow::bail!("user with hwid `{hwid}` is banned");
                    }

                    sqlx::query("UPDATE users SET last_seen = CURRENT_TIMESTAMP WHERE hwid = ?")
                        .bind(&hwid)
                        .execute(&db)
                        .await
                        .context("failed to update last_seen")?;
                } else {
                    sqlx::query("INSERT INTO users (hwid) VALUES (?)")
                        .bind(&hwid)
                        .execute(&db)
                        .await
                        .context("failed to insert new user")?;
                }

                state.insert(hwid.clone(), 1);
                println!("join accepted for hwid = {hwid}");
                Ok(())
            }
        }
    };

    let disconnect_fn = {
        let state = state.clone();
        move |hwid: String| {
            let state = state.clone();
            async move {
                state.remove(&hwid);
                println!("{hwid} is leaving");
            }
        }
    };

    let srv = Arc::new(
        Server::new("0.0.0.0:8897".to_string(), join_fn, disconnect_fn)
            .await
            .context("failed to start UDP server")?,
    );

    let db_rooms: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM rooms ORDER BY id")
        .fetch_all(&db)
        .await
        .context("failed to load rooms from database")?;

    for (id, name) in db_rooms {
        let id_u16 = id as u16; // assuming your IDs are in 0..65535
        srv.add_room_with_id(id_u16, &name);
        println!("Loaded room {id_u16}: {name}");
    }

    {
        let srv_clone = srv.clone();
        tokio::spawn(async move {
            srv_clone.listen().await;
        });
    }

    {
        let srv_clone = srv.clone();
        tokio::spawn(async move {
            let _ = srv_clone.routine().await;
        });
    }

    println!("Server running on 0.0.0.0:8897 (press Ctrl+C to exit)");

    tokio::signal::ctrl_c().await?;
    println!("Shutting down...");

    Ok(())
}
