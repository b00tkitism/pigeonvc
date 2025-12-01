// src/server/model.rs
use dashmap::DashMap;
use std::collections::VecDeque;
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU16, AtomicU64},
    },
};
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

pub const USER_TIMEOUT_SECS: u64 = 5;
pub const ROUTINE_SLEEP_MS: u64 = 500;
pub const MAX_EVENT_HISTORY: usize = 100;
pub const MAX_CONSECUTIVE_BEHIND: u8 = 3;

pub struct User {
    pub id: u64,
    pub name: String,
    pub last_seen: AtomicU64,
    pub room_id: AtomicU16,
    pub flags: u8,
    pub consecutive_behind: AtomicU8,
}

pub struct Room {
    pub name: String,
    pub users: DashMap<SocketAddr, Arc<User>>,
    pub joined_snapshot: RwLock<Vec<(u64, String)>>,
    pub addr_list: RwLock<Vec<SocketAddr>>,
}

#[derive(Clone)]
pub struct StoredEvent {
    pub seq: u64,
    pub data: Vec<u8>,
}

pub struct EventSystem {
    pub next_seq: u64,
    pub history: VecDeque<StoredEvent>,
}

pub struct Server {
    pub(crate) listener: Arc<UdpSocket>,
    pub(crate) rooms: DashMap<u16, Arc<Room>>,
    pub(crate) users: DashMap<SocketAddr, Arc<User>>,
    pub(crate) connected_addrs: RwLock<Vec<SocketAddr>>,
    pub(crate) next_user_id: AtomicU64,
    pub(crate) event_system: RwLock<EventSystem>,
}

impl Server {
    pub async fn new(listen_addr: String) -> anyhow::Result<Self> {
        let listener = UdpSocket::bind(listen_addr).await?;
        let listener = Arc::new(listener);

        let server = Self {
            listener,
            rooms: DashMap::new(),
            users: DashMap::new(),
            connected_addrs: RwLock::new(Vec::new()),
            next_user_id: AtomicU64::new(1),
            event_system: RwLock::new(EventSystem {
                next_seq: 1,
                history: VecDeque::with_capacity(MAX_EVENT_HISTORY),
            }),
        };

        Ok(server)
    }

    pub fn add_room(&self, name: &str) {
        self.rooms
            .insert(self.rooms.len() as u16, Server::make_room(name));
    }

    fn make_room(name: &str) -> Arc<Room> {
        Arc::new(Room {
            name: name.to_string(),
            users: DashMap::new(),
            joined_snapshot: RwLock::new(Vec::new()),
            addr_list: RwLock::new(Vec::new()),
        })
    }
}
