//! Explicit height payload contract. Raw i16 quarter-metres remain legacy.
//! GWHM v1 carries signed i32 millimetres on the same 1m horizontal grid.
use crate::baked_terrain::TerrainError;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeightEncoding {
    QuarterMetre,
    Millimetre,
}
pub const MAGIC: &[u8; 4] = b"GWHM";
pub const HEADER: usize = 16;
#[derive(Clone, Copy)]
pub struct HeightPayload<'a> {
    pub encoding: HeightEncoding,
    bytes: &'a [u8],
    pub count: usize,
}
impl<'a> HeightPayload<'a> {
    pub fn parse(bytes: &'a [u8], width: u32, depth: u32) -> Result<Self, TerrainError> {
        if width == 0 || depth == 0 || width > 512 || depth > 512 {
            return Err(TerrainError("invalid height payload dimensions".into()));
        }
        let count = width as usize * depth as usize;
        if bytes.len() == count * 2 {
            return Ok(Self {
                encoding: HeightEncoding::QuarterMetre,
                bytes,
                count,
            });
        }
        if bytes.len() != HEADER + count * 4 || bytes.get(..4) != Some(MAGIC.as_slice()) {
            return Err(TerrainError("height dimensions/payload mismatch".into()));
        }
        let get = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        if get(4) != 1 || get(8) != width || get(12) != depth {
            return Err(TerrainError(
                "unsupported/mismatched millimetre height header".into(),
            ));
        }
        Ok(Self {
            encoding: HeightEncoding::Millimetre,
            bytes: &bytes[HEADER..],
            count,
        })
    }
    pub fn raw(&self, i: usize) -> i32 {
        match self.encoding {
            HeightEncoding::QuarterMetre => i32::from(i16::from_le_bytes(
                self.bytes[i * 2..i * 2 + 2].try_into().unwrap(),
            )),
            HeightEncoding::Millimetre => {
                i32::from_le_bytes(self.bytes[i * 4..i * 4 + 4].try_into().unwrap())
            }
        }
    }
    pub fn metres(&self, i: usize) -> f64 {
        self.raw(i) as f64
            / match self.encoding {
                HeightEncoding::QuarterMetre => 4.,
                HeightEncoding::Millimetre => 1000.,
            }
    }
}
pub fn encode_millimetres(
    width: u32,
    depth: u32,
    metres: impl IntoIterator<Item = f64>,
) -> Result<Vec<u8>, TerrainError> {
    if width == 0 || depth == 0 || width > 512 || depth > 512 {
        return Err(TerrainError("invalid millimetre chunk dimensions".into()));
    }
    let mut bytes = Vec::with_capacity(HEADER + width as usize * depth as usize * 4);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&width.to_le_bytes());
    bytes.extend_from_slice(&depth.to_le_bytes());
    let mut count = 0;
    for h in metres {
        if count >= width as usize * depth as usize {
            return Err(TerrainError("too many millimetre height samples".into()));
        }
        let mm = (h * 1000.).round();
        if !mm.is_finite() || mm < i32::MIN as f64 || mm > i32::MAX as f64 {
            return Err(TerrainError(
                "height outside signed millimetre range".into(),
            ));
        }
        bytes.extend_from_slice(&(mm as i32).to_le_bytes());
        count += 1;
    }
    if count != width as usize * depth as usize {
        return Err(TerrainError("millimetre sample count mismatch".into()));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn millimetres_roundtrip_signed_samples_header_and_range() {
        let bytes = encode_millimetres(3, 1, [-1.234, 0.041, 255.999]).unwrap();
        assert_eq!(&bytes[..4], MAGIC);
        assert_eq!(bytes.len(), 28);
        let p = HeightPayload::parse(&bytes, 3, 1).unwrap();
        assert_eq!(p.encoding, HeightEncoding::Millimetre);
        assert_eq!(p.metres(0), -1.234);
        assert_eq!(p.metres(1), 0.041);
        assert_eq!(p.metres(2), 255.999);
        assert!(encode_millimetres(1, 1, [f64::NAN]).is_err());
        assert!(encode_millimetres(1, 1, [3_000_000.]).is_err());
        assert!(encode_millimetres(2, 1, [0.]).is_err());
        assert!(encode_millimetres(1, 1, [0., 1.]).is_err());
        assert!(HeightPayload::parse(&[], u32::MAX, u32::MAX).is_err());
        assert!(encode_millimetres(513, 1, [0.]).is_err());
    }
    #[test]
    fn headers_payload_count_and_legacy_signed_units_are_checked() {
        let legacy = [-5_i16, 3]
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect::<Vec<_>>();
        let p = HeightPayload::parse(&legacy, 2, 1).unwrap();
        assert_eq!(p.metres(0), -1.25);
        assert_eq!(p.metres(1), 0.75);
        let bytes = encode_millimetres(2, 1, [-1.25, 0.75]).unwrap();
        for at in [0, 4, 8, 12] {
            let mut bad = bytes.clone();
            bad[at] ^= 1;
            assert!(HeightPayload::parse(&bad, 2, 1).is_err());
        }
        assert!(HeightPayload::parse(&bytes[..bytes.len() - 1], 2, 1).is_err());
        assert!(HeightPayload::parse(&bytes, 1, 2).is_err());
    }
}
