use std::ops::BitOr;

use bytes::{Bytes, BytesMut};
use nom::{
    bytes::complete::{tag, take},
    combinator::{all_consuming, map, map_opt, opt},
    sequence::{preceded, tuple},
    IResult, Parser,
};

#[derive(PartialEq, Debug, Clone)]
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

#[derive(PartialEq, Debug, Clone)]
pub struct Param(u8);
pub const APPLICATION1: Param = Param(0x21);
pub const APPLICATION2: Param = Param(0x22);
pub const OPTIONS1: Param = Param(0x30);
pub const OPTIONS1_NV: Param = Param(0x41);
pub const OPTIONS3: Param = Param(0x42);

#[derive(PartialEq, Debug, Clone)]
pub struct Level(u8);
pub const ON: Level = Level(0xff);
pub const OFF: Level = Level(0x0);

#[derive(PartialEq, Debug, Clone)]
pub struct Group(u8);

#[derive(PartialEq, Debug, Clone)]
pub struct Rate(u8);

#[derive(PartialEq, Debug, Clone)]
pub enum Message {
    SetParam(Param, Setting),
    SetVar(Group, Level, Rate),
    Reset,
    StopRamp(Group),
    Unrecognised(Bytes),
}
use Message::*;

fn hex_extract(raw: &[u8]) -> Option<u8> {
    let s = std::str::from_utf8(raw).ok()?;
    u8::from_str_radix(s, 16).ok()
}

pub fn hex_byte(input: &[u8]) -> IResult<&[u8], u8> {
    map_opt(take(2usize), hex_extract).parse(input)
}

pub fn message_from_parts(parts: (u8, u8, u8, Option<u8>)) -> Option<Message> {
    match parts {
        (0x79, group, _check, None) => Some(SetVar(Group(group), ON, Rate(0))),
        (0x01, group, _check, None) => Some(SetVar(Group(group), OFF, Rate(0))),
        (0x09, group, _check, None) => Some(StopRamp(Group(group))),
        (rate, group, level, Some(_check)) => Some(SetVar(Group(group), Level(level), Rate(rate))),
        _ => None,
    }
}

pub fn decode(bytes: Bytes) -> Message {
    let mut pattern = all_consuming(map_opt(
        preceded(
            tuple((tag("05"), take(2usize), tag("3800"))),
            tuple((hex_byte, hex_byte, hex_byte, opt(hex_byte))),
        ),
        message_from_parts,
    ));

    let result = pattern.parse(&bytes[..]);

    match result {
        Ok((_, mesg)) => mesg,
        _ => Unrecognised(bytes.clone()),
    }
}

pub fn encode(mesg: Message) -> Bytes {
    match mesg {
        SetVar(Group(g), Level(l), Rate(_s)) => Bytes::from(format!("\\05380002{g:02X}{l:02X}\r")),
        SetParam(Param(p), Setting(s)) => Bytes::from(format!("@A3{p:02x}00{s:02x}\r")),
        Reset => Bytes::from(b"~".as_ref()),
        _ => Bytes::new(),
    }
}

pub fn preamble() -> Bytes {
    let mut p = BytesMut::new();
    p.extend(encode(Reset));
    p.extend(encode(SetParam(OPTIONS3, LOCAL_SAL | EX_STAT)));
    p.extend(encode(SetParam(
        OPTIONS1,
        SMART | ID_MON | CONNECT | MONITOR,
    )));
    p.freeze()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setvar_on() {
        let m = decode(b"05003800790400".as_ref().into());
        assert_eq!(m, SetVar(Group(4), ON, Rate(0)))
    }

    #[test]
    fn setvar_off() {
        let m = decode(b"05003800010400".as_ref().into());
        assert_eq!(m, SetVar(Group(4), OFF, Rate(0)))
    }

    #[test]
    fn stop_ramp() {
        let m = decode(b"05003800090400".as_ref().into());
        assert_eq!(m, StopRamp(Group(4)))
    }

    #[test]
    fn setvar_level() {
        let m = decode(b"0500380024041F00".as_ref().into());
        assert_eq!(m, SetVar(Group(4), Level(0x1f), Rate(36)))
    }

    #[test]
    fn short_message() {
        assert_unrecognised(b"050038007904".as_ref().into());
    }

    #[test]
    fn empty_message() {
        assert_unrecognised(b"".as_ref().into());
    }

    #[test]
    fn long_message() {
        assert_unrecognised(b"0500380024041F00A1".as_ref().into());
    }

    #[test]
    fn odd_length() {
        assert_unrecognised(b"0500380079040".as_ref().into());
    }

    #[test]
    fn not_hex() {
        assert_unrecognised(b"0500380009z400".as_ref().into());
    }

    fn assert_unrecognised(bytes: Bytes) {
        let m = decode(bytes.clone());
        assert_eq!(m, Unrecognised(bytes))
    }
}
