#![allow(dead_code)]

use std::ops::BitOr;

use bytes::{Bytes, BytesMut};
use nom::{
    branch::alt,
    bytes::complete::{tag, take},
    combinator::{all_consuming, map_opt, opt},
    multi::many1,
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

static RAMP_CODES: [(u8, u16); 16] = [
    (0x02, 0),
    (0x0A, 4),
    (0x12, 8),
    (0x1a, 12),
    (0x22, 20),
    (0x2a, 30),
    (0x32, 40),
    (0x3a, 60),
    (0x42, 90),
    (0x4a, 120),
    (0x52, 180),
    (0x5a, 300),
    (0x62, 420),
    (0x6a, 600),
    (0x72, 900),
    (0x7a, 1020),
];

#[derive(PartialEq, Debug, Clone)]
pub struct Ramp(u16);

impl Ramp {
    pub fn decode(code: u8) -> Option<Ramp> {
        for (c, s) in RAMP_CODES {
            if c == code {
                return Some(Ramp(s));
            }
        }
        None
    }

    pub fn encode(&self) -> u8 {
        let secs = self.0;
        for (c, s) in RAMP_CODES {
            if secs <= s {
                return c;
            }
        }
        0x7a
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct Level(u8);
pub const ON: Level = Level(0xff);
pub const OFF: Level = Level(0x0);

#[derive(PartialEq, Debug, Clone)]
pub struct Group(u8);

#[derive(PartialEq, Debug, Clone)]
pub enum Message {
    SetParam(Param, Setting),
    SetVar(Group, Level, Ramp),
    Reset,
    StopRamp(Group),
    Status(Group, Vec<u8>),
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

pub fn command_from_parts(parts: (u8, u8, u8, Option<u8>)) -> Option<Message> {
    match parts {
        (0x79, group, _check, None) => Some(SetVar(Group(group), ON, Ramp(0))),
        (0x01, group, _check, None) => Some(SetVar(Group(group), OFF, Ramp(0))),
        (0x09, group, _check, None) => Some(StopRamp(Group(group))),
        (rate, group, level, Some(_check)) => {
            Some(SetVar(Group(group), Level(level), Ramp::decode(rate)?))
        }
        _ => None,
    }
}

pub fn status_from_parts(parts: (u8, Vec<u8>)) -> Option<Message> {
    match parts {
        (offset, mut status) => {
            if let Some(_check) = status.pop() {
                Some(Status(Group(offset), status))
            } else {
                None
            }
        }
    }
}

pub fn decode(bytes: Bytes) -> Message {
    let command_pattern = map_opt(
        preceded(
            tuple((tag("05"), take(2usize), tag("3800"))),
            tuple((hex_byte, hex_byte, hex_byte, opt(hex_byte))),
        ),
        command_from_parts,
    );

    let status_pattern = map_opt(
        preceded(
            tuple((
                tag("86"),
                take(4usize),
                tag("00"),
                take(2usize),
                tag("4038"),
            )),
            tuple((hex_byte, many1(hex_byte))),
        ),
        status_from_parts,
    );

    let mut pattern = all_consuming(alt((command_pattern, status_pattern)));

    let result = pattern.parse(&bytes[..]);

    match result {
        Ok((_, mesg)) => mesg,
        _ => Unrecognised(bytes.clone()),
    }
}

pub fn encode(mesg: Message) -> Bytes {
    match mesg {
        SetVar(Group(g), Level(l), Ramp(_s)) => Bytes::from(format!("\\05380002{g:02X}{l:02X}\r")),
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
        assert_eq!(m, SetVar(Group(4), ON, Ramp(0)))
    }

    #[test]
    fn setvar_off() {
        let m = decode(b"05003800010400".as_ref().into());
        assert_eq!(m, SetVar(Group(4), OFF, Ramp(0)))
    }

    #[test]
    fn stop_ramp() {
        let m = decode(b"05003800090400".as_ref().into());
        assert_eq!(m, StopRamp(Group(4)))
    }

    #[test]
    fn setvar_level() {
        let m = decode(b"050038002A041F00".as_ref().into());
        assert_eq!(m, SetVar(Group(4), Level(0x1f), Ramp(30)))
    }

    #[test]
    fn status_zero() {
        let m = decode(
            b"86081500F74038B000000000000000000000000000000000000000003E"
                .as_ref()
                .into(),
        );
        assert_eq!(m, Status(Group(176), vec![0; 20]));
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
