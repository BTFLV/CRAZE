//! GDSII record layer: binary stream reading/writing and the KLayout "GDS2 text" format.
//!
//! Both formats are parsed into the same flat list of [`Record`]s. Everything above this
//! layer (structures, elements, flattening) is format independent.

use crate::error::{CrazeError, Result};

/// Record types used by CRAZE. Unknown records are preserved verbatim.
pub mod rt {
    pub const HEADER: u8 = 0x00;
    pub const BGNLIB: u8 = 0x01;
    pub const LIBNAME: u8 = 0x02;
    pub const UNITS: u8 = 0x03;
    pub const ENDLIB: u8 = 0x04;
    pub const BGNSTR: u8 = 0x05;
    pub const STRNAME: u8 = 0x06;
    pub const ENDSTR: u8 = 0x07;
    pub const BOUNDARY: u8 = 0x08;
    pub const PATH: u8 = 0x09;
    pub const SREF: u8 = 0x0A;
    pub const AREF: u8 = 0x0B;
    pub const TEXT: u8 = 0x0C;
    pub const LAYER: u8 = 0x0D;
    pub const DATATYPE: u8 = 0x0E;
    pub const WIDTH: u8 = 0x0F;
    pub const XY: u8 = 0x10;
    pub const ENDEL: u8 = 0x11;
    pub const SNAME: u8 = 0x12;
    pub const COLROW: u8 = 0x13;
    pub const NODE: u8 = 0x15;
    pub const TEXTTYPE: u8 = 0x16;
    pub const STRING: u8 = 0x19;
    pub const STRANS: u8 = 0x1A;
    pub const MAG: u8 = 0x1B;
    pub const ANGLE: u8 = 0x1C;
    pub const PATHTYPE: u8 = 0x21;
    pub const BOX: u8 = 0x2D;
    pub const BOXTYPE: u8 = 0x2E;
    pub const BGNEXTN: u8 = 0x30;
    pub const ENDEXTN: u8 = 0x31;
}

