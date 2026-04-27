use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Theme::Light => write!(f, "light"),
            Theme::Dark => write!(f, "dark"),
        }
    }
}

impl std::str::FromStr for Theme {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "light" => Ok(Theme::Light),
            "dark" => Ok(Theme::Dark),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UsageDto {
    pub processor: i32,
    pub ram: i32,
    pub storage: i32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ProcessorDto {
    pub name: String,
    #[serde(rename = "coreCount")]
    pub core_count: String,
    #[serde(rename = "clockSpeed")]
    pub clock_speed: String,
    #[serde(rename = "bitDepth")]
    pub bit_depth: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MachineDto {
    #[serde(rename = "operatingSystem")]
    pub operating_system: String,
    #[serde(rename = "totalRam")]
    pub total_ram: String,
    #[serde(rename = "ramTypeOrOSBitDepth")]
    pub ram_type_or_os_bit_depth: String,
    #[serde(rename = "procCount")]
    pub proc_count: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct StorageDto {
    #[serde(rename = "mainStorage")]
    pub main_storage: String,
    pub total: String,
    #[serde(rename = "diskCount")]
    pub disk_count: String,
    #[serde(rename = "swapAmount")]
    pub swap_amount: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct InfoDto {
    pub processor: ProcessorDto,
    pub machine: MachineDto,
    pub storage: StorageDto,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UptimeDto {
    pub days: String,
    pub hours: String,
    pub minutes: String,
    pub seconds: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SetupDto {
    #[serde(rename = "serverName")]
    pub server_name: String,
    pub theme: Theme,
    pub port: u16,
    #[serde(rename = "enableFog")]
    pub enable_fog: bool,
    #[serde(rename = "backgroundColor")]
    pub background_color: String,
}

impl SetupDto {
    pub fn validate(&self) -> Result<(), String> {
        let server_name = self.server_name.trim();
        if server_name.is_empty() {
            return Err("serverName cannot be empty".to_string());
        }
        if server_name.chars().count() > 32 {
            return Err("serverName too long".to_string());
        }
        if self.port < 1024 {
            return Err("port must be in range 1024-65535".to_string());
        }
        if self.background_color != "default" {
            let bytes = self.background_color.as_bytes();
            let is_hex = bytes.len() == 7
                && bytes[0] == b'#'
                && bytes[1..].iter().all(|b| b.is_ascii_hexdigit());
            if !is_hex {
                return Err(
                    "backgroundColor must be \"default\" or a hex color like \"#RRGGBB\""
                        .to_string(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ResponseDto {
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ErrorDto {
    pub message: String,
    pub exception: String,
    pub timestamp: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_setup() -> SetupDto {
        SetupDto {
            server_name: "Ward".to_string(),
            theme: Theme::Light,
            port: 4000,
            enable_fog: true,
            background_color: "default".to_string(),
        }
    }

    #[test]
    fn validate_ok_default_background() {
        let dto = base_setup();
        assert!(dto.validate().is_ok());
    }

    #[test]
    fn validate_ok_hex_background() {
        let mut dto = base_setup();
        dto.background_color = "#a1B2c3".to_string();
        assert!(dto.validate().is_ok());
    }

    #[test]
    fn validate_err_empty_server_name() {
        let mut dto = base_setup();
        dto.server_name = "   ".to_string();
        assert!(dto.validate().is_err());
    }

    #[test]
    fn validate_err_long_server_name() {
        let mut dto = base_setup();
        dto.server_name = "012345678901234567890123456789012".to_string();
        assert!(dto.validate().is_err());
    }

    #[test]
    fn validate_err_low_port() {
        let mut dto = base_setup();
        dto.port = 80;
        assert!(dto.validate().is_err());
    }

    #[test]
    fn validate_err_invalid_hex_background_missing_hash() {
        let mut dto = base_setup();
        dto.background_color = "a1b2c3".to_string();
        assert!(dto.validate().is_err());
    }

    #[test]
    fn validate_err_invalid_hex_background_wrong_length() {
        let mut dto = base_setup();
        dto.background_color = "#12345".to_string();
        assert!(dto.validate().is_err());
    }

    #[test]
    fn validate_err_invalid_hex_background_non_hex() {
        let mut dto = base_setup();
        dto.background_color = "#12zzzz".to_string();
        assert!(dto.validate().is_err());
    }
}
