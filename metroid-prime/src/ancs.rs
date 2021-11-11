use std::io::Read;

use anyhow::{bail, Result};
use gamecube::bytes::{
    ReadAsciiCStringExt, ReadFixedCapacityAsciiCStringExt, ReadFrom, ReadFromWithContext,
    ReadTypedWithContextExt,
};
use gamecube::{ReadBytesExt, ReadTypedExt};
use pretty_hex::PrettyHex;

#[derive(Clone, Debug)]
pub struct Ancs {
    character_set: CharacterSet,
    animation_set: AnimationSet,
}

impl ReadFrom for Ancs {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let version = r.read_u16()?;
        assert_eq!(version, 1);
        let character_set = r.read_typed()?;
        let animation_set = r.read_typed()?;

        Ok(Self {
            character_set,
            animation_set,
        })
    }
}

#[derive(Clone, Debug)]
struct CharacterSet {
    version: u16,
    characters: Vec<Character>,
}

impl ReadFrom for CharacterSet {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let version = r.read_u16()?;
        assert_eq!(version, 1);
        let count = r.read_u32()?;
        let mut characters = Vec::new();
        for _ in 0..count {
            characters.push(r.read_typed()?);
        }
        Ok(Self {
            version,
            characters,
        })
    }
}

#[derive(Clone, Debug)]
struct Character {
    id: u32,
    version: u16,
    name: String,
    model_id: u32,
    skin_id: u32,
    skeleton_id: u32,
    animations: Vec<AnimationName>,
    pas_database: PasDatabase,
    particle_resource_data: ParticleResourceData,
    animation_aabbs: Vec<AnimationAabb>,
    effects: Vec<Effect>,
    frozen_model_id: u32,
    frozen_skin_id: u32,
    animation_ids: Vec<u32>,
}

impl ReadFrom for Character {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let id = r.read_u32()?;
        let version = r.read_u16()?;
        if version > 6 {
            bail!("unexpected ANCS character version: {}", version);
        }
        let name = r.read_ascii_c_string()?;
        let model_id = r.read_u32()?;
        let skin_id = r.read_u32()?;
        let skeleton_id = r.read_u32()?;

        let count = r.read_u32()?;
        let mut animation_names = Vec::new();
        for _ in 0..count {
            animation_names.push(r.read_typed_with_context(AnimationContext { version })?);
        }

        let pas_database = r.read_typed()?;
        let particle_resource_data =
            r.read_typed_with_context(ParticleResourceDataContext { version })?;

        let _ = r.read_u32()?;
        if version >= 10 {
            let _ = r.read_u32()?;
        }

        let mut animation_aabbs = Vec::new();
        let mut effects = Vec::new();
        if version >= 2 {
            let count = r.read_u32()?;
            for _ in 0..count {
                animation_aabbs.push(r.read_typed()?);
            }

            let count = r.read_u32()?;
            for _ in 0..count {
                effects.push(r.read_typed()?);
            }
        }

        let mut frozen_model_id = 0;
        let mut frozen_skin_id = 0;
        if version >= 4 {
            frozen_model_id = r.read_u32()?;
            frozen_skin_id = r.read_u32()?;
        }

        let mut animation_ids = Vec::new();
        if version >= 5 {
            let count = r.read_u32()?;
            for _ in 0..count {
                animation_ids.push(r.read_u32()?);
            }
        }

        if version >= 10 {
            panic!();
        }

        Ok(Self {
            id,
            version,
            name,
            model_id,
            skin_id,
            skeleton_id,
            animations: animation_names,
            pas_database,
            particle_resource_data,
            animation_aabbs,
            effects,
            frozen_model_id,
            frozen_skin_id,
            animation_ids,
        })
    }
}

#[derive(Clone, Debug)]
struct AnimationName {
    id: u32,
    name: String,
}

struct AnimationContext {
    version: u16,
}

impl ReadFromWithContext for AnimationName {
    type Context = AnimationContext;