/// GDSII data type codes.
mod dt {
    pub const NONE: u8 = 0;
    pub const BITARRAY: u8 = 1;
    pub const I16: u8 = 2;
    pub const I32: u8 = 3;
    pub const REAL4: u8 = 4;
    pub const REAL8: u8 = 5;
    pub const ASCII: u8 = 6;
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecordData {
    None,
    BitArray(u16),
    I16(Vec<i16>),
    I32(Vec<i32>),
    Real(Vec<f64>),
    Ascii(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub rtype: u8,
    pub data: RecordData,
}

impl Record {
    pub fn new(rtype: u8, data: RecordData) -> Self {
        Self { rtype, data }
    }

    pub fn i16s(&self) -> &[i16] {
        match &self.data {
            RecordData::I16(v) => v,
            _ => &[],
        }
    }

    pub fn i32s(&self) -> &[i32] {
        match &self.data {
            RecordData::I32(v) => v,
            _ => &[],
        }
    }

    pub fn reals(&self) -> &[f64] {
        match &self.data {
            RecordData::Real(v) => v,
            _ => &[],
        }
    }

    pub fn text(&self) -> &str {
        match &self.data {
            RecordData::Ascii(s) => s,
            _ => "",
        }
    }

    pub fn bits(&self) -> u16 {
        match &self.data {
            RecordData::BitArray(b) => *b,
            RecordData::I16(v) => v.first().copied().unwrap_or(0) as u16,
            _ => 0,
        }
    }

    /// First integer value regardless of whether it was stored as i16 or i32.
    pub fn int(&self) -> i64 {
        match &self.data {
            RecordData::I16(v) => v.first().copied().unwrap_or(0) as i64,
            RecordData::I32(v) => v.first().copied().unwrap_or(0) as i64,
            RecordData::BitArray(b) => *b as i64,
            _ => 0,
        }
    }
}

// ---------------------------------------------------------------------------------------
// GDSII 8-byte real (excess-64, base-16)
// ---------------------------------------------------------------------------------------

pub fn decode_real8(b: [u8; 8]) -> f64 {
    let sign = if b[0] & 0x80 != 0 { -1.0 } else { 1.0 };
    let exp = (b[0] & 0x7F) as i32 - 64;
    let mut mant: u64 = 0;
    for &byte in &b[1..] {
        mant = (mant << 8) | byte as u64;
    }
    if mant == 0 {
        return 0.0;
    }
    sign * (mant as f64) / 2f64.powi(56) * 16f64.powi(exp)
}

pub fn encode_real8(v: f64) -> [u8; 8] {
    if v == 0.0 || !v.is_finite() {
        return [0; 8];
    }
    let sign: u8 = if v < 0.0 { 0x80 } else { 0 };
    let mut m = v.abs();
    let mut exp: i32 = 64;
    while m >= 1.0 {
        m /= 16.0;
        exp += 1;
    }
    while m < 1.0 / 16.0 {
        m *= 16.0;
        exp -= 1;
    }
    let mant = (m * 2f64.powi(56)).round() as u64;
    let mut out = [0u8; 8];
    out[0] = sign | (exp.clamp(0, 127) as u8);
    for i in 0..7 {
        out[7 - i] = ((mant >> (8 * i)) & 0xFF) as u8;
    }
    out
}

fn decode_real4(b: [u8; 4]) -> f64 {
    let sign = if b[0] & 0x80 != 0 { -1.0 } else { 1.0 };
    let exp = (b[0] & 0x7F) as i32 - 64;
    let mant = ((b[1] as u32) << 16) | ((b[2] as u32) << 8) | b[3] as u32;
    sign * (mant as f64) / 2f64.powi(24) * 16f64.powi(exp)
}

// ---------------------------------------------------------------------------------------
// Binary stream
// ---------------------------------------------------------------------------------------

/// Returns true if the bytes look like a binary GDSII stream (starts with a HEADER record).
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[2] == rt::HEADER && bytes[3] == dt::I16
}

pub fn read_binary(bytes: &[u8]) -> Result<Vec<Record>> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 4 <= bytes.len() {
        let len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        let rtype = bytes[pos + 2];
        let dtype = bytes[pos + 3];
        if len == 0 {
            // Zero padding after ENDLIB is allowed by the spec.
            break;
        }
        if len < 4 || pos + len > bytes.len() {
            return Err(CrazeError::Gds(format!(
                "corrupt record at byte {pos} (length {len})"
            )));
        }
        let payload = &bytes[pos + 4..pos + len];
        let data = match dtype {
            dt::NONE => RecordData::None,
            dt::BITARRAY => {
                RecordData::BitArray(if payload.len() >= 2 { u16::from_be_bytes([payload[0], payload[1]]) } else { 0 })
            }
            dt::I16 => RecordData::I16(payload.chunks_exact(2).map(|c| i16::from_be_bytes([c[0], c[1]])).collect()),
            dt::I32 => RecordData::I32(
                payload.chunks_exact(4).map(|c| i32::from_be_bytes([c[0], c[1], c[2], c[3]])).collect(),
            ),
            dt::REAL4 => RecordData::Real(payload.chunks_exact(4).map(|c| decode_real4([c[0], c[1], c[2], c[3]])).collect()),
            dt::REAL8 => RecordData::Real(
                payload.chunks_exact(8).map(|c| decode_real8(c.try_into().unwrap())).collect(),
            ),
            dt::ASCII => {
                let end = payload.iter().position(|&b| b == 0).unwrap_or(payload.len());
                RecordData::Ascii(String::from_utf8_lossy(&payload[..end]).into_owned())
            }
            other => return Err(CrazeError::Gds(format!("unknown data type {other} at byte {pos}"))),
        };
        let done = rtype == rt::ENDLIB;
        out.push(Record { rtype, data });
        pos += len;
        if done {
            break;
        }
    }
    Ok(out)
}

