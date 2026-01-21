#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorThemeId {
    GlacierCoast,
    NightHarbor,
    SlateDawn,
    AuroraDrift,
    DeepForest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ColorRgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticPalette {
    pub success: ColorRgb,
    pub warn: ColorRgb,
    pub error: ColorRgb,
    pub info: ColorRgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrayscalePalette {
    pub low: ColorRgb,
    pub high: ColorRgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTheme {
    pub id: ColorThemeId,
    pub name: &'static str,
    pub base: ColorRgb,
    pub primary: ColorRgb,
    pub secondary: ColorRgb,
    pub semantic: SemanticPalette,
    pub grayscale: GrayscalePalette,
}

impl ColorThemeId {
    pub fn name(self) -> &'static str {
        self.theme().name
    }

    pub fn theme(self) -> ColorTheme {
        match self {
            ColorThemeId::GlacierCoast => ColorTheme {
                id: self,
                name: "Glacier Coast",
                base: ColorRgb::new(0x4E, 0x6C, 0x92),
                primary: ColorRgb::new(0x8A, 0xB4, 0xF8),
                secondary: ColorRgb::new(0x6A, 0xA0, 0xE7),
                semantic: SemanticPalette {
                    success: ColorRgb::new(0x5E, 0xC3, 0x8F),
                    warn: ColorRgb::new(0xF4, 0xB0, 0x4C),
                    error: ColorRgb::new(0xE3, 0x8A, 0x90),
                    info: ColorRgb::new(0x5D, 0xD0, 0xFF),
                },
                grayscale: GrayscalePalette {
                    low: ColorRgb::new(0x1F, 0x24, 0x2A),
                    high: ColorRgb::new(0xDD, 0xE4, 0xED),
                },
            },
            ColorThemeId::NightHarbor => ColorTheme {
                id: self,
                name: "Night Harbor",
                base: ColorRgb::new(0x3E, 0x5A, 0x63),
                primary: ColorRgb::new(0x7E, 0xD0, 0xD9),
                secondary: ColorRgb::new(0x5C, 0xA9, 0xB4),
                semantic: SemanticPalette {
                    success: ColorRgb::new(0x3C, 0xC4, 0x8C),
                    warn: ColorRgb::new(0xF2, 0xA6, 0x66),
                    error: ColorRgb::new(0xD6, 0x72, 0x78),
                    info: ColorRgb::new(0x5F, 0xB7, 0xFF),
                },
                grayscale: GrayscalePalette {
                    low: ColorRgb::new(0x21, 0x26, 0x2A),
                    high: ColorRgb::new(0xCE, 0xD7, 0xE0),
                },
            },
            ColorThemeId::SlateDawn => ColorTheme {
                id: self,
                name: "Slate Dawn",
                base: ColorRgb::new(0x4B, 0x51, 0x61),
                primary: ColorRgb::new(0x9A, 0xA2, 0xB7),
                secondary: ColorRgb::new(0x7C, 0x86, 0xA3),
                semantic: SemanticPalette {
                    success: ColorRgb::new(0x6B, 0xC8, 0xA4),
                    warn: ColorRgb::new(0xF0, 0xAD, 0x6C),
                    error: ColorRgb::new(0xDA, 0x7D, 0x7D),
                    info: ColorRgb::new(0x76, 0xB0, 0xFF),
                },
                grayscale: GrayscalePalette {
                    low: ColorRgb::new(0x20, 0x25, 0x2E),
                    high: ColorRgb::new(0xD3, 0xD7, 0xDF),
                },
            },
            ColorThemeId::AuroraDrift => ColorTheme {
                id: self,
                name: "Aurora Drift",
                base: ColorRgb::new(0x4E, 0x5F, 0x4F),
                primary: ColorRgb::new(0xA4, 0xD8, 0xC3),
                secondary: ColorRgb::new(0x7E, 0xBC, 0xA8),
                semantic: SemanticPalette {
                    success: ColorRgb::new(0x50, 0xC0, 0x73),
                    warn: ColorRgb::new(0xF7, 0xBF, 0x5C),
                    error: ColorRgb::new(0xE0, 0x82, 0x7A),
                    info: ColorRgb::new(0x8F, 0xD2, 0xFF),
                },
                grayscale: GrayscalePalette {
                    low: ColorRgb::new(0x1E, 0x24, 0x20),
                    high: ColorRgb::new(0xD7, 0xE3, 0xD7),
                },
            },
            ColorThemeId::DeepForest => ColorTheme {
                id: self,
                name: "Deep Forest",
                base: ColorRgb::new(0x3F, 0x4F, 0x46),
                primary: ColorRgb::new(0x8A, 0xC6, 0xA5),
                secondary: ColorRgb::new(0x65, 0xA0, 0x8A),
                semantic: SemanticPalette {
                    success: ColorRgb::new(0x48, 0xB8, 0x7B),
                    warn: ColorRgb::new(0xF0, 0xB1, 0x5A),
                    error: ColorRgb::new(0xDD, 0x7B, 0x80),
                    info: ColorRgb::new(0x7D, 0xAF, 0xFF),
                },
                grayscale: GrayscalePalette {
                    low: ColorRgb::new(0x1C, 0x22, 0x1C),
                    high: ColorRgb::new(0xCC, 0xD7, 0xD3),
                },
            },
        }
    }

    pub fn all() -> &'static [ColorThemeId] {
        &[
            ColorThemeId::GlacierCoast,
            ColorThemeId::NightHarbor,
            ColorThemeId::SlateDawn,
            ColorThemeId::AuroraDrift,
            ColorThemeId::DeepForest,
        ]
    }

    pub fn from_name(name: &str) -> Option<ColorThemeId> {
        let normalized = name.trim();
        Self::all()
            .iter()
            .copied()
            .find(|id| id.name().eq_ignore_ascii_case(normalized))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_palette_matches_spec() {
        let theme = ColorThemeId::GlacierCoast.theme();

        assert_eq!(theme.name, "Glacier Coast");
        assert_eq!(theme.base, ColorRgb::new(0x4E, 0x6C, 0x92));
        assert_eq!(theme.primary, ColorRgb::new(0x8A, 0xB4, 0xF8));
        assert_eq!(theme.secondary, ColorRgb::new(0x6A, 0xA0, 0xE7));
        assert_eq!(theme.semantic.success, ColorRgb::new(0x5E, 0xC3, 0x8F));
        assert_eq!(theme.semantic.warn, ColorRgb::new(0xF4, 0xB0, 0x4C));
        assert_eq!(theme.semantic.error, ColorRgb::new(0xE3, 0x8A, 0x90));
        assert_eq!(theme.semantic.info, ColorRgb::new(0x5D, 0xD0, 0xFF));
        assert_eq!(theme.grayscale.low, ColorRgb::new(0x1F, 0x24, 0x2A));
        assert_eq!(theme.grayscale.high, ColorRgb::new(0xDD, 0xE4, 0xED));
    }

    #[test]
    fn theme_list_has_minimum_entries() {
        assert!(ColorThemeId::all().len() >= 5);
    }

    #[test]
    fn theme_lookup_is_case_insensitive() {
        assert_eq!(
            ColorThemeId::from_name("glacier coast"),
            Some(ColorThemeId::GlacierCoast)
        );
    }
}
