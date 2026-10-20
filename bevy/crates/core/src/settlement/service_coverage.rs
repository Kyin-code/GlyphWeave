//! B-04 service-radius consumer. The radius does not place facilities; the
//! authored service programme does. It answers the narrower question the
//! morphology profile promises: are the realized dwellings inside the
//! catchment of at least one realized service?
use crate::worldgen::EntityInstance;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceCatchment {
    pub service_id: String,
    pub kind: String,
    pub world_x: i32,
    pub world_z: i32,
    pub radius_m: f64,
    pub dwellings_within_radius: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceCoverageAudit {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub radius_m: f64,
    pub services: Vec<ServiceCatchment>,
    pub dwellings: usize,
    pub covered_dwellings: usize,
    pub uncovered_entity_ids: Vec<String>,
    pub failures: Vec<String>,
}

fn service(e: &EntityInstance) -> bool {
    matches!(
        e.kind.as_str(),
        "school"
            | "church"
            | "temple"
            | "convenience_store"
            | "storefront"
            | "market"
            | "hospital"
            | "clinic"
            | "fire_station"
            | "town_hall"
    )
}

fn dwelling(e: &EntityInstance) -> bool {
    matches!(
        e.kind.as_str(),
        "residential_home"
            | "residential_block"
            | "residential_tower"
            | "apartment"
            | "townhouse"
    )
}

fn inside(service: &EntityInstance, dwelling: &EntityInstance, radius_m: f64) -> bool {
    let dx = f64::from(service.world_x - dwelling.world_x);
    let dz = f64::from(service.world_z - dwelling.world_z);
    dx.hypot(dz) <= radius_m + 1e-9
}

/// Deterministic Euclidean catchment audit. No facilities are moved, shrunk or
/// invented: an uncovered dwelling is named and a failed service record stays
/// visible in the report.
pub fn audit(entities: &[EntityInstance], radius_m: f64) -> Result<ServiceCoverageAudit, String> {
    if !radius_m.is_finite() || radius_m <= 0. {
        return Err("service coverage radius must be finite and positive".into());
    }
    let services: Vec<&EntityInstance> = entities.iter().filter(|e| service(e)).collect();
    let dwellings: Vec<&EntityInstance> = entities.iter().filter(|e| dwelling(e)).collect();
    let mut catchments = Vec::with_capacity(services.len());
    let mut covered = Vec::with_capacity(dwellings.len());
    for service in &services {
        let count = dwellings
            .iter()
            .filter(|dwelling| inside(service, dwelling, radius_m))
            .count();
        catchments.push(ServiceCatchment {
            service_id: service.entity_id.clone(),
            kind: service.kind.clone(),
            world_x: service.world_x,
            world_z: service.world_z,
            radius_m,
            dwellings_within_radius: count,
        });
    }
    for dwelling in &dwellings {
        if services
            .iter()
            .any(|service| inside(service, dwelling, radius_m))
        {
            covered.push(dwelling.entity_id.clone());
        }
    }
    let uncovered_entity_ids = dwellings
        .iter()
        .map(|dwelling| dwelling.entity_id.clone())
        .filter(|id| !covered.contains(id))
        .collect::<Vec<_>>();
    let status = if services.is_empty() || dwellings.is_empty() {
        "not-applicable"
    } else if uncovered_entity_ids.is_empty() {
        "pass"
    } else {
        "incomplete"
    };
    Ok(ServiceCoverageAudit {
        format: "glyphweave.service-coverage".into(),
        version: 1,
        status: status.into(),
        radius_m,
        services: catchments,
        dwellings: dwellings.len(),
        covered_dwellings: covered.len(),
        uncovered_entity_ids,
        failures: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(id: &str, kind: &str, x: i32, z: i32) -> EntityInstance {
        EntityInstance {
            entity_id: id.into(),
            asset_id: format!("prop.{kind}"),
            kind: kind.into(),
            world_x: x,
            world_z: z,
            world_y: 0,
            width_m: 10.,
            depth_m: 10.,
            height_m: 5.,
            scale: 1.,
            rotation_y_deg: 0.,
            grounding: Default::default(),
            anchors: Vec::new(),
            bounds: None,
        }
    }

    #[test]
    fn a_dwelling_outside_every_catchment_is_named() {
        let entities = vec![
            entity("school", "school", 0, 0),
            entity("home.near", "residential_home", 40, 0),
            entity("home.far", "residential_home", 200, 0),
        ];
        let audit = audit(&entities, 60.).unwrap();
        assert_eq!(audit.status, "incomplete");
        assert_eq!(audit.services[0].dwellings_within_radius, 1);
        assert_eq!(audit.uncovered_entity_ids, vec!["home.far"]);
    }

    #[test]
    fn all_dwellings_inside_a_catchment_pass() {
        let entities = vec![
            entity("clinic", "clinic", 10, 10),
            entity("home", "residential_home", 40, 10),
        ];
        let audit = audit(&entities, 50.).unwrap();
        assert_eq!(audit.status, "pass");
        assert!(audit.uncovered_entity_ids.is_empty());
    }

    #[test]
    fn no_services_is_not_reported_as_pass() {
        let entities = vec![entity("home", "residential_home", 0, 0)];
        assert_eq!(audit(&entities, 50.).unwrap().status, "not-applicable");
    }

    #[test]
    fn invalid_radius_is_refused() {
        assert!(audit(&[], 0.).is_err());
        assert!(audit(&[], f64::NAN).is_err());
    }
}
