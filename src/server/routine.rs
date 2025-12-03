// src/server/routine.rs
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::protocol;
use crate::server::Server;

use super::model::{ROUTINE_SLEEP_MS, USER_TIMEOUT_SECS, User};

impl Server {
    pub async fn routine(&self) -> anyhow::Result<()> {
        loop {
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let mut to_remove: Vec<SocketAddr> = Vec::new();

            for entry in self.users.iter() {
                let user = entry.value();
                let addr = *entry.key();
                let expires_at = user.last_seen.load(std::sync::atomic::Ordering::Relaxed);
                if expires_at <= now {
                    to_remove.push(addr);
                }
            }

            if !to_remove.is_empty() {
                for addr in to_remove.drain(..) {
                    println!("Removing inactive user {addr}");
                    self.disconnect_user(addr, Some("Inactivity timeout")).await;
                }
            }
            tokio::time::sleep(Duration::from_millis(ROUTINE_SLEEP_MS)).await;
        }
    }

    pub async fn keepalive_user_arc(&self, addr: SocketAddr) -> Option<Arc<User>> {
        if let Some(user_entry) = self.users.get(&addr) {
            let user_arc = user_entry.value().clone();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            user_arc.last_seen.store(
                now + USER_TIMEOUT_SECS,
                std::sync::atomic::Ordering::Relaxed,
            );
            return Some(user_arc);
        }
        None
    }

    pub async fn disconnect_user(&self, addr: SocketAddr, notify_reason: Option<&str>) {
        use std::sync::atomic::Ordering;

        if let Some(reason) = notify_reason {
            let pkt = protocol::new_disconnect(reason);
            let _ =
                tokio::time::timeout(Duration::from_millis(50), self.listener.send_to(&pkt, addr))
                    .await;
        }

        let Some((_, user_arc)) = self.users.remove(&addr) else {
            return;
        };

        let room_id = user_arc.room_id.load(Ordering::Relaxed);
        let user_id = user_arc.id;
        let user_name = user_arc.name.clone();

        if let Some(room_arc) = self.rooms.get(&room_id).map(|r| r.value().clone()) {
            room_arc.users.remove(&addr);
            {
                let mut snap = room_arc.joined_snapshot.write().await;
                if let Some(pos) = snap.iter().position(|(id, _)| *id == user_id) {
                    snap.swap_remove(pos);
                }
            }
            {
                let mut addrs = room_arc.addr_list.write().await;
                if let Some(pos) = addrs.iter().position(|a| *a == addr) {
                    addrs.swap_remove(pos);
                }
            }
        }

        {
            let mut addrs = self.connected_addrs.write().await;
            if let Some(pos) = addrs.iter().position(|a| *a == addr) {
                addrs.swap_remove(pos);
            }
        }

        let recipients: Vec<SocketAddr> = {
            let addrs = self.connected_addrs.read().await;
            addrs.iter().copied().collect()
        };

        self.broadcast_event(
            |seq| protocol::new_event(seq, false, room_id, user_id, &user_name),
            &recipients,
        )
        .await;

        if self.users.len() == 0 {
            let mut event = self.event_system.write().await;
            event.next_seq = 1;
            event.history.clear();
            self.next_user_id.store(0, Ordering::Relaxed);
        }

        (self.on_disconnect)(user_arc.hwid.clone()).await;
    }
}
