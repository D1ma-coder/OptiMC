//! Official JSON block models and blockstate dispatch. No embedded Minecraft code.
mod bake;
pub mod random;
use anyhow::{bail, ensure, Context, Result};
pub use bake::BakedQuad;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
pub type Properties = BTreeMap<String, String>;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ResourceId {
    pub namespace: String,
    pub path: String,
}
impl ResourceId {
    pub fn parse(value: &str) -> Result<Self> {
        let (namespace, path) = value.split_once(':').unwrap_or(("minecraft", value));
        ensure!(
            !namespace.is_empty()
                && namespace
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_.-".contains(c)),
            "Invalid resource namespace: {value}"
        );
        ensure!(
            !path.is_empty()
                && path
                    .split('/')
                    .all(|s| !s.is_empty() && s != "." && s != "..")
                && path
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "/_.-".contains(c)),
            "Invalid resource path: {value}"
        );
        Ok(Self {
            namespace: namespace.into(),
            path: path.into(),
        })
    }
    pub fn file(&self, root: &Path, kind: &str, extension: &str) -> PathBuf {
        root.join(&self.namespace)
            .join(kind)
            .join(format!("{}.{}", self.path, extension))
    }
}
impl std::fmt::Display for ResourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}
impl Direction {
    pub fn normal(self) -> glam::Vec3 {
        use glam::Vec3;
        match self {
            Self::Down => -Vec3::Y,
            Self::Up => Vec3::Y,
            Self::North => -Vec3::Z,
            Self::South => Vec3::Z,
            Self::West => -Vec3::X,
            Self::East => Vec3::X,
        }
    }
    pub fn closest(n: glam::Vec3) -> Self {
        [
            Self::Down,
            Self::Up,
            Self::North,
            Self::South,
            Self::West,
            Self::East,
        ]
        .into_iter()
        .max_by(|a, b| a.normal().dot(n).total_cmp(&b.normal().dot(n)))
        .unwrap()
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Texture {
    Reference(String),
    Material {
        sprite: String,
        #[serde(default)]
        force_translucent: bool,
    },
}
#[derive(Clone, Debug)]
pub struct Material {
    pub sprite: ResourceId,
    pub force_translucent: bool,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Face {
    pub texture: String,
    pub uv: Option<[f32; 4]>,
    pub cullface: Option<Direction>,
    #[serde(default)]
    pub rotation: u16,
    #[serde(default = "untinted")]
    pub tintindex: i32,
}
fn untinted() -> i32 {
    -1
}
#[derive(Clone, Debug, Deserialize)]
pub struct Rotation {
    pub origin: [f32; 3],
    pub axis: Option<String>,
    pub angle: Option<f32>,
    #[serde(default)]
    pub rescale: bool,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub z: f32,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Element {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub rotation: Option<Rotation>,
    pub faces: BTreeMap<Direction, Face>,
    #[serde(default = "yes")]
    pub shade: bool,
    #[serde(default)]
    pub light_emission: u8,
}
fn yes() -> bool {
    true
}
#[derive(Clone, Debug, Deserialize)]
pub struct DisplayTransform {
    #[serde(default)]
    pub rotation: [f32; 3],
    #[serde(default)]
    pub translation: [f32; 3],
    #[serde(default = "unit")]
    pub scale: [f32; 3],
}
impl Default for DisplayTransform {
    fn default() -> Self {
        Self {
            rotation: [0.; 3],
            translation: [0.; 3],
            scale: [1.; 3],
        }
    }
}
fn unit() -> [f32; 3] {
    [1.; 3]
}
#[derive(Clone, Debug, Default, Deserialize)]
struct RawModel {
    parent: Option<String>,
    #[serde(default)]
    textures: BTreeMap<String, Texture>,
    elements: Option<Vec<Element>>,
    ambientocclusion: Option<bool>,
    gui_light: Option<String>,
    #[serde(default)]
    display: BTreeMap<String, DisplayTransform>,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub textures: BTreeMap<String, Texture>,
    pub elements: Vec<Element>,
    pub ambient_occlusion: bool,
    pub gui_light: String,
    pub display: BTreeMap<String, DisplayTransform>,
}
impl Model {
    pub fn material(&self, value: &str) -> Result<Material> {
        let mut next = value;
        let mut seen = HashSet::new();
        loop {
            if let Some(key) = next.strip_prefix('#') {
                ensure!(seen.insert(key), "Texture reference cycle at {key}");
                match self
                    .textures
                    .get(key)
                    .with_context(|| format!("Unresolved texture #{key}"))?
                {
                    Texture::Reference(value) => next = value,
                    Texture::Material {
                        sprite,
                        force_translucent,
                    } => {
                        return Ok(Material {
                            sprite: ResourceId::parse(sprite)?,
                            force_translucent: *force_translucent,
                        })
                    }
                }
            } else {
                return Ok(Material {
                    sprite: ResourceId::parse(next)?,
                    force_translucent: false,
                });
            }
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct Variant {
    pub model: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub uvlock: bool,
    #[serde(default = "one")]
    pub weight: u32,
}
fn one() -> u32 {
    1
}
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Variants {
    One(Variant),
    Many(Vec<Variant>),
}
impl Variants {
    pub fn all(&self) -> &[Variant] {
        match self {
            Self::One(v) => std::slice::from_ref(v),
            Self::Many(v) => v,
        }
    }
    /// Caller supplies a random integer in the total weight range; selection is
    /// kept separate from the game's seeded random source.
    pub fn choose(&self, random: u64) -> Result<&Variant> {
        let variants = self.all();
        let total = variants.iter().try_fold(0u64, |n, v| {
            ensure!(v.weight > 0, "Zero model weight");
            n.checked_add(v.weight as u64)
                .context("Variant weight overflow")
        })?;
        ensure!(total > 0, "Empty model variants");
        let mut ticket = random % total;
        for v in variants {
            if ticket < v.weight as u64 {
                return Ok(v);
            }
            ticket -= v.weight as u64;
        }
        unreachable!()
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct Multipart {
    pub when: Option<serde_json::Value>,
    pub apply: Variants,
}
#[derive(Clone, Debug, Default, Deserialize)]
pub struct BlockState {
    #[serde(default)]
    pub variants: BTreeMap<String, Variants>,
    #[serde(default)]
    pub multipart: Vec<Multipart>,
}
impl BlockState {
    pub fn choices(&self, properties: &Properties) -> Result<Vec<&Variants>> {
        let mut result = Vec::new();
        for (key, variants) in &self.variants {
            if variant_matches(key, properties)? {
                result.push(variants);
            }
        }
        ensure!(
            result.len() <= 1,
            "Ambiguous blockstate variants for {properties:?}"
        );
        for part in &self.multipart {
            if part
                .when
                .as_ref()
                .map(|v| condition(v, properties))
                .transpose()?
                .unwrap_or(true)
            {
                result.push(&part.apply);
            }
        }
        Ok(result)
    }
    pub fn selected(&self, properties: &Properties, random: u64) -> Result<Vec<&Variant>> {
        let mut result = Vec::new();
        for (key, variants) in &self.variants {
            if variant_matches(key, properties)? {
                result.push(variants.choose(random)?);
            }
        }
        ensure!(
            result.len() <= 1,
            "Ambiguous blockstate variants for {properties:?}"
        );
        for part in &self.multipart {
            if part
                .when
                .as_ref()
                .map(|v| condition(v, properties))
                .transpose()?
                .unwrap_or(true)
            {
                result.push(part.apply.choose(random)?);
            }
        }
        Ok(result)
    }
    pub fn all_variants(&self) -> impl Iterator<Item = &Variant> {
        self.variants
            .values()
            .chain(self.multipart.iter().map(|p| &p.apply))
            .flat_map(Variants::all)
    }
}
fn variant_matches(key: &str, properties: &Properties) -> Result<bool> {
    if key.is_empty() {
        return Ok(true);
    }
    for pair in key.split(',') {
        let (key, value) = pair.split_once('=').context("Invalid variant selector")?;
        if properties.get(key).map(String::as_str) != Some(value) {
            return Ok(false);
        }
    }
    Ok(true)
}
fn condition(value: &serde_json::Value, properties: &Properties) -> Result<bool> {
    let map = value
        .as_object()
        .context("Multipart condition must be an object")?;
    for (key, value) in map {
        if key == "OR" || key == "AND" {
            let children = value.as_array().context("AND/OR must contain conditions")?;
            ensure!(!children.is_empty(), "Empty AND/OR");
            let matches = children
                .iter()
                .map(|child| condition(child, properties))
                .collect::<Result<Vec<_>>>()?;
            if if key == "OR" {
                !matches.into_iter().any(|v| v)
            } else {
                !matches.into_iter().all(|v| v)
            } {
                return Ok(false);
            }
        } else {
            let expression = value.as_str().context("Property selector must be string")?;
            let (negated, expression) = expression
                .strip_prefix('!')
                .map_or((false, expression), |e| (true, e));
            let actual = properties
                .get(key)
                .with_context(|| format!("Missing property {key}"))?;
            ensure!(!expression.is_empty(), "Empty property selector");
            let matches = expression.split('|').any(|expected| expected == actual);
            if matches == negated {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub struct Catalog {
    root: PathBuf,
    models: HashMap<ResourceId, Arc<Model>>,
    states: HashMap<ResourceId, Arc<BlockState>>,
    baked: HashMap<(ResourceId, i32, i32, bool), Arc<[BakedQuad]>>,
}
impl Catalog {
    pub fn new(cache: &Path) -> Self {
        Self {
            root: super::root(cache).join("resources/assets"),
            models: HashMap::new(),
            states: HashMap::new(),
            baked: HashMap::new(),
        }
    }
    pub fn model(&mut self, id: &ResourceId) -> Result<Arc<Model>> {
        self.load_model(id, &mut HashSet::new())
    }
    fn load_model(
        &mut self,
        id: &ResourceId,
        visiting: &mut HashSet<ResourceId>,
    ) -> Result<Arc<Model>> {
        if let Some(model) = self.models.get(id) {
            return Ok(model.clone());
        }
        ensure!(
            visiting.len() < 128 && visiting.insert(id.clone()),
            "Model parent cycle/depth at {id}"
        );
        if id.namespace == "minecraft"
            && matches!(id.path.as_str(), "builtin/generated" | "builtin/entity")
        {
            let model = Arc::new(Model::default());
            self.models.insert(id.clone(), model.clone());
            visiting.remove(id);
            return Ok(model);
        }
        let path = id.file(&self.root, "models", "json");
        let raw: RawModel = serde_json::from_slice(
            &fs::read(&path).with_context(|| format!("Loading {}", path.display()))?,
        )
        .with_context(|| format!("Parsing {id}"))?;
        let mut model = if let Some(parent) = raw.parent {
            (*self.load_model(&ResourceId::parse(&parent)?, visiting)?).clone()
        } else {
            Model {
                ambient_occlusion: true,
                gui_light: "side".into(),
                ..Model::default()
            }
        };
        model.textures.extend(raw.textures);
        model.display.extend(raw.display);
        if let Some(elements) = raw.elements {
            model.elements = elements;
        }
        if let Some(value) = raw.ambientocclusion {
            model.ambient_occlusion = value;
        }
        if let Some(value) = raw.gui_light {
            model.gui_light = value;
        }
        validate(&model)?;
        visiting.remove(id);
        let model = Arc::new(model);
        self.models.insert(id.clone(), model.clone());
        Ok(model)
    }
    pub fn blockstate(&mut self, id: &ResourceId) -> Result<Arc<BlockState>> {
        if let Some(state) = self.states.get(id) {
            return Ok(state.clone());
        }
        let path = id.file(&self.root, "blockstates", "json");
        let state: BlockState = serde_json::from_slice(&fs::read(&path)?)
            .with_context(|| format!("Parsing blockstate {id}"))?;
        ensure!(
            state.variants.is_empty() || state.multipart.is_empty(),
            "Mixed blockstate dispatch {id}"
        );
        let state = Arc::new(state);
        self.states.insert(id.clone(), state.clone());
        Ok(state)
    }
    pub fn bake(&mut self, variant: &Variant) -> Result<Arc<[BakedQuad]>> {
        ensure!(
            variant.x.rem_euclid(90) == 0 && variant.y.rem_euclid(90) == 0,
            "Invalid variant rotation"
        );
        let key = (
            ResourceId::parse(&variant.model)?,
            variant.x.rem_euclid(360),
            variant.y.rem_euclid(360),
            variant.uvlock,
        );
        if let Some(quads) = self.baked.get(&key) {
            return Ok(quads.clone());
        }
        let model = self.model(&key.0)?;
        let variant = Variant {
            x: key.1,
            y: key.2,
            ..variant.clone()
        };
        let quads: Arc<[BakedQuad]> = bake::bake(&model, &variant)?.into();
        self.baked.insert(key, quads.clone());
        Ok(quads)
    }
    pub fn texture_file(&self, id: &ResourceId) -> PathBuf {
        id.file(&self.root, "textures", "png")
    }
    pub fn block_ids(&self) -> Result<Vec<ResourceId>> {
        let mut ids = Vec::new();
        for file in fs::read_dir(self.root.join("minecraft/blockstates"))? {
            let file = file?;
            if file.path().extension().is_some_and(|e| e == "json") {
                ids.push(ResourceId::parse(&format!(
                    "minecraft:{}",
                    file.path().file_stem().unwrap().to_string_lossy()
                ))?);
            }
        }
        ids.sort();
        Ok(ids)
    }
}
fn validate(model: &Model) -> Result<()> {
    ensure!(model.elements.len() <= 4096, "Too many model elements");
    for element in &model.elements {
        ensure!(
            element
                .from
                .iter()
                .chain(&element.to)
                .all(|v| v.is_finite() && (-16.0..=32.0).contains(v)),
            "Invalid element bounds"
        );
        for face in element.faces.values() {
            ensure!(
                face.rotation <= 270 && face.rotation.is_multiple_of(90),
                "Invalid face rotation"
            );
            if let Some(uv) = face.uv {
                ensure!(uv.into_iter().all(f32::is_finite), "Non-finite UV");
            }
        }
        if let Some(r) = &element.rotation {
            ensure!(
                r.origin
                    .into_iter()
                    .chain([r.x, r.y, r.z])
                    .chain(r.angle)
                    .all(f32::is_finite),
                "Non-finite rotation"
            );
            if let Some(axis) = &r.axis {
                ensure!(
                    ["x", "y", "z"].contains(&axis.as_str()) && r.angle.is_some(),
                    "Invalid rotation axis/angle"
                );
            } else if r.angle.is_some() {
                bail!("Angle needs axis");
            }
        }
    }
    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct Audit {
    pub blockstates: usize,
    pub models: usize,
    pub variants: usize,
    pub quads: usize,
    pub textures: usize,
    pub translucent_quads: usize,
    pub element_rotations: usize,
}
impl Catalog {
    pub fn audit(&mut self) -> Result<Audit> {
        let mut report = Audit {
            blockstates: 0,
            models: 0,
            variants: 0,
            quads: 0,
            textures: 0,
            translucent_quads: 0,
            element_rotations: 0,
        };
        for file in fs::read_dir(self.root.join("minecraft/models/block"))? {
            let file = file?;
            if file.path().extension().is_some_and(|e| e == "json") {
                let id = ResourceId::parse(&format!(
                    "block/{}",
                    file.path().file_stem().unwrap().to_string_lossy()
                ))?;
                self.model(&id).with_context(|| format!("Resolving {id}"))?;
            }
        }
        let mut textures = HashSet::new();
        for id in self.block_ids()? {
            let state = self.blockstate(&id)?;
            report.blockstates += 1;
            for variant in state.all_variants() {
                let quads = self
                    .bake(variant)
                    .with_context(|| format!("Baking {id}: {}", variant.model))?;
                report.variants += 1;
                report.quads += quads.len();
                for quad in quads.iter() {
                    report.translucent_quads += quad.material.force_translucent as usize;
                    if quad.material.sprite.path != "missingno" {
                        ensure!(
                            self.texture_file(&quad.material.sprite).exists(),
                            "Missing texture {} in {}",
                            quad.material.sprite,
                            id
                        );
                    }
                    textures.insert(quad.material.sprite.clone());
                }
            }
        }
        report.models = self.models.len();
        report.textures = textures.len();
        report.element_rotations = self
            .models
            .values()
            .map(|m| m.elements.iter().filter(|e| e.rotation.is_some()).count())
            .sum();
        Ok(report)
    }
}

#[cfg(test)]
mod official_tests {
    use super::*;
    #[test]
    #[ignore = "Requires imported official 26.2 resources"]
    fn entire_official_block_catalog_resolves_and_bakes() {
        let mut catalog = Catalog::new(Path::new(".cache"));
        let report = catalog.audit().unwrap();
        assert_eq!(report.blockstates, 1198);
        assert_eq!(report.models, 2657);
        assert_eq!(report.variants, 8661);
        assert_eq!(report.quads, 86486);
        for state in &crate::assets::builtins::get().unwrap().block_states {
            let dispatcher = catalog
                .blockstate(&ResourceId::parse(&state.block).unwrap())
                .unwrap();
            assert!(
                !dispatcher.variants.is_empty() || !dispatcher.multipart.is_empty(),
                "Missing dispatcher for {}",
                state.block
            );
            let selected = dispatcher.selected(&state.properties, 0).unwrap();
            // A multipart wall with no arms and no post intentionally selects
            // no visible parts. A variant dispatcher must select one variant.
            if !dispatcher.variants.is_empty() {
                assert_eq!(selected.len(), 1, "{} {:?}", state.block, state.properties);
            }
        }
        let state = catalog
            .blockstate(&ResourceId::parse("oak_fence").unwrap())
            .unwrap();
        let properties = Properties::from([
            ("north".into(), "true".into()),
            ("east".into(), "false".into()),
            ("south".into(), "false".into()),
            ("west".into(), "true".into()),
        ]);
        assert_eq!(state.selected(&properties, 0).unwrap().len(), 3);
        let grass = catalog
            .model(&ResourceId::parse("block/grass_block").unwrap())
            .unwrap();
        assert_eq!(grass.elements.len(), 2);
        let heavy = catalog
            .bake(&Variant {
                model: "block/heavy_core".into(),
                x: 0,
                y: 0,
                uvlock: false,
                weight: 1,
            })
            .unwrap();
        assert_eq!(heavy.len(), 6);
        let cached = catalog
            .bake(&Variant {
                model: "block/heavy_core".into(),
                x: 360,
                y: 0,
                uvlock: false,
                weight: 9,
            })
            .unwrap();
        assert!(Arc::ptr_eq(&heavy, &cached));
        assert!(heavy
            .iter()
            .all(|q| q.material.sprite.path == "block/heavy_core"));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_ids_cannot_escape_asset_tree() {
        for id in [
            "../foo",
            "foo:../bar",
            "foo:/bar",
            "foo:bar//baz",
            "a/b:x",
            "Minecraft:block/stone",
        ] {
            assert!(ResourceId::parse(id).is_err());
        }
        assert_eq!(
            ResourceId::parse("block/stone").unwrap().to_string(),
            "minecraft:block/stone"
        );
    }
    #[test]
    fn texture_indirection_and_cycles_are_resolved() {
        let mut model = Model::default();
        model
            .textures
            .insert("a".into(), Texture::Reference("#b".into()));
        model.textures.insert(
            "b".into(),
            Texture::Material {
                sprite: "block/stone".into(),
                force_translucent: true,
            },
        );
        assert!(model.material("#a").unwrap().force_translucent);
        model
            .textures
            .insert("b".into(), Texture::Reference("#a".into()));
        assert!(model.material("#a").is_err());
    }
    #[test]
    fn multipart_combines_and_or_and_negated_values() {
        let properties = Properties::from([
            ("north".into(), "true".into()),
            ("east".into(), "false".into()),
            ("age".into(), "2".into()),
        ]);
        assert!(condition(
            &serde_json::json!({"AND":[{"OR":[{"north":"true"},{"east":"true"}]},{"age":"!0|1"}]}),
            &properties
        )
        .unwrap());
        assert!(!condition(&serde_json::json!({"north":"false"}), &properties).unwrap());
    }
    #[test]
    fn multipart_can_intentionally_select_no_visible_geometry() {
        let state:BlockState=serde_json::from_value(serde_json::json!({"multipart":[{"when":{"up":"true"},"apply":{"model":"block/wall_post"}}]})).unwrap();
        assert!(state
            .selected(&Properties::from([("up".into(), "false".into())]), 0)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn weighted_variants_honor_each_weight() {
        let variants: Variants = serde_json::from_value(
            serde_json::json!([{"model":"block/a","weight":1},{"model":"block/b","weight":3}]),
        )
        .unwrap();
        assert_eq!(
            (0..4)
                .map(|i| variants.choose(i).unwrap().model.as_str())
                .collect::<Vec<_>>(),
            ["block/a", "block/b", "block/b", "block/b"]
        );
    }
}