pub fn write_binary(records: &[Record]) -> Vec<u8> {
    let mut out = Vec::new();
    for r in records {
        let (dtype, payload): (u8, Vec<u8>) = match &r.data {
            RecordData::None => (dt::NONE, vec![]),
            RecordData::BitArray(b) => (dt::BITARRAY, b.to_be_bytes().to_vec()),
            RecordData::I16(v) => (dt::I16, v.iter().flat_map(|x| x.to_be_bytes()).collect()),
            RecordData::I32(v) => (dt::I32, v.iter().flat_map(|x| x.to_be_bytes()).collect()),
            RecordData::Real(v) => (dt::REAL8, v.iter().flat_map(|x| encode_real8(*x)).collect()),
            RecordData::Ascii(s) => {
                let mut b = s.as_bytes().to_vec();
                if b.len() % 2 == 1 {
                    b.push(0);
                }
                (dt::ASCII, b)
            }
        };
        let len = (payload.len() + 4) as u16;
        out.extend_from_slice(&len.to_be_bytes());
        out.push(r.rtype);
        out.push(dtype);
        out.extend_from_slice(&payload);
    }
    // Pad to a multiple of 2048 bytes like most tools do (tape block legacy, harmless).
    while out.len() % 2048 != 0 {
        out.push(0);
    }
    out
}

// ---------------------------------------------------------------------------------------
// KLayout GDS2 text format
// ---------------------------------------------------------------------------------------

fn text_record_spec(name: &str) -> Option<(u8, u8)> {
    Some(match name {
        "HEADER" => (rt::HEADER, dt::I16),
        "BGNLIB" => (rt::BGNLIB, dt::I16),
        "LIBNAME" => (rt::LIBNAME, dt::ASCII),
        "UNITS" => (rt::UNITS, dt::REAL8),
        "ENDLIB" => (rt::ENDLIB, dt::NONE),
        "BGNSTR" => (rt::BGNSTR, dt::I16),
        "STRNAME" => (rt::STRNAME, dt::ASCII),
        "ENDSTR" => (rt::ENDSTR, dt::NONE),
        "BOUNDARY" => (rt::BOUNDARY, dt::NONE),
        "PATH" => (rt::PATH, dt::NONE),
        "SREF" => (rt::SREF, dt::NONE),
        "AREF" => (rt::AREF, dt::NONE),
        "TEXT" => (rt::TEXT, dt::NONE),
        "LAYER" => (rt::LAYER, dt::I16),
        "DATATYPE" => (rt::DATATYPE, dt::I16),
        "WIDTH" => (rt::WIDTH, dt::I32),
        "XY" => (rt::XY, dt::I32),
        "ENDEL" => (rt::ENDEL, dt::NONE),
        "SNAME" => (rt::SNAME, dt::ASCII),
        "COLROW" => (rt::COLROW, dt::I16),
        "NODE" => (rt::NODE, dt::NONE),
        "TEXTTYPE" => (rt::TEXTTYPE, dt::I16),
        "STRING" => (rt::STRING, dt::ASCII),
        "STRANS" => (rt::STRANS, dt::BITARRAY),
        "MAG" => (rt::MAG, dt::REAL8),
        "ANGLE" => (rt::ANGLE, dt::REAL8),
        "PATHTYPE" => (rt::PATHTYPE, dt::I16),
        "BOX" => (rt::BOX, dt::NONE),
        "BOXTYPE" => (rt::BOXTYPE, dt::I16),
        "BGNEXTN" => (rt::BGNEXTN, dt::I32),
        "ENDEXTN" => (rt::ENDEXTN, dt::I32),
        _ => return None,
    })
}

/// Parses a `M/D/Y h:m:s` timestamp pair into 12 GDSII date fields.
fn parse_text_dates(rest: &str) -> Vec<i16> {
    let mut out = Vec::new();
    for tok in rest.split_whitespace() {
        if tok.contains('/') {
            let p: Vec<i16> = tok.split('/').filter_map(|s| s.parse().ok()).collect();
            if p.len() == 3 {
                out.extend_from_slice(&[p[2], p[0], p[1]]);
            }
        } else if tok.contains(':') {
            let p: Vec<i16> = tok.split(':').filter_map(|s| s.parse().ok()).collect();
            if p.len() == 3 {
                out.extend_from_slice(&p);
            }
        }
    }
    out.resize(12, 0);
    out
}

