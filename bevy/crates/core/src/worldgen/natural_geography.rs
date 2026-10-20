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
    pub river_density: f32,
    pub channel_sinuosity: f32,
    pub floodplain_width_m: f32,
    pub rain_shadow: f32,
    pub tectonic_uplift: f32,
    pub tectonic_subsidence: f32,
    pub fluvial_incision: f32,
    pub fluvial_deposition: f32,
    pub coastal_erosion: f32,
    pub aeolian_erosion: f32,
    pub aeolian_deposition: f32,
    pub karst_strength: f32,
    pub glacial_strength: f32,
    pub mass_wasting: f32,
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
            river_density: 0.5,
            channel_sinuosity: 1.0,
            floodplain_width_m: 0.0,
            rain_shadow: 0.0,
            tectonic_uplift: 0.0,
            tectonic_subsidence: 0.0,
            fluvial_incision: 0.0,
            fluvial_deposition: 0.0,
            coastal_erosion: 0.0,
            aeolian_erosion: 0.0,
            aeolian_deposition: 0.0,
            karst_strength: 0.0,
            glacial_strength: 0.0,
            mass_wasting: 0.0,
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
    #[serde(default)]
    pub shelf_width_m: Option<f32>,
    #[serde(default)]
    pub tidal_range_m: Option<f32>,
    #[serde(default)]
    pub erosion_bias: Option<f32>,
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
    #[serde(default)]
    pub rain_shadow: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hydrology {
    pub regime: String,
    pub historic_channel_shift_m: f32,
    pub wetland_bias: f32,
    #[serde(default)]
    pub drainage_pattern: Option<String>,
    #[serde(default)]
    pub river_density: Option<f32>,
    #[serde(default)]
    pub channel_sinuosity: Option<f32>,
    #[serde(default)]
    pub floodplain_width_m: Option<f32>,
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
    #[serde(default)]
    pub karst_strength: Option<f32>,
    #[serde(default)]
    pub aeolian_strength: Option<f32>,
    #[serde(default)]
    pub glacial_strength: Option<f32>,
    #[serde(default)]
    pub mass_wasting: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Process {
    pub kind: String,
    pub strength: f32,
    #[serde(default)]
    pub axis_deg: Option<f32>,
    #[serde(default)]
    pub note: Option<String>,
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
    pub processes: Vec<Process>,
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
    pub drainage_pattern: String,
    pub river_density: f32,
    pub channel_sinuosity: f32,
    pub floodplain_width_m: f32,
    pub rain_shadow: f32,
    pub hazards: Vec<String>,
    pub geomorphic_processes: Vec<Process>,
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
    pub river_density: f32,
    pub channel_sinuosity: f32,
    pub floodplain_width_m: f32,
    pub rain_shadow: f32,
    pub tectonic_uplift: f32,
    pub tectonic_subsidence: f32,
    pub fluvial_incision: f32,
    pub fluvial_deposition: f32,
    pub coastal_erosion: f32,
    pub aeolian_erosion: f32,
    pub aeolian_deposition: f32,
    pub karst_strength: f32,
    pub glacial_strength: f32,
    pub mass_wasting: f32,
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
    from_style(style)?.map_or_else(
        || Ok(GeographyControls::default()),
        |program| {
            let processes = effective_processes(&program);
            Ok(controls(&program, &processes))
        },
    )
}

pub fn compile(style: &Value) -> Result<Option<Plan>, String> {
    let Some(program) = from_style(style)? else {
        return Ok(None);
    };
    let processes = effective_processes(&program);
    let controls = controls(&program, &processes);
    let drainage_pattern = program
        .hydrology
        .drainage_pattern
        .clone()
        .unwrap_or_else(|| default_drainage_pattern(&program));
    let mut sequence = vec![
        "regional-tectonics".into(),
        "climate-and-wind".into(),
        "hydrology-and-historic-channel-shift".into(),
        "landform-overprint".into(),
        "geomorphic-processes".into(),
        "biome-and-hazard-field".into(),
        "natural-only-heightfield".into(),
    ];
    sequence.splice(
        4..5,
        processes.iter().map(|process| process.kind.clone()),
    );
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
        drainage_pattern,
        river_density: controls.river_density,
        channel_sinuosity: controls.channel_sinuosity,
        floodplain_width_m: controls.floodplain_width_m,
        rain_shadow: controls.rain_shadow,
        hazards: program.hazards.clone(),
        geomorphic_processes: processes.clone(),
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
            river_density: controls.river_density,
            channel_sinuosity: controls.channel_sinuosity,
            floodplain_width_m: controls.floodplain_width_m,
            rain_shadow: controls.rain_shadow,
            tectonic_uplift: controls.tectonic_uplift,
            tectonic_subsidence: controls.tectonic_subsidence,
            fluvial_incision: controls.fluvial_incision,
            fluvial_deposition: controls.fluvial_deposition,
            coastal_erosion: controls.coastal_erosion,
            aeolian_erosion: controls.aeolian_erosion,
            aeolian_deposition: controls.aeolian_deposition,
            karst_strength: controls.karst_strength,
            glacial_strength: controls.glacial_strength,
            mass_wasting: controls.mass_wasting,
        },
        sequence,
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
        || program
            .coast
            .shelf_width_m
            .is_some_and(|value| !(0.0..=20_000.0).contains(&value))
        || program
            .coast
            .tidal_range_m
            .is_some_and(|value| !(0.0..=20.0).contains(&value))
        || program
            .coast
            .erosion_bias
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
        || program
            .climate
            .rain_shadow
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
        || program
            .hydrology
            .river_density
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
        || program
            .hydrology
            .channel_sinuosity
            .is_some_and(|value| !(1.0..=4.0).contains(&value))
        || program
            .hydrology
            .floodplain_width_m
            .is_some_and(|value| !(0.0..=5000.0).contains(&value))
        || program
            .landform
            .karst_strength
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
        || program
            .landform
            .aeolian_strength
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
        || program
            .landform
            .glacial_strength
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
        || program
            .landform
            .mass_wasting
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
    {
        return Err("invalid naturalGeography values".into());
    }
    if let Some(pattern) = &program.hydrology.drainage_pattern {
        const PATTERNS: [&str; 7] = [
            "dendritic",
            "trellis",
            "radial",
            "parallel",
            "deranged",
            "anastomosing",
            "endorheic",
        ];
        if !PATTERNS.contains(&pattern.as_str()) {
            return Err("unsupported naturalGeography drainagePattern".into());
        }
    }
    const PROCESSES: [&str; 13] = [
        "tectonic-uplift",
        "tectonic-subsidence",
        "fluvial-incision",
        "fluvial-deposition",
        "river-avulsion",
        "coastal-wave-erosion",
        "coastal-deposition",
        "aeolian-erosion",
        "aeolian-deposition",
        "glacial-scour",
        "karst-dissolution",
        "mass-wasting",
        "volcanic-arc",
    ];
    let mut seen = std::collections::BTreeSet::new();
    for process in &program.processes {
        if !PROCESSES.contains(&process.kind.as_str())
            || !(0.0..=1.0).contains(&process.strength)
            || process.axis_deg.is_some_and(|axis| !axis.is_finite())
            || !seen.insert(process.kind.as_str())
        {
            return Err("invalid naturalGeography geomorphic process".into());
        }
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

fn default_drainage_pattern(program: &Program) -> String {
    match program.setting.as_str() {
        "inland-basin" | "inland-steppe" | "plateau-basin" => "endorheic",
        "volcanic-island" => "radial",
        "coastal-mountain" => "dendritic",
        "river-delta" | "ria-bay" | "coastal-bay" => "dendritic",
        "fold-mountain" | "rift-valley" => "trellis",
        _ => "dendritic",
    }
    .into()
}

fn effective_processes(program: &Program) -> Vec<Process> {
    let mut processes = program.processes.clone();
    let mut add = |kind: &str, strength: f32, axis_deg: Option<f32>, note: &str| {
        if strength > 0.01
            && !processes
                .iter()
                .any(|process| process.kind == kind)
        {
            processes.push(Process {
                kind: kind.into(),
                strength: strength.clamp(0.0, 1.0),
                axis_deg,
                note: Some(note.into()),
            });
        }
    };
    add(
        "tectonic-uplift",
        program.uplift_bias,
        Some(program.orogeny_axis_deg),
        "authored regional relief and fold/uplift bias",
    );
    if matches!(
        program.setting.as_str(),
        "inland-basin" | "river-delta" | "coastal-bay" | "ria-bay"
    ) {
        add(
            "tectonic-subsidence",
            0.42,
            Some(program.orogeny_axis_deg + 90.0),
            "lowland or basin setting accumulates sediment and subsides",
        );
    }
    add(
        "fluvial-incision",
        program.landform.incised_valleys,
        None,
        "valley incision follows drainage and relief",
    );
    if matches!(
        program.setting.as_str(),
        "river-delta" | "coastal-bay" | "ria-bay"
    ) || program.hydrology.wetland_bias > 0.35
    {
        add(
            "fluvial-deposition",
            (program.hydrology.wetland_bias * 0.65 + 0.25).min(1.0),
            None,
            "low-gradient drainage deposits alluvium and builds floodplains",
        );
    }
    add(
        "river-avulsion",
        (program.hydrology.historic_channel_shift_m / 900.0).clamp(0.0, 1.0),
        None,
        "historic channel-shift distance records abandoned channels",
    );
    if program.coast.coast_type != "none" {
        let exposure = match program.coast.wave_exposure.as_str() {
            "high" | "exposed" => 0.82,
            "moderate" => 0.56,
            "low" | "sheltered" => 0.26,
            _ => 0.42,
        };
        add(
            "coastal-wave-erosion",
            program
                .coast
                .erosion_bias
                .unwrap_or(exposure),
            None,
            "coastal exposure controls wave erosion and cliff/beach formation",
        );
        if matches!(
            program.setting.as_str(),
            "river-delta" | "coastal-bay" | "ria-bay" | "tidal-flat"
        ) {
            add(
                "coastal-deposition",
                0.58,
                None,
                "sheltered coast and river mouths build spits, mudflats and deltas",
            );
        }
    }
    if program.landform.aeolian {
        add(
            "aeolian-erosion",
            program.landform.aeolian_strength.unwrap_or(0.54),
            Some(program.climate.prevailing_wind_deg),
            "prevailing wind removes fines and exposes deflation surfaces",
        );
        add(
            "aeolian-deposition",
            program.landform.aeolian_strength.unwrap_or(0.58) * 0.8,
            Some(program.climate.prevailing_wind_deg),
            "transported sand forms dunes and loess-like downwind deposits",
        );
    }
    if program.landform.glacial {
        add(
            "glacial-scour",
            program.landform.glacial_strength.unwrap_or(0.68),
            Some(program.orogeny_axis_deg + 35.0),
            "glacial troughs and scoured lows overprint older relief",
        );
    }
    if program.landform.karst {
        add(
            "karst-dissolution",
            program.landform.karst_strength.unwrap_or(0.72),
            None,
            "soluble bedrock produces sinkholes, closed depressions and caves",
        );
    }
    if program.hazards.iter().any(|hazard| hazard == "landslide")
        || program
            .landform
            .mass_wasting
            .is_some_and(|strength| strength > 0.0)
    {
        add(
            "mass-wasting",
            program.landform.mass_wasting.unwrap_or(0.48),
            None,
            "steep relief and landslide hazard produce colluvium and talus",
        );
    }
    processes
}

fn controls(program: &Program, processes: &[Process]) -> GeographyControls {
    let coastality = match program.setting.as_str() {
        "coastal-mountain" | "coastal-bay" | "island" => 1.0,
        "river-delta" | "ria-bay" => 0.8,
        "inland-basin" => 0.15,
        _ => 0.45,
    };
    let strength = |kind: &str| {
        processes
            .iter()
            .find(|process| process.kind == kind)
            .map_or(0.0, |process| process.strength)
    };
    let karst_strength = program
        .landform
        .karst_strength
        .unwrap_or(if program.landform.karst { 0.72 } else { 0.0 });
    let aeolian_strength = program
        .landform
        .aeolian_strength
        .unwrap_or(if program.landform.aeolian { 0.62 } else { 0.0 });
    let glacial_strength = program
        .landform
        .glacial_strength
        .unwrap_or(if program.landform.glacial { 0.68 } else { 0.0 });
    let river_density = program
        .hydrology
        .river_density
        .unwrap_or_else(|| match program.hydrology.regime.as_str() {
            "perennial" | "monsoon-pulse" => 0.62,
            "endorheic-ephemeral" => 0.16,
            "tidal" => 0.78,
            _ => 0.48,
        });
    let channel_sinuosity = program
        .hydrology
        .channel_sinuosity
        .unwrap_or(if program.hydrology.wetland_bias > 0.4 {
            1.55
        } else {
            1.22
        });
    let floodplain_width_m = program
        .hydrology
        .floodplain_width_m
        .unwrap_or((program.hydrology.historic_channel_shift_m * 0.8)
            .clamp(40.0, 1800.0));
    let rain_shadow = program.climate.rain_shadow.unwrap_or_else(|| {
        ((program.regional_relief_m / 2500.0) * 0.45
            + if program.climate.monsoon { 0.15 } else { 0.0 })
            .clamp(0.0, 0.72)
    });
    GeographyControls {
        authored: true,
        relief_scale: (program.regional_relief_m / 600.0).clamp(0.5, 2.0),
        uplift_bias: program.uplift_bias,
        orogeny_axis_rad: program.orogeny_axis_deg.to_radians(),
        coastality,
        rainfall_bias: program.climate.rainfall_bias,
        seasonality: program.climate.seasonality,
        wind_rad: program.climate.prevailing_wind_deg.to_radians(),
        karst: karst_strength,
        aeolian: aeolian_strength,
        glacial: glacial_strength,
        historic_channel_shift_m: program.hydrology.historic_channel_shift_m,
        wetland_bias: program.hydrology.wetland_bias,
        incised_valleys: program.landform.incised_valleys,
        river_density,
        channel_sinuosity,
        floodplain_width_m,
        rain_shadow,
        tectonic_uplift: strength("tectonic-uplift"),
        tectonic_subsidence: strength("tectonic-subsidence"),
        fluvial_incision: strength("fluvial-incision"),
        fluvial_deposition: strength("fluvial-deposition"),
        coastal_erosion: strength("coastal-wave-erosion"),
        aeolian_erosion: strength("aeolian-erosion"),
        aeolian_deposition: strength("aeolian-deposition"),
        karst_strength,
        glacial_strength,
        mass_wasting: strength("mass-wasting"),
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
        assert_eq!(plan.drainage_pattern, "dendritic");
        assert!(plan
            .geomorphic_processes
            .iter()
            .any(|process| process.kind == "tectonic-uplift"));
        assert!(plan
            .geomorphic_processes
            .iter()
            .any(|process| process.kind == "fluvial-incision"));
        assert!(plan
            .geomorphic_processes
            .iter()
            .any(|process| process.kind == "coastal-wave-erosion"));
        assert_eq!(
            compile(&style()).unwrap().unwrap(),
            plan,
            "same authored geography must compile deterministically"
        );
    }

    #[test]
    fn process_overrides_and_drainage_geometry_feed_controls() {
        let mut authored = style();
        authored["naturalGeography"]["hydrology"]["drainagePattern"] = json!("trellis");
        authored["naturalGeography"]["hydrology"]["riverDensity"] = json!(0.22);
        authored["naturalGeography"]["hydrology"]["channelSinuosity"] = json!(2.1);
        authored["naturalGeography"]["hydrology"]["floodplainWidthM"] = json!(740);
        authored["naturalGeography"]["landform"]["aeolian"] = json!(true);
        authored["naturalGeography"]["landform"]["aeolianStrength"] = json!(0.81);
        authored["naturalGeography"]["processes"] = json!([
            {"kind":"aeolian-deposition","strength":0.93,"axisDeg":315},
            {"kind":"coastal-deposition","strength":0.66}
        ]);
        let plan = compile(&authored).unwrap().unwrap();
        assert_eq!(plan.drainage_pattern, "trellis");
        assert_eq!(plan.river_density, 0.22);
        assert_eq!(plan.channel_sinuosity, 2.1);
        assert_eq!(plan.floodplain_width_m, 740.0);
        assert_eq!(plan.controls.aeolian_deposition, 0.93);
        assert!(plan
            .geomorphic_processes
            .iter()
            .any(|process| process.kind == "aeolian-erosion"));
    }

    #[test]
    fn invalid_geography_fails_closed() {
        let mut bad = style();
        bad["naturalGeography"]["climate"]["rainfallBias"] = json!(1.5);
        assert!(compile(&bad).is_err());
        let mut hazard = style();
        hazard["naturalGeography"]["hazards"] = json!(["meteor"]);
        assert!(compile(&hazard).is_err());
        let mut process = style();
        process["naturalGeography"]["processes"] = json!([
            {"kind":"unknown-process","strength":0.5}
        ]);
        assert!(compile(&process).is_err());
        let mut drainage = style();
        drainage["naturalGeography"]["hydrology"]["drainagePattern"] = json!("grid");
        assert!(compile(&drainage).is_err());
    }
}
