//! Explicit development-tool selection; the workspace default remains Chataigne.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppTarget {
    #[default]
    Chataigne,
    MapGyver,
}

impl AppTarget {
    pub fn cargo_package(self) -> &'static str {
        match self {
            Self::Chataigne => "Chataigne2",
            Self::MapGyver => "MapGyver",
        }
    }

    pub fn ui_directory(self) -> &'static str {
        match self {
            Self::Chataigne => "apps/chataigne/ui",
            Self::MapGyver => "apps/mapgyver/ui",
        }
    }

    pub fn frontend_port(self) -> u16 {
        match self {
            Self::Chataigne => 5173,
            Self::MapGyver => 5174,
        }
    }

    pub fn backend_port(self) -> u16 {
        match self {
            Self::Chataigne => 7010,
            Self::MapGyver => 7011,
        }
    }
}

impl std::str::FromStr for AppTarget {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "chataigne" => Ok(Self::Chataigne),
            "mapgyver" => Ok(Self::MapGyver),
            _ => Err(format!("unknown app '{value}'; expected chataigne or mapgyver")),
        }
    }
}
