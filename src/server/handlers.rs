// src/server/handlers.rs
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::protocol;
use crate::protocol::PacketType;
use crate::server::Server;

use super::model::{USER_TIMEOUT_SECS, User};

impl Server {
    pub async fn handle(&self, addr: SocketAddr, buf: &[u8]) -> anyhow::Result<()> {
        let packet_type = protocol::parse_from_client_packet(buf)?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        match packet_type {
            PacketType::Ping => {
                self.listener.send_to(&protocol::new_pong(), addr).await?;
            }
            PacketType::Rooms { mut offset } => {
                let mut remaining = false;
                let mut list = Vec::new();
                if offset == 0 {
                    offset = 1;
                }

                for i in offset..offset + 10 {
                    let room = match self.rooms.get(&i) {
                        Some(r) => r.value().clone(),
                        None => break,
                    };
                    list.push((i, room.name.clone()));
                }
                if self.rooms.len() as u16 >= offset + 10 {
                    remaining = true;
                }

                self.listener
                    .send_to(&protocol::new_rooms_list(remaining, list), addr)
                    .await?;
            }
            PacketType::Alive { seq: client_seq } => {
                if let Some(user_arc) = self.keepalive_user_arc(addr).await {
                    self.listener.send_to(&protocol::new_alived(), addr).await?;
                    if client_seq > 0 {
                        self.handle_alive_sync(addr, user_arc, client_seq).await;
                    }
                }
            }
            PacketType::Talk { audio_data } => {
                if let Some(user_arc) = self.keepalive_user_arc(addr).await {
                    let user_id = user_arc.id;
                    let room_id = user_arc.room_id.load(std::sync::atomic::Ordering::Relaxed);
                    let pkt = protocol::new_talked_audio(user_id, &audio_data);
                    self.batch_send_room(&pkt, room_id, Some(addr)).await;
                }
            }
            PacketType::Leave => {
                println!("User {addr} is leaving voluntarily.");
                self.disconnect_user(addr, None).await;
                return Ok(());
            }
            PacketType::Join {
                name,
                hwid,
                room_id,
            } => {
                if self.users.contains_key(&addr) {
                    return Ok(());
                }

                if let Err(e) = (self.try_join)(hwid).await {
                    self.disconnect_user(addr, Some(&e.to_string())).await;
                    return Err(e);
                };

                let user = Arc::new(User {
                    id: self
                        .next_user_id
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    name: name.clone(),
                    room_id: std::sync::atomic::AtomicU16::new(room_id),
                    last_seen: std::sync::atomic::AtomicU64::new(now + USER_TIMEOUT_SECS),
                    flags: 0,
                    consecutive_behind: std::sync::atomic::AtomicU8::new(0),
                });

                self.users.insert(addr, user.clone());
                {
                    let mut addrs = self.connected_addrs.write().await;
                    addrs.push(addr);
                }

                if let Some(room_arc) = self.rooms.get(&room_id).map(|r| r.value().clone()) {
                    room_arc.users.insert(addr, user.clone());
                    {
                        let mut snap = room_arc.joined_snapshot.write().await;
                        snap.push((user.id, user.name.clone()));
                    }
                    {
                        let mut addrs = room_arc.addr_list.write().await;
                        addrs.push(addr);
                    }
                }

                for room_ref in self.rooms.iter() {
                    let room_arc = room_ref.value().clone();
                    let users_snapshot = room_arc.joined_snapshot.read().await.clone();
                    let pkt = protocol::new_joined(*room_ref.key(), users_snapshot);
                    self.listener.send_to(&pkt, addr).await?;
                }

                let recipients: Vec<SocketAddr> = {
                    let addrs = self.connected_addrs.read().await;
                    addrs.iter().copied().collect()
                };

                self.broadcast_event(
                    |seq| protocol::new_event(seq, true, room_id, user.id, &name),
                    &recipients,
                )
                .await;

                let _ = self
                    .listener
                    .send_to(
                        &protocol::new_accepted(
                            self.event_system.read().await.next_seq - 1,
                            user.id,
                        ),
                        addr,
                    )
                    .await;
            }
            PacketType::Switch { room_id } => {
                if let Some(user_arc) = self.keepalive_user_arc(addr).await {
                    use std::sync::atomic::Ordering;
                    let user_id = user_arc.id;
                    let old_room_id = user_arc.room_id.load(Ordering::Relaxed);

                    if old_room_id == room_id {
                        return Ok(());
                    }

                    let Some(new_room_arc) = self.rooms.get(&room_id).map(|r| r.value().clone())
                    else {
                        return Ok(());
                    };

                    user_arc.room_id.store(room_id, Ordering::Relaxed);
                    let user_name = user_arc.name.clone();

                    if let Some(old_room_arc) =
                        self.rooms.get(&old_room_id).map(|r| r.value().clone())
                    {
                        old_room_arc.users.remove(&addr);
                        {
                            let mut snap = old_room_arc.joined_snapshot.write().await;
                            if let Some(pos) = snap.iter().position(|(id, _)| *id == user_id) {
                                snap.swap_remove(pos);
                            }
                        }
                        {
                            let mut addrs = old_room_arc.addr_list.write().await;
                            if let Some(pos) = addrs.iter().position(|a| *a == addr) {
                                addrs.swap_remove(pos);
                            }
                        }
                    }

                    new_room_arc.users.insert(addr, user_arc.clone());
                    {
                        let mut snap = new_room_arc.joined_snapshot.write().await;
                        snap.push((user_id, user_name.clone()));
                    }
                    {
                        let mut addrs = new_room_arc.addr_list.write().await;
                        addrs.push(addr);
                    }

                    let joined_users = new_room_arc.joined_snapshot.read().await.clone();
                    self.listener
                        .send_to(&protocol::new_joined(room_id, joined_users), addr)
                        .await?;

                    let recipients: Vec<SocketAddr> = {
                        let addrs = self.connected_addrs.read().await;
                        addrs.iter().copied().collect()
                    };

                    self.broadcast_event(
                        |seq| protocol::new_event(seq, false, old_room_id, user_id, &user_name),
                        &recipients,
                    )
                    .await;

                    self.broadcast_event(
                        |seq| protocol::new_event(seq, true, room_id, user_id, &user_name),
                        &recipients,
                    )
                    .await;
                }
            }
            _ => { /* ignore others for now */ }
        }

        Ok(())
    }
}
