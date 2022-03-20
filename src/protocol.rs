use nom;
use nom::bytes::complete::{tag, take};
use nom::combinator::{map, map_res};
use nom::multi::{count, length_data};
use nom::number::streaming::{be_u16, be_u8};
use nom::sequence::tuple;
use nom::IResult;
use std::collections::HashMap;
use std::convert::TryInto;
use std::net::IpAddr;
use thiserror::Error;

#[derive(Debug)]
#[allow(dead_code)]
pub struct Header {
    pub length: u8,
    pub protocol_version: u8,
}
impl Header {
    pub fn header(input: &[u8]) -> IResult<&[u8], Header> {
        map(
            tuple((be_u8, tag("LSDP"), be_u8)),
            |(length, _, protocol_version)| Header {
                length,
                protocol_version,
            },
        )(input)
    }

    pub fn new() -> Header {
        Header {
            length: 6,
            protocol_version: 1,
        }
    }

    pub fn bytes(self) -> Vec<u8> {
        let mut r = Vec::new();
        r.push(self.length);
        r.extend_from_slice("LSDP".as_bytes());
        r.push(self.protocol_version);
        r
    }
}

#[derive(Debug, PartialEq)]
pub enum ClassID {
    Player = 0x0001,
    Server = 0x0002,
    SecondaryPlayer = 0x0003,
    Testing = 0x0004,
    All = 0xFFFF,

    Unimplemented = 0x0000,
}

impl ClassID {
    pub fn decode(input: &[u8]) -> IResult<&[u8], ClassID> {
        map(be_u16, |f| f.into())(input)
    }

    pub fn bytes(self) -> [u8; 2] {
        let v: u16 = self.into();
        v.to_ne_bytes()
    }
}

