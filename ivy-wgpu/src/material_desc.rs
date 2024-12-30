use ivy_assets::{loadable::Load, Asset, AssetCache, AssetDesc, DynAssetDesc};
use ivy_gltf::GltfMaterial;
use ivy_graphics::texture::{TextureData, TextureDesc};
use ordered_float::NotNan;
use wgpu::TextureFormat;

use crate::{
    material::{
        emissive::PbrEmissiveMaterialParams, PbrMaterialParams, RenderMaterial, ShadowMaterialDesc,
    },
    shader::ShaderPass,
    shaders::{EmissiveShaderDesc, PbrShaderDesc, UnlitShaderDesc},
    texture::TextureWithFormatDesc,
};

/// Asynchronously loadable material, e.g; from json and texture file paths
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MaterialDesc {
    PbrMaterial(PbrMaterialDesc),
    UnlitMaterial(PbrMaterialDesc),
    EmissiveMaterial(PbrEmissiveMaterialDesc),
    ShadowMaterial,
}

impl Load for MaterialDesc {
    type Output = MaterialData;

    type Error = anyhow::Error;

    async fn load(self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
        match self {
            MaterialDesc::PbrMaterial(desc) => {
                Ok(MaterialData::PbrMaterial(desc.load(assets).await?))
            }
            MaterialDesc::UnlitMaterial(desc) => {
                Ok(MaterialData::UnlitMaterial(desc.load(assets).await?))
            }
            MaterialDesc::EmissiveMaterial(desc) => {
                Ok(MaterialData::EmissiveMaterial(desc.load(assets).await?))
            }
            MaterialDesc::ShadowMaterial => Ok(MaterialData::ShadowMaterial),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PbrMaterialDesc {
    label: String,
    #[serde(default = "TextureDesc::white")]
    albedo: TextureDesc,
    #[serde(default = "TextureDesc::default_normal")]
    normal: TextureDesc,
    #[serde(default = "TextureDesc::white")]
    metallic_roughness: TextureDesc,
    #[serde(default = "TextureDesc::white")]
    ambient_occlusion: TextureDesc,
    #[serde(default = "TextureDesc::white")]
    displacement: TextureDesc,
    roughness_factor: NotNan<f32>,
    metallic_factor: NotNan<f32>,
}

impl Load for PbrMaterialDesc {
    type Output = PbrMaterialData;

    type Error = anyhow::Error;

    async fn load(self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
        Ok(Self::Output {
            label: self.label,
            albedo: self.albedo.load(assets).await?,
            normal: self.normal.load(assets).await?,
            metallic_roughness: self.metallic_roughness.load(assets).await?,
            ambient_occlusion: self.ambient_occlusion.load(assets).await?,
            displacement: self.displacement.load(assets).await?,
            roughness_factor: self.roughness_factor,
            metallic_factor: self.metallic_factor,
        })
    }
}

impl PbrMaterialDesc {
    pub fn new() -> Self {
        Self {
            albedo: TextureDesc::white(),
            normal: TextureDesc::default_normal(),
            metallic_roughness: TextureDesc::white(),
            ambient_occlusion: TextureDesc::white(),
            displacement: TextureDesc::white(),
            roughness_factor: 1.0.try_into().unwrap(),
            metallic_factor: 1.0.try_into().unwrap(),
            label: "unknown_material".into(),
        }
    }

    /// Set the label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set the albedo
    pub fn with_albedo(mut self, albedo: impl Into<TextureDesc>) -> Self {
        self.albedo = albedo.into();
        self
    }

    /// Set the normal
    pub fn with_normal(mut self, normal: impl Into<TextureDesc>) -> Self {
        self.normal = normal.into();
        self
    }

    /// Set the metallic roughness
    pub fn with_metallic_roughness(mut self, metallic_roughness: impl Into<TextureDesc>) -> Self {
        self.metallic_roughness = metallic_roughness.into();
        self
    }

    /// Set the ambient occlusion
    pub fn with_ambient_occlusion(mut self, ambient_occlusion: impl Into<TextureDesc>) -> Self {
        self.ambient_occlusion = ambient_occlusion.into();
        self
    }

    /// Set the displacement
    pub fn with_displacement(mut self, displacement: impl Into<TextureDesc>) -> Self {
        self.displacement = displacement.into();
        self
    }

    /// Set the roughness factor
    pub fn with_roughness_factor(mut self, roughness: f32) -> Self {
        self.roughness_factor = roughness.try_into().unwrap();
        self
    }

    /// Set the metallic factor
    pub fn with_metallic_factor(mut self, metallic: f32) -> Self {
        self.metallic_factor = metallic.try_into().unwrap();
        self
    }
}

impl Default for PbrMaterialDesc {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PbrEmissiveMaterialDesc {
    pbr: PbrMaterialDesc,
    emissive_color: TextureDesc,
    emissive_factor: NotNan<f32>,
}

impl Load for PbrEmissiveMaterialDesc {
    type Output = PbrEmissiveMaterialData;

    type Error = anyhow::Error;

    async fn load(self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
        Ok(Self::Output {
            pbr: self.pbr.load(assets).await?,
            emissive_color: self.emissive_color.load(assets).await?,
            emissive_factor: self.emissive_factor,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MaterialData {
    PbrMaterial(PbrMaterialData),
    UnlitMaterial(PbrMaterialData),
    EmissiveMaterial(PbrEmissiveMaterialData),
    ShadowMaterial,
}

impl From<PbrMaterialData> for MaterialData {
    fn from(v: PbrMaterialData) -> Self {
        Self::PbrMaterial(v)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PbrMaterialData {
    label: String,
    albedo: TextureData,
    normal: TextureData,
    metallic_roughness: TextureData,
    ambient_occlusion: TextureData,
    displacement: TextureData,
    roughness_factor: NotNan<f32>,
    metallic_factor: NotNan<f32>,
}

impl PbrMaterialData {
    pub fn new() -> Self {
        Self {
            albedo: TextureData::white(),
            normal: TextureData::default_normal(),
            metallic_roughness: TextureData::white(),
            ambient_occlusion: TextureData::white(),
            displacement: TextureData::white(),
            roughness_factor: 1.0.try_into().unwrap(),
            metallic_factor: 1.0.try_into().unwrap(),
            label: "unknown_material".into(),
        }
    }

    pub fn from_gltf_material(material: GltfMaterial) -> Self {
        let textures = material.data().images();

        let material = material.material();
        let pbr = material.pbr_metallic_roughness();

        let mut material_data = PbrMaterialData::new();

        if let Some(albedo) = pbr.base_color_texture() {
            let texture = textures[albedo.texture().index()].clone();
            material_data.albedo = TextureData::Content(texture);
        }

        if let Some(normal) = material.normal_texture() {
            let texture = textures[normal.texture().index()].clone();
            material_data.normal = TextureData::Content(texture);
        }

        // TODO: some kind of preprocess for e.g; grayscale roughness only maps
        if let Some(metallic_roughness) = pbr.metallic_roughness_texture() {
            let texture = textures[metallic_roughness.texture().index()].clone();
            material_data.metallic_roughness = TextureData::Content(texture);
        }

        material_data.metallic_factor = NotNan::new(pbr.metallic_factor()).unwrap();
        material_data.roughness_factor = NotNan::new(pbr.roughness_factor()).unwrap();

        material_data
    }

    fn create(
        &self,
        assets: &AssetCache,
        shader: Asset<ShaderPass>,
    ) -> anyhow::Result<Asset<RenderMaterial>> {
        let albedo = assets.try_load(&TextureWithFormatDesc::new(
            self.albedo.clone(),
            TextureFormat::Rgba8UnormSrgb,
        ))?;

        let normal = assets.try_load(&TextureWithFormatDesc::new(
            self.normal.clone(),
            TextureFormat::Rgba8Unorm,
        ))?;

        let metallic_roughness = assets.try_load(&TextureWithFormatDesc::new(
            self.metallic_roughness.clone(),
            TextureFormat::Rgba8Unorm,
        ))?;

        let ambient_occlusion = assets.try_load(&TextureWithFormatDesc::new(
            self.ambient_occlusion.clone(),
            TextureFormat::Rgba8Unorm,
        ))?;

        let displacement = assets.try_load(&TextureWithFormatDesc::new(
            self.displacement.clone(),
            TextureFormat::Rgba8Unorm,
        ))?;

        Ok(assets.insert(
            PbrMaterialParams {
                albedo,
                normal,
                metallic_roughness,
                ambient_occlusion,
                displacement,
                roughness_factor: *self.roughness_factor,
                metallic_factor: *self.metallic_factor,
                shader,
            }
            .create_material(self.label.clone(), assets),
        ))
    }

    /// Set the label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set the albedo
    pub fn with_albedo(mut self, albedo: impl Into<TextureData>) -> Self {
        self.albedo = albedo.into();
        self
    }

    /// Set the normal
    pub fn with_normal(mut self, normal: impl Into<TextureData>) -> Self {
        self.normal = normal.into();
        self
    }

    /// Set the metallic roughness
    pub fn with_metallic_roughness(mut self, metallic_roughness: impl Into<TextureData>) -> Self {
        self.metallic_roughness = metallic_roughness.into();
        self
    }

    /// Set the ambient occlusion
    pub fn with_ambient_occlusion(mut self, ambient_occlusion: impl Into<TextureData>) -> Self {
        self.ambient_occlusion = ambient_occlusion.into();
        self
    }

    /// Set the displacement
    pub fn with_displacement(mut self, displacement: impl Into<TextureData>) -> Self {
        self.displacement = displacement.into();
        self
    }

    /// Set the roughness factor
    pub fn with_roughness_factor(mut self, roughness: f32) -> Self {
        self.roughness_factor = roughness.try_into().unwrap();
        self
    }

    /// Set the metallic factor
    pub fn with_metallic_factor(mut self, metallic: f32) -> Self {
        self.metallic_factor = metallic.try_into().unwrap();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PbrEmissiveMaterialData {
    pbr: PbrMaterialData,
    emissive_color: TextureData,
    emissive_factor: NotNan<f32>,
}

impl PbrEmissiveMaterialData {
    pub fn new(pbr: PbrMaterialData, emissive_color: TextureData, emissive_factor: f32) -> Self {
        Self {
            pbr,
            emissive_color,
            emissive_factor: NotNan::new(emissive_factor).unwrap(),
        }
    }

    fn create(
        &self,
        assets: &AssetCache,
        shader: Asset<ShaderPass>,
    ) -> anyhow::Result<Asset<RenderMaterial>> {
        let albedo = assets.try_load(&TextureWithFormatDesc::new(
            self.pbr.albedo.clone(),
            TextureFormat::Rgba8UnormSrgb,
        ))?;

        let normal = assets.try_load(&TextureWithFormatDesc::new(
            self.pbr.normal.clone(),
            TextureFormat::Rgba8Unorm,
        ))?;

        let metallic_roughness = assets.try_load(&TextureWithFormatDesc::new(
            self.pbr.metallic_roughness.clone(),
            TextureFormat::Rgba8Unorm,
        ))?;

        let ambient_occlusion = assets.try_load(&TextureWithFormatDesc::new(
            self.pbr.ambient_occlusion.clone(),
            TextureFormat::Rgba8Unorm,
        ))?;

        let displacement = assets.try_load(&TextureWithFormatDesc::new(
            self.pbr.displacement.clone(),
            TextureFormat::Rgba8Unorm,
        ))?;

        let emissive_color = assets.try_load(&TextureWithFormatDesc::new(
            self.emissive_color.clone(),
            TextureFormat::Rgba8Unorm,
        ))?;

        Ok(assets.insert(
            PbrEmissiveMaterialParams {
                pbr: PbrMaterialParams {
                    albedo,
                    normal,
                    metallic_roughness,
                    ambient_occlusion,
                    displacement,
                    roughness_factor: *self.pbr.roughness_factor,
                    metallic_factor: *self.pbr.metallic_factor,
                    shader,
                },
                emissive_color,
                emissive_factor: *self.emissive_factor,
            }
            .create_material(self.pbr.label.clone(), assets),
        ))
    }
}

impl Default for PbrMaterialData {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetDesc<RenderMaterial> for MaterialData {
    type Error = anyhow::Error;

    fn create(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> Result<Asset<RenderMaterial>, Self::Error> {
        match self {
            MaterialData::PbrMaterial(v) => v.create(assets, PbrShaderDesc.load(assets)),
            MaterialData::UnlitMaterial(v) => v.create(assets, UnlitShaderDesc.load(assets)),
            MaterialData::EmissiveMaterial(v) => v.create(assets, EmissiveShaderDesc.load(assets)),
            MaterialData::ShadowMaterial => {
                Ok(assets.insert(ShadowMaterialDesc {}.create_material("shadow".into(), assets)))
            }
        }
    }
}
