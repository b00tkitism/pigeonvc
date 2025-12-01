// src/protocol/packet.rs
pub enum PacketType {
    Ping,
    Pong,
    Rooms {
        offset: u16,
    },
    RoomsList {
        remaining: bool,
        list: Vec<(u16, String)>,
    },
    Join {
        name: String,
        hwid: String,
        room_id: u16,
    },
    Joined {
        users: Vec<String>,
    },
    Talk {
        audio_data: Vec<u8>,
    },
    Talked {
        audio_data: Vec<u8>,
    },
    Event {
        joined: bool,
        room_id: u16,
        user_id: u64,
        name: String,
    },
    Switch {
        room_id: u16,
    },
    Alive {
        seq: u64,
    },
    Alived,
    Leave,
}
