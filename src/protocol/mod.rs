use anyhow;

const MAGIC: &[u8] = &[0xde, 0xad, 0xc0, 0xde];

const PING: u32 = 1;
const PONG: u32 = 2;
const JOIN: u32 = 3;
const JOINED: u32 = 4;
const TALK: u32 = 5;
const TALKED: u32 = 6;
const ALIVE: u32 = 7;
const ALIVED: u32 = 8;

pub enum PacketType {
    Ping,
    Pong,
    Join { name: String },
    Joined { users: Vec<String> },
    Talk { audio_data: Vec<u8> },
    Talked { audio_data: Vec<u8> },
    Alive,
    Alived,
}

pub fn parse_from_client_packet(buf: &[u8]) -> anyhow::Result<PacketType> {
    if buf.len() < 8 {
        return Err(anyhow::format_err!("invalid packet"));
    }

    let magic = &buf[..4];
    if magic != MAGIC {
        return Err(anyhow::format_err!("invalid packet"));
    }

    let packet_type = u32::from_be_bytes(buf[4..8].try_into()?);
    let rest = &buf[8..];

    match packet_type {
        PING if rest.len() == 0 => Ok(PacketType::Ping),
        // PONG if rest.len() == 0 => Ok(PacketType::Pong),
        JOIN => {
            let Some(pos) = rest.iter().position(|&c| c == b'\0') else {
                return Err(anyhow::format_err!("invalid packet type"));
            };

            let name = std::str::from_utf8(&rest[..pos])?;
            Ok(PacketType::Join {
                name: name.to_string(),
            })
        }
        // JOINED => Ok(PacketType::Joined),
        TALK => Ok(PacketType::Talk {
            audio_data: rest.to_vec(),
        }),
        // TALKED => Ok(PacketType::Talked),
        ALIVE => Ok(PacketType::Alive),
        _ => Err(anyhow::format_err!("invalid packet type")),
    }
}

pub fn parse_from_server_packet(buf: &[u8]) -> anyhow::Result<PacketType> {
    if buf.len() < 8 {
        return Err(anyhow::format_err!("invalid packet: too small"));
    }

    if &buf[..4] != MAGIC {
        return Err(anyhow::format_err!("invalid magic"));
    }

    let packet_type = u32::from_be_bytes(buf[4..8].try_into()?);
    let rest = &buf[8..];

    match packet_type {
        PONG => {
            if rest.len() != 0 {
                return Err(anyhow::format_err!("invalid pong payload"));
            }
            Ok(PacketType::Pong)
        }

        JOINED => {
            let mut users = vec![];
            let mut start = 0;

            for i in 0..rest.len() {
                if rest[i] == 0 {
                    let name = String::from_utf8(rest[start..i].to_vec())?;
                    users.push(name);
                    start = i + 1;
                }
            }

            Ok(PacketType::Joined { users })
        }

        TALKED => {
            let audio_data = rest.to_vec();
            Ok(PacketType::Talked { audio_data })
        }

        ALIVED => {
            if rest.len() != 0 {
                return Err(anyhow::format_err!("invalid alived payload"));
            }
            Ok(PacketType::Alived)
        }

        _ => Err(anyhow::format_err!("unknown packet type")),
    }
}

pub fn new_ping() -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&PING.to_be_bytes());
    packet
}

pub fn new_pong() -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&PONG.to_be_bytes());
    packet
}

pub fn new_join(name: String) -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&JOIN.to_be_bytes());
    packet.extend_from_slice(name.as_bytes());
    packet.push(0); // null terminator like server expects
    packet
}

pub fn new_joined(users: Vec<String>) -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&JOINED.to_be_bytes());
    for user in users {
        packet.extend_from_slice(&user.as_bytes());
        packet.push(0);
    }

    packet
}

pub fn new_talk(audio_data: &[u8]) -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&TALK.to_be_bytes());
    packet.extend_from_slice(audio_data);
    packet
}

pub fn new_talked(audio_data: &[u8]) -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&TALKED.to_be_bytes());
    packet.extend_from_slice(audio_data);
    packet
}

pub fn new_alive() -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&ALIVE.to_be_bytes());
    packet
}

pub fn new_alived() -> Vec<u8> {
    let mut packet = MAGIC.to_vec();
    packet.extend_from_slice(&ALIVED.to_be_bytes());
    packet
}