pub fn read_text(src: &str) -> Result<Vec<Record>> {
    let mut out: Vec<Record> = Vec::new();
    // XY coordinates may continue over several lines; collect until the next keyword.
    let mut xy_acc: Option<Vec<i32>> = None;

    let flush_xy = |acc: &mut Option<Vec<i32>>, out: &mut Vec<Record>| {
        if let Some(v) = acc.take() {
            out.push(Record::new(rt::XY, RecordData::I32(v)));
        }
    };

    for (lineno, raw) in src.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let first = line.split_whitespace().next().unwrap_or("");
        let is_keyword = first.chars().next().is_some_and(|c| c.is_ascii_alphabetic());

        if !is_keyword {
            match xy_acc.as_mut() {
                Some(acc) => {
                    parse_xy_pairs(line, acc).map_err(|e| CrazeError::Gds(format!("line {}: {e}", lineno + 1)))?
                }
                None => {
                    return Err(CrazeError::Gds(format!("line {}: unexpected data '{line}'", lineno + 1)));
                }
            }
            continue;
        }

        flush_xy(&mut xy_acc, &mut out);
        let rest = line[first.len()..].trim();
        let Some((rtype, dtype)) = text_record_spec(first) else {
            // Unknown keywords (PROPATTR, ...) are irrelevant for simulation.
            continue;
        };
        let err = |m: &str| CrazeError::Gds(format!("line {}: {m} in '{line}'", lineno + 1));
        let data = match (rtype, dtype) {
            (rt::XY, _) => {
                let mut acc = Vec::new();
                parse_xy_pairs(rest, &mut acc).map_err(|e| err(&e))?;
                xy_acc = Some(acc);
                continue;
            }
            (rt::BGNLIB | rt::BGNSTR, _) => RecordData::I16(parse_text_dates(rest)),
            (_, dt::NONE) => RecordData::None,
            (_, dt::ASCII) => RecordData::Ascii(rest.to_string()),
            (_, dt::BITARRAY) => {
                let v: i64 = rest.split_whitespace().next().unwrap_or("0").parse().map_err(|_| err("number expected"))?;
                RecordData::BitArray(v as i16 as u16)
            }
            (_, dt::I16) => RecordData::I16(
                rest.split_whitespace().map(|t| t.parse::<i16>()).collect::<std::result::Result<_, _>>().map_err(|_| err("integer expected"))?,
            ),
            (_, dt::I32) => RecordData::I32(
                rest.split_whitespace().map(|t| t.parse::<i32>()).collect::<std::result::Result<_, _>>().map_err(|_| err("integer expected"))?,
            ),
            (_, dt::REAL8) => RecordData::Real(
                rest.split_whitespace().map(|t| t.parse::<f64>()).collect::<std::result::Result<_, _>>().map_err(|_| err("number expected"))?,
            ),
            _ => RecordData::None,
        };
        out.push(Record::new(rtype, data));
    }
    flush_xy(&mut xy_acc, &mut out);
    if out.is_empty() {
        return Err(CrazeError::Gds("the file contains no GDSII data".into()));
    }
    Ok(out)
}

fn parse_xy_pairs(s: &str, acc: &mut Vec<i32>) -> std::result::Result<(), String> {
    // Format: "x: y" possibly several per line, separated by whitespace or ';'.
    let cleaned = s.replace(':', " ").replace(';', " ").replace(',', " ");
    for tok in cleaned.split_whitespace() {
        let v: f64 = tok.parse().map_err(|_| format!("invalid coordinate '{tok}'"))?;
        acc.push(v.round() as i32);
    }
    Ok(())
}
