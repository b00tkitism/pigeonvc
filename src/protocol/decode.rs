// src/protocol/decode.rs
use anyhow::{self, Result};

use crate::protocol::constants::*;
use crate::protocol::packet::PacketType;

pub fn parse_from_client_packet(buf: &[u8]) -> Result<PacketType> {
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
        JOIN => {
            let (name, rest) = take_cstring(rest)?;
            let (hwid, rest) = take_cstring(rest)?;

            Ok(PacketType::Join {
                name: name.to_string(),
                hwid: hwid.to_string(),
                room_id: u16::from_be_bytes(rest[..2].try_into()?),
            })
        }
        TALK => Ok(PacketType::Talk {
            audio_data: rest.to_vec(),
        }),
        ROOMS if rest.len() == 2 => Ok(PacketType::Rooms {
            offset: u16::from_be_bytes(rest[..2].try_into()?),
        }),
        SWITCH if rest.len() == 2 => Ok(PacketType::Switch {
            room_id: u16::from_be_bytes(rest[..2].try_into()?),
        }),
        ALIVE if rest.len() == 8 => Ok(PacketType::Alive {
            seq: u64::from_be_bytes(rest[..8].try_into()?),
        }),
        LEAVE => Ok(PacketType::Leave),
        _ => Err(anyhow::format_err!("invalid packet type")),
    }
}

pub fn parse_from_server_packet(buf: &[u8]) -> Result<PacketType> {
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

fn take_cstring(input: &[u8]) -> Result<(&str, &[u8])> {
    if let Some(pos) = input.iter().position(|&c| c == 0) {
        let (left, rest) = input.split_at(pos);
        let rest = &rest[1..]; // skip the null byte
        let s = std::str::from_utf8(left)?;
        Ok((s, rest))
    } else {
        Err(anyhow::anyhow!("invalid packet: missing null terminator"))
    }
}