impl From<u16> for ClassID {
    fn from(v: u16) -> Self {
        match v {
            0x0001 => ClassID::Player,
            0x0002 => ClassID::Server,
            0x0003 => ClassID::SecondaryPlayer,
            0x0004 => ClassID::Testing,
            0xFFFF => ClassID::All,
            _ => ClassID::Unimplemented,
        }
    }
}
impl Into<u16> for ClassID {
    fn into(self) -> u16 {
        match self {
            ClassID::Player => 0x0001,
            ClassID::Server => 0x0002,
            ClassID::SecondaryPlayer => 0x0003,
            ClassID::Testing => 0x0004,
            ClassID::All => 0xFFFF,
            _ => 0x0000,
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct QueryMessage {
    pub classes: Vec<ClassID>,
}

impl QueryMessage {
    pub fn decode(input: &[u8]) -> IResult<&[u8], QueryMessage> {
        let (remain, num) = be_u8(input)?;
        let (remain, classes) = count(ClassID::decode, num.into())(remain)?;

        Ok((remain, QueryMessage { classes }))
    }

    pub fn bytes(self) -> Vec<u8> {
        let mut r = Vec::new();
        r.push(self.classes.len() as u8);
        for c in self.classes {
            r.extend_from_slice(&c.bytes())
        }
        r
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct AnnounceRecord<'a> {
    pub cid: ClassID,
    pub data: HashMap<&'a str, &'a str>,
}

impl<'a> AnnounceRecord<'a> {
    pub fn decode(input: &[u8]) -> IResult<&[u8], AnnounceRecord> {
        let (remain, cid) = ClassID::decode(input)?;
        let (remain, num) = be_u8(remain)?;

        let mut anrec = HashMap::new();

        let mut m = remain;
        for _ in 0..num {
            let (remain, (key, value)) = tuple((parse_str, parse_str))(m)?;
            anrec.insert(key, value);
            m = remain;
        }

        Ok((m, AnnounceRecord { cid, data: anrec }))
    }

    pub fn bytes(self) -> Vec<u8> {
        let mut r = Vec::new();
        r.extend_from_slice(&self.cid.bytes());
        r.push(self.data.len() as u8);
        for (key, value) in self.data {
            r.push(key.len() as u8);
            r.extend_from_slice(key.as_bytes());
            r.push(value.len() as u8);
            r.extend_from_slice(value.as_bytes());
        }
        r
    }
}
#[derive(Debug)]
#[allow(dead_code)]
pub struct AnnounceMessage<'a> {
    pub node_id: &'a [u8],
    pub addr: std::net::IpAddr,
    pub records: Vec<AnnounceRecord<'a>>,
}

fn parse_str(input: &[u8]) -> IResult<&[u8], &str> {
    map_res(length_data(be_u8), std::str::from_utf8)(input)
}

fn parse_ip(input: &[u8]) -> IResult<&[u8], std::net::IpAddr> {
    map_res(length_data(be_u8), to_ip)(input)
}

fn take_field(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let (input, length) = be_u8(input)?;
    take(length)(input)
}

pub fn to_ip(v: &[u8]) -> Result<IpAddr, IpParseError> {
    match v.len() {
        4 => {
            let m: [u8; 4] = v.try_into()?;
            Ok(m.into())
        }
        16 => {
            let m: [u8; 16] = v.try_into()?;
            Ok(m.into())
        }
        _ => Err(IpParseError::UnknownLength(v.len())),
    }
}

#[derive(Error, Debug)]
pub enum IpParseError {
    #[error("Length of IP field: `{0}` doesn't seem to be v4 or v6")]
    UnknownLength(usize),

    #[error(transparent)]
    IpConversionError(#[from] std::array::TryFromSliceError),
}

impl<'a> AnnounceMessage<'a> {
    pub fn decode(input: &[u8]) -> IResult<&[u8], AnnounceMessage> {
        //Get NodeID
        let (remain, nodeid) = take_field(input)?;

        //Get IP
        let (remain, address) = parse_ip(remain)?;

        //See how many records to read
        let (remain, num) = be_u8(remain)?;

        //Read
        let (remain, ar) = count(AnnounceRecord::decode, num.into())(remain)?;

        Ok((
            remain,
            AnnounceMessage {
                node_id: nodeid,
                addr: address,
                records: ar,
            },
        ))
    }

    pub fn bytes(self) -> Vec<u8> {
        let mut r = Vec::new();
        r.push(self.node_id.len() as u8);
        r.extend_from_slice(&self.node_id);
        let ip_bytes = match self.addr {
            IpAddr::V4(ip) => ip.octets().to_vec(),
            IpAddr::V6(ip) => ip.octets().to_vec(),
        };
        r.push(ip_bytes.len() as u8);
        r.extend_from_slice(&ip_bytes);
        r.push(self.records.len() as u8);

        for i in self.records {
            r.extend_from_slice(&i.bytes());
        }

        r
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct DeleteMessage<'a> {
    pub node_id: &'a [u8],
    pub classes: Vec<ClassID>,
}

impl<'a> DeleteMessage<'a> {
    pub fn decode(input: &[u8]) -> IResult<&[u8], DeleteMessage> {
        let (remain, node_id) = take_field(input)?;

        let (remain, num) = be_u8(remain)?;
        let (remain, classes) = count(ClassID::decode, num.into())(remain)?;

        Ok((remain, DeleteMessage { node_id, classes }))
    }
}

#[derive(Debug)]
pub enum MessageType<'a> {
    Announce(AnnounceMessage<'a>),
    Delete(DeleteMessage<'a>),
    BroadcastQuery(QueryMessage),
    UnicastQuery(QueryMessage),
    Unimplemented,
}

impl<'a> MessageType<'a> {
    pub fn decode(input: &[u8]) -> IResult<&[u8], MessageType> {
        let (i, (_, f)) = tuple((be_u8, be_u8))(input)?;
        let r = match f {
            0x41 => MessageType::Announce(AnnounceMessage::decode(i)?.1),
            0x44 => MessageType::Delete(DeleteMessage::decode(i)?.1),
            0x51 => MessageType::BroadcastQuery(QueryMessage::decode(i)?.1),
            0x52 => MessageType::UnicastQuery(QueryMessage::decode(i)?.1),
            _ => MessageType::Unimplemented,
        };
        Ok((i, r))
    }

    pub fn bytes(self) -> Vec<u8> {
        let t: u8 = self.mtype();
        let b = match self {
            MessageType::Announce(v) => v.bytes(),
            MessageType::BroadcastQuery(v) => v.bytes(),
            MessageType::UnicastQuery(v) => v.bytes(),
            _ => Vec::new(),
        };

        let mut r = Vec::new();
        r.push((b.len() as u8) + 2);
        r.push(t);
        r.extend_from_slice(&b);
        r
    }

    pub fn mtype(&self) -> u8 {
        match self {
            MessageType::Announce(_) => 0x41,
            MessageType::Delete(_) => 0x44,
            MessageType::BroadcastQuery(_) => 0x51,
            MessageType::UnicastQuery(_) => 0x52,
            _ => 0x00,
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct Packet<'a> {
    header: Header,
    pub message: MessageType<'a>,
}

impl<'a> Packet<'a> {
    pub fn decode(input: &[u8]) -> Result<Packet, nom::Err<nom::error::Error<&[u8]>>> {
        let (remain, header) = Header::header(input)?;
        let (_, message) = MessageType::decode(&remain)?;

        Ok(Packet { header, message })
    }

    pub fn new(message: MessageType) -> Packet {
        Packet {
            header: Header::new(),
            message,
        }
    }

    pub fn bytes(self) -> Vec<u8> {
        let mut r = Vec::new();
        r.extend_from_slice(&self.header.bytes());
        r.extend_from_slice(&self.message.bytes());
        r
    }
}

#[test]
fn announce() {
    let decoded_string = hex::decode(
        "064c534450016a41069056820e1\
        b00040a00012402000105046\
    e616d650a5345414c504c4159455204\
    706f7274053131303030056d6f64656\
    c04433338380776657273696f6e0633\
    2e31362e35027a730130000402046e6\
    16d650a5345414c504c415945520470\
    6f7274053131343331",
    )
    .unwrap();

    let p = Packet::decode(&decoded_string).unwrap();

    match p.message {
        MessageType::Announce(v) => {
            assert_eq!(v.node_id, [144, 86, 130, 14, 27, 0]);
            assert_eq!(v.addr, std::net::Ipv4Addr::new(10, 0, 1, 36));
            assert_eq!(v.records[0].cid, ClassID::Player);
            assert_eq!(
                v.records[0].data.get("name").unwrap().to_string(),
                "SEALPLAYER"
            );
            assert_eq!(v.records[1].cid, ClassID::Testing);
        }
        _ => {
            panic!("does not parse correctly");
        }
    }
}

#[test]
fn decode_encode() {
    let decoded_string = hex::decode("064c53445001055101ffff").unwrap();

    let p = Packet::decode(&decoded_string).unwrap();

    assert_eq!(decoded_string, p.bytes());
}

#[test]
fn decode_encode_complex() {
    let decoded_string = hex::decode(
        "064c534450016a41069056820e1\
        b00040a00012402000105046\
    e616d650a5345414c504c4159455204\
    706f7274053131303030056d6f64656\
    c04433338380776657273696f6e0633\
    2e31362e35027a730130000402046e6\
    16d650a5345414c504c415945520470\
    6f7274053131343331",
    )
    .unwrap();

    let p = Packet::decode(&decoded_string).unwrap();

    let b = p.bytes();

    let p2 = Packet::decode(&b).unwrap();

    match p2.message {
        MessageType::Announce(v) => {
            assert_eq!(v.node_id, [144, 86, 130, 14, 27, 0]);
            assert_eq!(v.addr, std::net::Ipv4Addr::new(10, 0, 1, 36));
            assert_eq!(v.records[0].cid, ClassID::Player);
            assert_eq!(
                v.records[0].data.get("name").unwrap().to_string(),
                "SEALPLAYER"
            );
            assert_eq!(v.records[1].cid, ClassID::Testing);
        }
        _ => {
            panic!("does not parse correctly");
        }
    }
}

#[test]
fn broadcast_query() {
    let decoded_string = hex::decode("064c53445001055101ffff").unwrap();

    let p = Packet::decode(&decoded_string).unwrap();

    match p.message {
        MessageType::BroadcastQuery(v) => {
            assert_eq!(v.classes[0], ClassID::All)
        }
        _ => {
            panic!("does not parse correctly");
        }
    }
}
