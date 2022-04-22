use std::ops::BitOr;

use bytes::Bytes;
use nom::{
    bytes::complete::{tag, take, take_while_m_n},
    character::is_hex_digit,
    combinator::{map, map_opt, map_res, opt},
    error::Error,
    sequence::{preceded, tuple},
    IResult, Parser,
};

#[derive(Debug)]
pub struct Setting(u8);

// Options 1
pub const CONNECT: Setting = Setting(1 << 0);
pub const SR_CHK: Setting = Setting(1 << 3);
pub const SMART: Setting = Setting(1 << 4);
pub const MONITOR: Setting = Setting(1 << 5);
pub const ID_MON: Setting = Setting(1 << 6);

// Options 3
pub const PARAM_CHANGE_NOTIFY: Setting = Setting(1 << 0);
pub const LOCAL_SAL: Setting = Setting(1 << 1);
pub const POWER_UP_NOTIFY: Setting = Setting(1 << 2);
pub const EX_STAT: Setting = Setting(1 << 3);

// Options can be combined
impl BitOr for Setting {
    type Output = Setting;

    fn bitor(self, rhs: Self) -> Self::Output {
        Setting(self.0 | rhs.0)
    }
}

#[derive(Debug)]
pub struct Param(u8);
pub const APPLICATION1: Param = Param(0x21);
pub const APPLICATION2: Param = Param(0x22);
pub const OPTIONS1: Param = Param(0x30);
pub const OPTIONS1_NV: Param = Param(0x41);
pub const OPTIONS3: Param = Param(0x42);

#[derive(Debug)]
pub struct Level(u8);
pub const ON: Level = Level(0xff);
pub const OFF: Level = Level(0x0);

#[derive(Debug)]
pub struct Group(u8);

#[derive(Debug)]
pub struct Rate(u8);

#[derive(Debug)]
pub enum Message {
    SetParam(Param, Setting),
    SetVar(Group, Level, Rate),
    Reset,
    StopRamp(Group),
    Unrecognised,
}
use Message::*;

fn hex_extract(raw: &[u8]) -> Option<u8> {
    let s = std::str::from_utf8(raw).ok()?;
    u8::from_str_radix(s, 16).ok()
}

pub fn hex_byte(input: &[u8]) -> IResult<&[u8], u8> {
    map_opt(take(2usize), hex_extract).parse(input)
}

pub fn message_from_parts(parts: (u8, u8, u8, Option<u8>)) -> Message {
    match parts {
        (0x79, group, _check, None) => SetVar(Group(group), ON, Rate(0)),
        (0x01, group, _check, None) => SetVar(Group(group), OFF, Rate(0)),
        (0x09, group, _check, None) => SetVar(Group(group), OFF, Rate(0)),
        (rate, group, level, Some(_check)) => SetVar(Group(group), Level(level), Rate(rate)),
        _ => Unrecognised,
    }
}

pub fn decode(bytes: Bytes) -> Message {
    let mut pattern = map(
        preceded(
            tuple((tag("05"), take(2usize), tag("3800"))),
            tuple((hex_byte, hex_byte, hex_byte, opt(hex_byte))),
        ),
        message_from_parts,
    );
    pattern
        .parse(&bytes[..])
        .map(|(_, x)| x)
        .unwrap_or(Unrecognised)
}
