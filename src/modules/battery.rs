use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter, Write};
use std::path::Path;
use udev::Device;
use crate::modules::{Module, Result, Error, Named};

enum ChargeState {
    Charging,
    Discharging,
    Full,
}

impl Display for ChargeState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Charging => "Charging",
            Self::Discharging => "Discharging",
            Self::Full => "Full",
        })
    }
}

impl Default for ChargeState {
    fn default() -> Self {
        ChargeState::Charging
    }
}

pub struct BatteryModule {
    device: Device,
    charge: u32,
    charge_state: ChargeState,
}

impl BatteryModule {
    pub fn init() -> Result<Self> {
        const PATH: &str = "/sys/class/power_supply/BAT0";
        // TODO optionally grab device path from config
        let device = udev::Device::from_syspath(&Path::new(PATH))?;

        let charge = device.attribute_value("capacity")
            .and_then(OsStr::to_str)
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or(Error::udev_invalid_device_attribute(PATH, "capacity"))?;

        //dev.attribute_value("status");
        let charge_state = ChargeState::Charging;

        Ok(BatteryModule {
            device,
            charge,
            charge_state,
        })
    }
}

impl Named for BatteryModule {
    const NAME: &'static str = "battery";
}

impl Module for BatteryModule {
    fn write(&self, field: &str, dst: &mut String) -> Result<bool> {
        match field {
            "charge" => { write!(dst, "{}", self.charge).expect("Write failed");},
            "charge_state" => {write!(dst, "{}", self.charge_state).expect("Write failed");},
            _ => eprintln!("Unrecognized requested field {}:{}, leaving string empty.", Self::NAME, field)
        }
        Ok(true)
    }
}