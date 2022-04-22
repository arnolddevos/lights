use std::ops::BitOr;

use bytes::Bytes;
use nom::{
    bytes::complete::{tag, take, take_while_m_n},
    character::is_hex_digit,
    combinator::{map, map_opt, map_res, opt},
    error::Error,
    sequence::{preceded, tuple},
    AsBytes, IResult, Parser,
};

#[derive(PartialEq, Debug)]
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

#[derive(PartialEq, Debug)]
pub struct Param(u8);
pub const APPLICATION1: Param = Param(0x21);
pub const APPLICATION2: Param = Param(0x22);
pub const OPTIONS1: Param = Param(0x30);
pub const OPTIONS1_NV: Param = Param(0x41);
pub const OPTIONS3: Param = Param(0x42);

#[derive(PartialEq, Debug)]
pub struct Level(u8);
pub const ON: Level = Level(0xff);
pub const OFF: Level = Level(0x0);

#[derive(PartialEq, Debug)]
pub struct Group(u8);

#[derive(PartialEq, Debug)]
pub struct Rate(u8);

#[derive(PartialEq, Debug)]
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
        (0x09, group, _check, None) => StopRamp(Group(group)),
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

    let result = pattern.parse(&bytes[..]);

    match result {
        Ok((remainder, message)) if remainder.is_empty() => message,
        _ => Unrecognised,
    }
}

mod tests {
    use super::*;

    #[tokio::test]
    async fn setvar_on() {
        let m = decode(b"05003800790400".as_bytes().into());
        assert_eq!(m, SetVar(Group(4), ON, Rate(0)))
    }

    #[tokio::test]
    async fn setvar_off() {
        let m = decode(b"05003800010400".as_bytes().into());
        assert_eq!(m, SetVar(Group(4), OFF, Rate(0)))
    }

    #[tokio::test]
    async fn stop_ramp() {
        let m = decode(b"05003800090400".as_bytes().into());
        assert_eq!(m, StopRamp(Group(4)))
    }

    #[tokio::test]
    async fn setvar_level() {
        let m = decode(b"0500380024041F00".as_bytes().into());
        assert_eq!(m, SetVar(Group(4), Level(0x1f), Rate(36)))
    }

    #[tokio::test]
    async fn short_message() {
        let m = decode(b"050038007904".as_bytes().into());
        assert_eq!(m, Unrecognised)
    }

    #[tokio::test]
    async fn empty_message() {
        let m = decode(b"".as_bytes().into());
        assert_eq!(m, Unrecognised)
    }

    #[tokio::test]
    async fn long_message() {
        let m = decode(b"0500380024041F00A1".as_bytes().into());
        assert_eq!(m, Unrecognised)
    }

    #[tokio::test]
    async fn odd_length() {
        let m = decode(b"0500380079040".as_bytes().into());
        assert_eq!(m, Unrecognised)
    }

    #[tokio::test]
    async fn not_hex() {
        let m = decode(b"0500380009z400".as_bytes().into());
        assert_eq!(m, Unrecognised)
    }
}
