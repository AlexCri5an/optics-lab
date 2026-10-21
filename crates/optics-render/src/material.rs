//! GPU material representation for multi-material spectral dispersion and BSDF shading.

use bytemuck::{Pod, Zeroable};

/// Material type classification for WGSL shader evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum GpuMaterialType {
    /// Transparent dielectric with Sellmeier spectral dispersion.
    DielectricSellmeier = 0,
    /// Glossy GGX specular surface (e.g. floor plane, metallic mirrors).
    GlossyGgx = 1,
    /// Lambertian diffuse receiver or absorber.
    Diffuse = 2,
}

/// GPU storage buffer layout for optical materials.
///
/// 16-byte aligned, 64-byte size, matching WGSL struct layout.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct GpuMaterial {
    /// Sellmeier B coefficients: [B1, B2, B3, 0.0].
    pub b: [f32; 4],
    /// Sellmeier C coefficients: [C1, C2, C3, 0.0].
    pub c: [f32; 4],
    /// Material properties: [material_type, roughness, ior_constant, pad].
    pub properties: [f32; 4],
    /// Base albedo / tint: [r, g, b, pad].
    pub albedo: [f32; 4],
}

impl Default for GpuMaterial {
    fn default() -> Self {
        Self::n_bk7()
    }
}

impl GpuMaterial {
    /// Creates a dielectric material with custom Sellmeier coefficients.
    #[must_use]
    pub fn sellmeier(b: [f32; 3], c: [f32; 3]) -> Self {
        Self {
            b: [b[0], b[1], b[2], 0.0],
            c: [c[0], c[1], c[2], 0.0],
            properties: [GpuMaterialType::DielectricSellmeier as u32 as f32, 0.0, 1.0, 0.0],
            albedo: [1.0, 1.0, 1.0, 1.0],
        }
    }

    /// Canonical Schott N-BK7 optical glass.
    #[must_use]
    pub fn n_bk7() -> Self {
        Self::sellmeier(
            [1.039_612_2_f32, 0.231_792_35_f32, 1.010_469_4_f32],
            [0.006_000_698_5_f32, 0.020_017_914_f32, 103.560_65_f32],
        )
    }

    /// Canonical Schott SF11 dense flint optical glass.
    #[must_use]
    pub fn sf11() -> Self {
        Self::sellmeier(
            [1.737_597_f32, 0.313_747_35_f32, 1.898_781_f32],
            [0.013_188_707_f32, 0.062_306_814_f32, 155.236_3_f32],
        )
    }

    /// Canonical Schott F2 flint optical glass.
    #[must_use]
    pub fn f2() -> Self {
        Self::sellmeier(
            [1.345_333_6_f32, 0.209_073_18_f32, 0.937_357_2_f32],
            [0.009_977_439_f32, 0.047_045_077_f32, 111.886_765_f32],
        )
    }

    /// Canonical Schott N-SK16 dense crown optical glass.
    #[must_use]
    pub fn sk16() -> Self {
        Self::sellmeier(
            [1.332_306_8_f32, 0.145_558_94_f32, 0.957_582_24_f32],
            [0.008_447_386_f32, 0.038_487_737_f32, 107.039_02_f32],
        )
    }

    /// Canonical Corning 7980 Fused Silica.
    #[must_use]
    pub fn fused_silica() -> Self {
        Self::sellmeier(
            [0.696_166_3_f32, 0.407_942_6_f32, 0.897_479_4_f32],
            [0.004_679_148_f32, 0.013_512_06_f32, 97.934_006_f32],
        )
    }

    /// Glossy specular surface (GGX microfacet).
    #[must_use]
    pub fn glossy(roughness: f32, f0: f32, albedo: [f32; 3]) -> Self {
        Self {
            b: [0.0; 4],
            c: [0.0; 4],
            properties: [
                GpuMaterialType::GlossyGgx as u32 as f32,
                roughness.clamp(0.001, 1.0),
                f0.clamp(0.0, 1.0),
                0.0,
            ],
            albedo: [albedo[0], albedo[1], albedo[2], 1.0],
        }
    }

    /// Lambertian diffuse receiver or absorber.
    #[must_use]
    pub fn diffuse(albedo: [f32; 3]) -> Self {
        Self {
            b: [0.0; 4],
            c: [0.0; 4],
            properties: [GpuMaterialType::Diffuse as u32 as f32, 1.0, 1.0, 0.0],
            albedo: [albedo[0], albedo[1], albedo[2], 1.0],
        }
    }

    /// Creates a material from a recognized glass name or defaults to N-BK7.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_uppercase().replace(['-', '_', ' '], "").as_str() {
            "NBK7" | "BK7" => Self::n_bk7(),
            "SF11" => Self::sf11(),
            "F2" => Self::f2(),
            "NSK16" | "SK16" => Self::sk16(),
            "FUSEDSILICA" | "SILICA" | "QUARTZ" => Self::fused_silica(),
            _ => Self::n_bk7(),
        }
    }
}

/// Packs an array of [`GpuMaterial`] elements into a contiguous byte buffer for WebGPU storage binding.
#[must_use]
pub fn pack_materials_to_gpu_buffer(materials: &[GpuMaterial]) -> Vec<u8> {
    if materials.is_empty() {
        // Guarantee at least one valid material (64 bytes) to satisfy wgpu minimum buffer sizing
        bytemuck::bytes_of(&GpuMaterial::default()).to_vec()
    } else {
        bytemuck::cast_slice(materials).to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_gpu_material_layout() -> TestResult {
        assert_eq!(
            std::mem::align_of::<GpuMaterial>(),
            16,
            "GpuMaterial must be 16-byte aligned"
        );
        assert_eq!(
            std::mem::size_of::<GpuMaterial>(),
            64,
            "GpuMaterial must be exactly 64 bytes"
        );
        Ok(())
    }

    #[test]
    fn test_pack_materials() -> TestResult {
        let mats = [GpuMaterial::n_bk7(), GpuMaterial::sf11(), GpuMaterial::f2()];
        let bytes = pack_materials_to_gpu_buffer(&mats);
        assert_eq!(bytes.len(), 64 * 3);
        Ok(())
    }
}