    fn read_from_with_context<R: Read>(r: &mut R, ctx: AnimationContext) -> Result<Self> {
        let index = r.read_u32()?;
        if ctx.version < 10 {
            let _ = r.read_ascii_c_string()?;
        }
        let name = r.read_ascii_c_string()?;
        Ok(AnimationName { id: index, name })
    }
}

#[derive(Clone, Debug)]
struct PasDatabase {
    default_anim_state: u32,
    anim_states: Vec<AnimState>,
}

impl ReadFrom for PasDatabase {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let fourcc = r.read_fixed_capacity_ascii_c_string(4)?;
        assert_eq!(fourcc, "PAS4");
        let count = r.read_u32()?;
        let default_anim_state = r.read_u32()?;
        let mut anim_states = Vec::new();
        for _ in 0..count {
            anim_states.push(r.read_typed()?);
        }
        Ok(Self {
            default_anim_state,
            anim_states,
        })
    }
}

#[derive(Clone, Debug)]
struct AnimState {
    kind: u32,
    parm_infos: Vec<ParmInfo>,
    anim_infos: Vec<AnimInfo>,
}

impl ReadFrom for AnimState {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let kind = r.read_u32()?;
        let parm_info_count = r.read_u32()?;
        let anim_info_count = r.read_u32()?;

        let mut parm_infos = Vec::new();
        for _ in 0..parm_info_count {
            parm_infos.push(r.read_typed()?);
        }

        let mut anim_infos = Vec::new();
        for _ in 0..anim_info_count {
            anim_infos.push(r.read_typed_with_context(AnimInfoContext {
                parm_infos: parm_infos.clone(),
            })?);
        }

        Ok(Self {
            kind,
            parm_infos,
            anim_infos,
        })
    }
}

#[derive(Clone, Debug)]
struct ParmInfo {
    kind: ParmKind,
    function: u32,
    weight: f32,
    min: ParmValue,
    max: ParmValue,
}

