//! Shared integer parcel geometry for frontage selection and development.
//! The request supplies only explicit insets; building yaw and exact
//! dimensions remain authoritative and are never moved or resized here.
use super::EntityInstance;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParcelInsets {
    pub front_m: u32,
    pub side_m: u32,
    pub rear_m: u32,
}

impl Default for ParcelInsets {
    fn default() -> Self {
        Self::legacy()
    }
}

impl ParcelInsets {
    pub const fn legacy() -> Self {
        Self {
            front_m: 2,
            side_m: 2,
            rear_m: 2,
        }
    }

    pub const fn is_legacy(&self) -> bool {
        self.front_m == 2 && self.side_m == 2 && self.rear_m == 2
    }

    pub fn validate(self, width_m: u32, depth_m: u32) -> Result<(), String> {
        if self.front_m % 2 != 0
            || self.side_m % 2 != 0
            || self.rear_m % 2 != 0
            || self.front_m > 32
            || self.side_m > 32
            || self.rear_m > 64
        {
            return Err("parcel insets must be even and within front/side 32m, rear 64m".into());
        }
        let parcel_width = width_m
            .checked_add(self.side_m.checked_mul(2).ok_or("parcel width overflow")?)
            .ok_or("parcel width overflow")?;
        let parcel_depth = depth_m
            .checked_add(self.front_m)
            .and_then(|v| v.checked_add(self.rear_m))
            .ok_or("parcel depth overflow")?;
        if parcel_width == 0 || parcel_depth == 0 || parcel_width > 128 || parcel_depth > 128 {
            return Err("parcel dimensions must be nonzero and at most 128m".into());
        }
        Ok(())
    }

    pub fn validate_for_request(
        self,
        width_m: u32,
        depth_m: u32,
        setback_m: u32,
    ) -> Result<(), String> {
        self.validate(width_m, depth_m)?;
        if self.front_m > setback_m {
            return Err("parcel front inset exceeds request setback".into());
        }
        Ok(())
    }
}

/// Returns `[x0, z0, x1, z1]` in integer world metres. Front is the existing
/// cardinal entrance direction: north for yaw 0, west for 90, south for 180,
/// east for 270.
pub fn boundary(
    world_x: i32,
    world_z: i32,
    width_m: u32,
    depth_m: u32,
    rotation_y_deg: f32,
    insets: ParcelInsets,
) -> Result<[i64; 4], String> {
    if width_m == 0 || depth_m == 0 || width_m % 2 != 0 || depth_m % 2 != 0 {
        return Err("parcel building dimensions must be positive even metres".into());
    }
    insets.validate(width_m, depth_m)?;
    let x = i64::from(world_x);
    let z = i64::from(world_z);
    let hw = i64::from(width_m / 2);
    let hd = i64::from(depth_m / 2);
    let front = i64::from(insets.front_m);
    let side = i64::from(insets.side_m);
    let rear = i64::from(insets.rear_m);
    let result = match rotation_y_deg.round() as i32 {
        0 => [x - hw - side, z - hd - front, x + hw + side, z + hd + rear],
        90 => [x - hd - front, z - hw - side, x + hd + rear, z + hw + side],
        180 => [x - hw - side, z - hd - rear, x + hw + side, z + hd + front],
        270 => [x - hd - rear, z - hw - side, x + hd + front, z + hw + side],
        _ => return Err("parcel requires a cardinal building rotation".into()),
    };
    if result[2] <= result[0]
        || result[3] <= result[1]
        || result[2] - result[0] > 128
        || result[3] - result[1] > 128
        || result.iter().any(|v| *v < i64::from(i32::MIN) || *v > i64::from(i32::MAX))
    {
        return Err("parcel boundary is degenerate, exceeds 128m, or overflows i32".into());
    }
    Ok(result)
}

pub fn boundary_for_entity(e: &EntityInstance, insets: ParcelInsets) -> Result<[i64; 4], String> {
    if !e.width_m.is_finite()
        || !e.depth_m.is_finite()
        || e.width_m.fract() != 0.
        || e.depth_m.fract() != 0.
        || e.scale != 1.
    {
        return Err("parcel entity dimensions are not exact metre geometry".into());
    }
    boundary(
        e.world_x,
        e.world_z,
        e.width_m as u32,
        e.depth_m as u32,
        e.rotation_y_deg,
        insets,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::GroundingSpec;

    fn entity(x: i32, z: i32, rotation_y_deg: f32) -> EntityInstance {
        EntityInstance {
            entity_id: "building".into(),
            asset_id: "prop.residential_home".into(),
            kind: "residential_home".into(),
            world_x: x,
            world_z: z,
            world_y: 0,
            width_m: 12.,
            depth_m: 10.,
            height_m: 8.,
            scale: 1.,
            rotation_y_deg,
            grounding: GroundingSpec::default(),
            anchors: vec![],
            bounds: None,
        }
    }

    #[test]
    fn asymmetric_parcel_is_exact_for_all_cardinal_fronts() {
        let insets = ParcelInsets {
            front_m: 4,
            side_m: 4,
            rear_m: 12,
        };
        for (rotation, expected) in [
            (0., [-10, -9, 10, 17]),
            (90., [-9, -10, 17, 10]),
            (180., [-10, -17, 10, 9]),
            (270., [-17, -10, 9, 10]),
        ] {
            assert_eq!(boundary_for_entity(&entity(0, 0, rotation), insets).unwrap(), expected);
        }
        let p = boundary_for_entity(&entity(100, 200, 0.0), insets).unwrap();
        assert_eq!((p[2] - p[0]) * (p[3] - p[1]), 520);
    }

    #[test]
    fn legacy_and_explicit_two_metres_are_equivalent() {
        assert_eq!(ParcelInsets::default(), ParcelInsets::legacy());
        assert_eq!(
            boundary_for_entity(&entity(7, -3, 0.0), ParcelInsets::legacy()).unwrap(),
            boundary_for_entity(&entity(7, -3, 0.0), ParcelInsets::legacy()).unwrap()
        );
        let explicit = ParcelInsets {
            front_m: 2,
            side_m: 2,
            rear_m: 2,
        };
        assert!(explicit.is_legacy());
        assert_eq!(
            boundary_for_entity(&entity(7, -3, 90.0), explicit).unwrap(),
            boundary_for_entity(&entity(7, -3, 90.0), ParcelInsets::legacy()).unwrap()
        );
    }

    #[test]
    fn invalid_insets_and_overflow_fail_closed() {
        assert!(ParcelInsets {
            front_m: 3,
            side_m: 2,
            rear_m: 2
        }
        .validate(12, 10)
        .is_err());
        assert!(ParcelInsets {
            front_m: 4,
            side_m: 64,
            rear_m: 12
        }
        .validate(12, 10)
        .is_err());
        assert!(ParcelInsets {
            front_m: 4,
            side_m: 4,
            rear_m: 12
        }
        .validate_for_request(12, 10, 2)
        .is_err());
        assert!(boundary(i32::MAX, 0, 12, 10, 0., ParcelInsets::legacy()).is_err());
    }
}
