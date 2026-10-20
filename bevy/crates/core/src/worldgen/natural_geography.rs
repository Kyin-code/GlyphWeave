//! Authored macro-geography expanded into deterministic terrain controls.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeographyControls {
    pub authored: bool,
    pub relief_scale: f32,
    pub uplift_bias: f32,
    pub orogeny_axis_rad: f32,
    pub coastality: f32,
    pub rainfall_bias: f32,
    pub seasonality: f32,
    pub wind_rad: f32,
    pub karst: f32,
    pub aeolian: f32,
    pub glacial: f32,
    pub historic_channel_shift_m: f32,
    pub wetland_bias: f32,
    pub incised_valleys: f32,
    pub land_pressure: f32,
    pub engineering_scale: f32,
}

impl Default for GeographyControls {
    fn default() -> Self {
        Self {
            authored: false,
            relief_scale: 1.0,
            uplift_bias: 0.5,
            orogeny_axis_rad: 0.0,
            coastality: 0.0,
            rainfall_bias: 0.5,
            seasonality: 0.4,
            wind_rad: 0.0,
            karst: 0.0,
            aeolian: 0.0,
            glacial: 0.0,
            historic_channel_shift_m: 0.0,
            wetland_bias: 0.0,
            incised_valleys: 0.5,
            land_pressure: 1.0,
            engineering_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Coast {
    #[serde(rename = "type")]
    pub coast_type: String,
    #[serde(default)]
    pub wave_exposure: String,
    #[serde(default)]
    pub trench: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Climate {
    pub band: String,
    pub prevailing_wind_deg: f32,
    #[serde(default)]
    pub monsoon: bool,
    pub rainfall_bias: f32,
    pub seasonality: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hydrology {
    pub regime: String,
    pub historic_channel_shift_m: f32,
    pub wetland_bias: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Landform {
    #[serde(default)]
    pub karst: bool,
    #[serde(default)]
    pub aeolian: bool,
    #[serde(default)]
    pub glacial: bool,
    pub incised_valleys: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Program {
    pub version: u32,
    pub setting: String,
    pub regional_relief_m: f32,
    pub orogeny_axis_deg: f32,
    pub uplift_bias: f32,
    pub coast: Coast,
    pub climate: Climate,
    pub hydrology: Hydrology,
    pub landform: Landform,
    #[serde(default)]
    pub hazards: Vec<String>,
    #[serde(default)]
    pub evidence: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub format: String,
    pub version: u32,
    pub status: String,
    pub setting: String,
    pub regional_relief_m: f32,
    pub orogeny_axis_deg: f32,
    pub uplift_bias: f32,
    pub coast_type: String,
    pub wave_exposure: String,
    pub trench: bool,
    pub climate_band: String,
    pub prevailing_wind_deg: f32,
    pub monsoon: bool,
    pub rainfall_bias: f32,
    pub seasonality: f32,
    pub hydrology_regime: String,
    pub historic_channel_shift_m: f32,
    pub wetland_bias: f32,
    pub karst: bool,
    pub aeolian: bool,
    pub glacial: bool,
    pub incised_valleys: f32,
    pub hazards: Vec<String>,
    pub controls: PlanControls,
    pub sequence: Vec<String>,
    pub evidence: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanControls {
    pub relief_scale: f32,
    pub uplift_bias: f32,
    pub orogeny_axis_deg: f32,
    pub coastality: f32,
    pub rainfall_bias: f32,
    pub seasonality: f32,
    pub wind_deg: f32,
    pub karst: f32,
    pub aeolian: f32,
    pub glacial: f32,
    pub historic_channel_shift_m: f32,
    pub wetland_bias: f32,
    pub incised_valleys: f32,
}

pub fn from_style(style: &Value) -> Result<Option<Program>, String> {
    let Some(value) = style.get("naturalGeography") else {
        return Ok(None);
    };
    let program: Program =
        serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
    validate(&program)?;
    Ok(Some(program))
}

pub fn validate_style(style: &Value) -> Result<(), String> {
    from_style(style).map(|_| ())
}

pub fn controls_for_style(style: &Value) -> Result<GeographyControls, String> {
    from_style(style)?.map_or_else(|| Ok(GeographyControls::default()), |program| Ok(controls(&program)))
}

pub fn compile(style: &Value) -> Result<Option<Plan>, String> {
    let Some(program) = from_style(style)? else {
        return Ok(None);
    };
    let controls = controls(&program);
    Ok(Some(Plan {
        format: "glyphweave.natural-geography-plan".into(),
        version: 1,
        status: "compiled".into(),
        setting: program.setting.clone(),
        regional_relief_m: program.regional_relief_m,
        orogeny_axis_deg: program.orogeny_axis_deg,
        uplift_bias: program.uplift_bias,
        coast_type: program.coast.coast_type.clone(),
        wave_exposure: program.coast.wave_exposure.clone(),
        trench: program.coast.trench,
        climate_band: program.climate.band.clone(),
        prevailing_wind_deg: program.climate.prevailing_wind_deg,
        monsoon: program.climate.monsoon,
        rainfall_bias: program.climate.rainfall_bias,
        seasonality: program.climate.seasonality,
        hydrology_regime: program.hydrology.regime.clone(),
        historic_channel_shift_m: program.hydrology.historic_channel_shift_m,
        wetland_bias: program.hydrology.wetland_bias,
        karst: program.landform.karst,
        aeolian: program.landform.aeolian,
        glacial: program.landform.glacial,
        incised_valleys: program.landform.incised_valleys,
        hazards: program.hazards.clone(),
        controls: PlanControls {
            relief_scale: controls.relief_scale,
            uplift_bias: controls.uplift_bias,
            orogeny_axis_deg: program.orogeny_axis_deg,
            coastality: controls.coastality,
            rainfall_bias: controls.rainfall_bias,
            seasonality: controls.seasonality,
            wind_deg: program.climate.prevailing_wind_deg,
            karst: controls.karst,
            aeolian: controls.aeolian,
            glacial: controls.glacial,
            historic_channel_shift_m: controls.historic_channel_shift_m,
            wetland_bias: controls.wetland_bias,
            incised_valleys: controls.incised_valleys,
        },
        sequence: vec![
            "regional-tectonics".into(),
            "climate-and-wind".into(),
            "hydrology-and-historic-channel-shift".into(),
            "landform-overprint".into(),
            "biome-and-hazard-field".into(),
            "natural-only-heightfield".into(),
        ],
        evidence: program.evidence,
    }))
}

fn validate(program: &Program) -> Result<(), String> {
    if program.version != 1
        || program.setting.is_empty()
        || !(50.0..=4000.0).contains(&program.regional_relief_m)
        || !program.orogeny_axis_deg.is_finite()
        || !program.uplift_bias.is_finite()
        || !(0.0..=1.0).contains(&program.uplift_bias)
        || !program.climate.prevailing_wind_deg.is_finite()
        || !(0.0..=1.0).contains(&program.climate.rainfall_bias)
        || !(0.0..=1.0).contains(&program.climate.seasonality)
        || !(0.0..=2000.0).contains(&program.hydrology.historic_channel_shift_m)
        || !(0.0..=1.0).contains(&program.hydrology.wetland_bias)
        || !(0.0..=1.0).contains(&program.landform.incised_valleys)
        || program.climate.band.is_empty()
        || program.hydrology.regime.is_empty()
    {
        return Err("invalid naturalGeography values".into());
    }
    let hazards = [
        "flood",
        "landslide",
        "earthquake",
        "storm-surge",
        "wildfire",
        "karst-collapse",
        "erosion",
        "drought",
        "tsunami",
    ];
    if program
        .hazards
        .iter()
        .any(|hazard| !hazards.contains(&hazard.as_str()))
    {
        return Err("unsupported naturalGeography hazard".into());
    }
    Ok(())
}

fn controls(program: &Program) -> GeographyControls {
    let coastality = match program.setting.as_str() {
        "coastal-mountain" | "coastal-bay" | "island" => 1.0,
        "river-delta" | "ria-bay" => 0.8,
        "inland-basin" => 0.15,
        _ => 0.45,
    };
    GeographyControls {
        authored: true,
        relief_scale: (program.regional_relief_m / 600.0).clamp(0.5, 2.0),
        uplift_bias: program.uplift_bias,
        orogeny_axis_rad: program.orogeny_axis_deg.to_radians(),
        coastality,
        rainfall_bias: program.climate.rainfall_bias,
        seasonality: program.climate.seasonality,
        wind_rad: program.climate.prevailing_wind_deg.to_radians(),
        karst: if program.landform.karst { 1.0 } else { 0.0 },
        aeolian: if program.landform.aeolian { 1.0 } else { 0.0 },
        glacial: if program.landform.glacial { 1.0 } else { 0.0 },
        historic_channel_shift_m: program.hydrology.historic_channel_shift_m,
        wetland_bias: program.hydrology.wetland_bias,
        incised_valleys: program.landform.incised_valleys,
        land_pressure: 1.0,
        engineering_scale: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn style() -> Value {
        json!({
            "naturalGeography": {
                "version": 1,
                "setting": "coastal-mountain",
                "regionalReliefM": 900,
                "orogenyAxisDeg": 28,
                "upliftBias": 0.72,
                "coast": {"type":"ria-bay","waveExposure":"moderate","trench":false},
                "climate": {
                    "band":"humid-subtropical",
                    "prevailingWindDeg":125,
                    "monsoon":true,
                    "rainfallBias":0.65,
                    "seasonality":0.48
                },
                "hydrology": {
                    "regime":"perennial",
                    "historicChannelShiftM":260,
                    "wetlandBias":0.3
                },
                "landform": {
                    "karst":false,
                    "aeolian":false,
                    "glacial":false,
                    "incisedValleys":0.68
                },
                "hazards":["flood","landslide"]
            }
        })
    }

    #[test]
    fn geography_compiles_into_deterministic_controls_and_sequence() {
        let plan = compile(&style()).unwrap().unwrap();
        assert_eq!(plan.setting, "coastal-mountain");
        assert_eq!(plan.coast_type, "ria-bay");
        assert_eq!(plan.sequence.last().unwrap(), "natural-only-heightfield");
        assert!(plan.controls.relief_scale > 1.0);
        assert_eq!(
            compile(&style()).unwrap().unwrap(),
            plan,
            "same authored geography must compile deterministically"
        );
    }

    #[test]
    fn invalid_geography_fails_closed() {
        let mut bad = style();
        bad["naturalGeography"]["climate"]["rainfallBias"] = json!(1.5);
        assert!(compile(&bad).is_err());
        let mut hazard = style();
        hazard["naturalGeography"]["hazards"] = json!(["meteor"]);
        assert!(compile(&hazard).is_err());
    }
}