impl ReadFrom for ParmInfo {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let kind = r.read_typed()?;
        let function = r.read_u32()?;
        let weight = f32::from_bits(r.read_u32()?);
        let min = r.read_typed_with_context(kind)?;
        let max = r.read_typed_with_context(kind)?;
        Ok(Self {
            kind,
            function,
            weight,
            min,
            max,
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum ParmKind {
    I32,
    U32,
    F32,
    Bool,
    Enum,
}

impl ReadFrom for ParmKind {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        Ok(match r.read_u32()? {
            0 => ParmKind::I32,
            1 => ParmKind::U32,
            2 => ParmKind::F32,
            3 => ParmKind::Bool,
            4 => ParmKind::Enum,
            x => bail!("unexpected parm type: {}", x),
        })
    }
}

#[derive(Clone, Debug)]
enum ParmValue {
    I32(i32),
    U32(u32),
    F32(f32),
    Bool(bool),
    Enum(u32),
}

impl ReadFromWithContext for ParmValue {
    type Context = ParmKind;

    fn read_from_with_context<R: Read>(r: &mut R, ctx: ParmKind) -> Result<Self> {
        Ok(match ctx {
            ParmKind::I32 => ParmValue::I32(r.read_i32()?),
            ParmKind::U32 => ParmValue::U32(r.read_u32()?),
            ParmKind::F32 => ParmValue::F32(f32::from_bits(r.read_u32()?)),
            ParmKind::Bool => ParmValue::Bool(r.read_u8()? != 0),
            ParmKind::Enum => ParmValue::Enum(r.read_u32()?),
        })
    }
}

#[derive(Clone, Debug)]
struct AnimInfo {
    id: u32,
    parm_values: Vec<ParmValue>,
}

struct AnimInfoContext {
    parm_infos: Vec<ParmInfo>,
}

impl ReadFromWithContext for AnimInfo {
    type Context = AnimInfoContext;

    fn read_from_with_context<R: Read>(r: &mut R, ctx: AnimInfoContext) -> Result<Self> {
        let id = r.read_u32()?;
        let mut parm_values = Vec::new();
        for parm_info in &ctx.parm_infos {
            parm_values.push(r.read_typed_with_context(parm_info.kind)?);
        }
        Ok(Self { id, parm_values })
    }
}

#[derive(Clone, Debug)]
struct ParticleResourceData {
    generic_particle_ids: Vec<u32>,
    swoosh_particle_ids: Vec<u32>,
    electric_particle_ids: Vec<u32>,
}

struct ParticleResourceDataContext {
    version: u16,
}

impl ReadFromWithContext for ParticleResourceData {
    type Context = ParticleResourceDataContext;

    fn read_from_with_context<R: Read>(
        r: &mut R,
        ctx: ParticleResourceDataContext,
    ) -> Result<Self> {
        let count = r.read_u32()?;
        let mut generic_particle_ids = Vec::new();
        for _ in 0..count {
            generic_particle_ids.push(r.read_typed()?);
        }

        let count = r.read_u32()?;
        let mut swoosh_particle_ids = Vec::new();
        for _ in 0..count {
            swoosh_particle_ids.push(r.read_typed()?);
        }

        if ctx.version >= 6 {
            let _ = r.read_u32()?;
        }

        let count = r.read_u32()?;
        let mut electric_particle_ids = Vec::new();
        for _ in 0..count {
            electric_particle_ids.push(r.read_typed()?);
        }

        if ctx.version >= 10 {
            panic!();
        }

        Ok(Self {
            generic_particle_ids,
            swoosh_particle_ids,
            electric_particle_ids,
        })
    }
}

#[derive(Clone, Debug)]
struct AnimationAabb {
    name: String,
    min_x: f32,
    min_y: f32,
    min_z: f32,
    max_x: f32,
    max_y: f32,
    max_z: f32,
}

impl ReadFrom for AnimationAabb {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let name = r.read_ascii_c_string()?;
        let min_x = f32::from_bits(r.read_u32()?);
        let min_y = f32::from_bits(r.read_u32()?);
        let min_z = f32::from_bits(r.read_u32()?);
        let max_x = f32::from_bits(r.read_u32()?);
        let max_y = f32::from_bits(r.read_u32()?);
        let max_z = f32::from_bits(r.read_u32()?);
        Ok(Self {
            name,
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        })
    }
}

#[derive(Clone, Debug)]
struct Effect {
    name: String,
    components: Vec<EffectComponent>,
}

impl ReadFrom for Effect {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let name = r.read_ascii_c_string()?;
        let count = r.read_u32()?;
        let mut components = Vec::new();
        for _ in 0..count {
            components.push(r.read_typed()?);
        }
        Ok(Self { name, components })
    }
}

#[derive(Clone, Debug)]
struct EffectComponent {
    name: String,
    particle_asset_type: String,
    particle_asset_id: u32,
    bone_name: String,
    scale: f32,
    parented: u32,
    flags: u32,
}

impl ReadFrom for EffectComponent {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let name = r.read_ascii_c_string()?;
        let particle_asset_type = r.read_fixed_capacity_ascii_c_string(4)?;
        let particle_asset_id = r.read_u32()?;
        let bone_name = r.read_ascii_c_string()?;
        let scale = f32::from_bits(r.read_u32()?);
        let parented = r.read_u32()?;
        let flags = r.read_u32()?;
        Ok(Self {
            name,
            particle_asset_type,
            particle_asset_id,
            bone_name,
            scale,
            parented,
            flags,
        })
    }
}

#[derive(Clone, Debug)]
struct AnimationSet {
    version: u16,
    animations: Vec<Animation>,
    transitions: Vec<Transition>,
    default_transition: MetaTransition,
    additive_animations: Vec<AdditiveAnimation>,
    default_additive_fade_in_time: Option<f32>,
    default_additive_fade_out_time: Option<f32>,
    half_transitions: Vec<HalfTransition>,
    animation_resources: Vec<AnimationResource>,
}

