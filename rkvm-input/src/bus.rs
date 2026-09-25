use crate::convert::Convert;
use crate::glue;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Bus {
    Pci,
    IsaPnp,
    Usb,
    Hil,
    Bluetooth,
    Virtual,
    Isa,
    I8042,
    XtKbd,
    Rs232,
    GamePort,
    ParPort,
    Amiga,
    Adb,
    I2c,
    Host,
    Gsc,
    Atari,
    Spi,
    Rmi,
    Cec,
    IntelIshtp,
    AmdSfh,
    Sdw,
}

impl Convert for Bus {
    type Raw = u16;

    fn from_raw(raw: Self::Raw) -> Option<Self> {
        let bus = match raw as _ {
            glue::BUS_PCI => Self::Pci,
            glue::BUS_ISAPNP => Self::IsaPnp,
            glue::BUS_USB => Self::Usb,
            glue::BUS_HIL => Self::Hil,
            glue::BUS_BLUETOOTH => Self::Bluetooth,
            glue::BUS_VIRTUAL => Self::Virtual,
            glue::BUS_ISA => Self::Isa,
            glue::BUS_I8042 => Self::I8042,
            glue::BUS_XTKBD => Self::XtKbd,
            glue::BUS_RS232 => Self::Rs232,
            glue::BUS_GAMEPORT => Self::GamePort,
            glue::BUS_PARPORT => Self::ParPort,
            glue::BUS_AMIGA => Self::Amiga,
            glue::BUS_ADB => Self::Adb,
            glue::BUS_I2C => Self::I2c,
            glue::BUS_HOST => Self::Host,
            glue::BUS_GSC => Self::Gsc,
            glue::BUS_ATARI => Self::Atari,
            glue::BUS_SPI => Self::Spi,
            glue::BUS_RMI => Self::Rmi,
            glue::BUS_CEC => Self::Cec,
            glue::BUS_INTEL_ISHTP => Self::IntelIshtp,
            glue::BUS_AMD_SFH => Self::AmdSfh,
            #[cfg(have_bus_sdw)]
            glue::BUS_SDW => Self::Sdw,
            _ => return None,
        };

        Some(bus)
    }

    fn to_raw(&self) -> Option<Self::Raw> {
        let raw = match self {
            Self::Pci => glue::BUS_PCI,
            Self::IsaPnp => glue::BUS_ISAPNP,
            Self::Usb => glue::BUS_USB,
            Self::Hil => glue::BUS_HIL,
            Self::Bluetooth => glue::BUS_BLUETOOTH,
            Self::Virtual => glue::BUS_VIRTUAL,
            Self::Isa => glue::BUS_ISA,
            Self::I8042 => glue::BUS_I8042,
            Self::XtKbd => glue::BUS_XTKBD,
            Self::Rs232 => glue::BUS_RS232,
            Self::GamePort => glue::BUS_GAMEPORT,
            Self::ParPort => glue::BUS_PARPORT,
            Self::Amiga => glue::BUS_AMIGA,
            Self::Adb => glue::BUS_ADB,
            Self::I2c => glue::BUS_I2C,
            Self::Host => glue::BUS_HOST,
            Self::Gsc => glue::BUS_GSC,
            Self::Atari => glue::BUS_ATARI,
            Self::Spi => glue::BUS_SPI,
            Self::Rmi => glue::BUS_RMI,
            Self::Cec => glue::BUS_CEC,
            Self::IntelIshtp => glue::BUS_INTEL_ISHTP,
            Self::AmdSfh => glue::BUS_AMD_SFH,
            #[cfg(have_bus_sdw)]
            Self::Sdw => glue::BUS_SDW,
            #[cfg(not(have_bus_sdw))]
            Self::Sdw => return None,
        };

        Some(raw as _)
    }
}