impl ReadFrom for AnimationSet {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let version = r.read_u16()?;
        if version < 2 || version > 4 {
            bail!("unexpected animation set version: {}", version);
        }

        let count = r.read_u32()?;
        let mut animations = Vec::new();
        for _ in 0..count {
            animations.push(r.read_typed()?);
        }

        let count = r.read_u32()?;
        let mut transitions = Vec::new();
        for _ in 0..count {
            transitions.push(r.read_typed()?);
        }
        let default_transition = r.read_typed()?;

        if version == 2 {
            let count1 = r.read_u32()?;
            if count1 != 0 {
                let mut buf = [0; 256];
                let n = r.read(&mut buf)?;
                println!("{:?}", (&buf[..n]).hex_dump());
                bail!("AnimationSet version 2 count1 is nonzero: {}", count1);
            }

            let count2 = r.read_u32()?;
            if count2 != 0 {
                let mut buf = [0; 256];
                let n = r.read(&mut buf)?;
                println!("{:?}", (&buf[..n]).hex_dump());
                bail!("AnimationSet version 2 count2 is nonzero: {}", count2);
            }

            let count3 = r.read_u32()?;
            if count3 != 0 {
                let mut buf = [0; 256];
                let n = r.read(&mut buf)?;
                println!("{:?}", (&buf[..n]).hex_dump());
                bail!("AnimationSet version 2 count3 is nonzero: {}", count3);
            }

            return Ok(Self {
                version,
                animations,
                transitions,
                default_transition,
                additive_animations: vec![],
                default_additive_fade_in_time: None,
                default_additive_fade_out_time: None,
                half_transitions: vec![],
                animation_resources: vec![],
            });
        }

        let count = r.read_u32()?;
        let mut additive_animations = Vec::new();
        for _ in 0..count {
            additive_animations.push(r.read_typed()?);
        }
        let mut default_additive_fade_in_time = None;
        let mut default_additive_fade_out_time = None;
        if version >= 4 {
            default_additive_fade_in_time = Some(f32::from_bits(r.read_u32()?));
            default_additive_fade_out_time = Some(f32::from_bits(r.read_u32()?));
        }

        let count = r.read_u32()?;
        let mut half_transitions = Vec::new();
        for _ in 0..count {
            half_transitions.push(r.read_typed()?);
        }

        let count = r.read_u32()?;
        let mut animation_resources = Vec::new();
        for _ in 0..count {
            animation_resources.push(r.read_typed()?);
        }

        Ok(Self {
            version,
            animations,
            transitions,
            default_transition,
            additive_animations,
            default_additive_fade_in_time,
            default_additive_fade_out_time,
            half_transitions,
            animation_resources,
        })
    }
}

#[derive(Clone, Debug)]
struct Animation {
    name: String,
    meta_animation: MetaAnimation,
}

impl ReadFrom for Animation {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let name = r.read_ascii_c_string()?;
        let meta_animation = r.read_typed()?;
        Ok(Self {
            name,
            meta_animation,
        })
    }
}

#[derive(Clone, Debug)]
enum MetaAnimation {
    Play {
        animation_id: u32,
        primitive_id: u32,
        primitive_name: String,
        char_anim_time: CharAnimTime,
    },
    Random(Vec<(MetaAnimation, u32)>),
    Sequence(Vec<MetaAnimation>),
}

impl ReadFrom for MetaAnimation {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let kind = r.read_u32()?;
        Ok(match kind {
            0 => {
                let animation_id = r.read_u32()?;
                let primitive_id = r.read_u32()?;
                let primitive_name = r.read_ascii_c_string()?;
                let char_anim_time = r.read_typed()?;
                MetaAnimation::Play {
                    animation_id,
                    primitive_id,
                    primitive_name,
                    char_anim_time,
                }
            }
            3 => {
                let count = r.read_u32()?;
                let mut pairs = Vec::new();
                for _ in 0..count {
                    let meta_animation = r.read_typed()?;
                    let probability = r.read_u32()?;
                    pairs.push((meta_animation, probability));
                }
                MetaAnimation::Random(pairs)
            }
            4 => {
                let count = r.read_u32()?;
                let mut animations = Vec::new();
                for _ in 0..count {
                    animations.push(r.read_typed()?);
                }
                MetaAnimation::Sequence(animations)
            }
            _ => bail!("unexpected meta animation type: {}", kind),
        })
    }
}

#[derive(Clone, Debug)]
struct CharAnimTime {
    time: f32,
    differential_state: u32,
}

impl ReadFrom for CharAnimTime {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let time = f32::from_bits(r.read_u32()?);
        let differential_state = r.read_u32()?;
        Ok(Self {
            time,
            differential_state,
        })
    }
}

#[derive(Clone, Debug)]
struct Transition {
    unknown: u32,
    animation_id_a: u32,
    animation_id_b: u32,
    meta_transition: MetaTransition,
}

impl ReadFrom for Transition {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let unknown = r.read_u32()?;
        let animation_id_a = r.read_u32()?;
        let animation_id_b = r.read_u32()?;
        let meta_transition = r.read_typed()?;
        Ok(Self {
            unknown,
            animation_id_a,
            animation_id_b,
            meta_transition,
        })
    }
}

#[derive(Clone, Debug)]
enum MetaTransition {
    Animation(MetaAnimation),
    Transition {
        unknown1: f32,
        unknown2: u32,
        unknown3: bool,
        unknown4: bool,
        unknown5: u32,
    },
    PhaseTransition {
        unknown1: f32,
        unknown2: u32,
        unknown3: bool,
        unknown4: bool,
        unknown5: u32,
    },
    Snap,
}

impl ReadFrom for MetaTransition {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let kind = r.read_u32()?;
        Ok(match kind {
            0 => {
                let meta_animation = r.read_typed()?;
                MetaTransition::Animation(meta_animation)
            }
            1 => {
                let unknown1 = f32::from_bits(r.read_u32()?);
                let unknown2 = r.read_u32()?;
                let unknown3 = r.read_u8()? != 0;
                let unknown4 = r.read_u8()? != 0;
                let unknown5 = r.read_u32()?;
                MetaTransition::Transition {
                    unknown1,
                    unknown2,
                    unknown3,
                    unknown4,
                    unknown5,
                }
            }
            2 => {
                let unknown1 = f32::from_bits(r.read_u32()?);
                let unknown2 = r.read_u32()?;
                let unknown3 = r.read_u8()? != 0;
                let unknown4 = r.read_u8()? != 0;
                let unknown5 = r.read_u32()?;
                MetaTransition::PhaseTransition {
                    unknown1,
                    unknown2,
                    unknown3,
                    unknown4,
                    unknown5,
                }
            }
            3 => MetaTransition::Snap,
            _ => bail!("unexpected meta transition type: {}", kind),
        })
    }
}

#[derive(Clone, Debug)]
struct AdditiveAnimation {
    animation_id: u32,
    fade_in_time: f32,
    fade_out_time: f32,
}

impl ReadFrom for AdditiveAnimation {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let animation_id = r.read_u32()?;
        let fade_in_time = f32::from_bits(r.read_u32()?);
        let fade_out_time = f32::from_bits(r.read_u32()?);
        Ok(Self {
            animation_id,
            fade_in_time,
            fade_out_time,
        })
    }
}

#[derive(Clone, Debug)]
struct HalfTransition {
    animation_id: u32,
    meta_transition: MetaTransition,
}

impl ReadFrom for HalfTransition {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let animation_id = r.read_u32()?;
        let meta_transition = r.read_typed()?;
        Ok(Self {
            animation_id,
            meta_transition,
        })
    }
}

#[derive(Clone, Debug)]
struct AnimationResource {
    animation_id: u32,
    event_id: u32,
}

impl ReadFrom for AnimationResource {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let animation_id = r.read_u32()?;
        let event_id = r.read_u32()?;
        Ok(Self {
            animation_id,
            event_id,
        })
    }
}
