use std::fs;
use std::io::{Error, ErrorKind};
use std::os::unix;
use std::path::{Path, PathBuf};

use crate::config::Usb;

const CONFIGFS_PATH: &str = "/sys/kernel/config/usb_gadget";
const UDC_PATH: &str = "/sys/class/udc";
const NAME: &str = "rkvm";
const LANGUAGE: &str = "0x409";
const CONFIGURATION: &str = "c.1";

pub struct Function<'a> {
    pub name: &'a str,
    pub subclass: u8,
    pub protocol: u8,
    pub report_length: usize,
    pub report_descriptor: &'a [u8],
}

// A USB gadget composed of HID functions, configured through libcomposite's configfs interface.
//
// The gadget is unbound and removed on drop.
pub struct Gadget {
    path: PathBuf,
    udc: String,
}

impl Gadget {
    pub fn create(config: &Usb, functions: &[Function<'_>]) -> Result<Self, Error> {
        let root = Path::new(CONFIGFS_PATH);
        if !root.is_dir() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!(
                    "{} does not exist, make sure configfs is mounted and the libcomposite module is loaded",
                    CONFIGFS_PATH
                ),
            ));
        }

        let udc = match &config.udc {
            Some(udc) => udc.clone(),
            None => find_udc()?,
        };

        let path = root.join(NAME);
        if path.exists() {
            tracing::warn!(?path, "Removing stale USB gadget");
            remove(&path)?;
        }

        fs::create_dir(&path)?;

        // From now on, the gadget gets cleaned up on drop if anything fails.
        let gadget = Self { path, udc };
        gadget.setup(config, functions)?;

        Ok(gadget)
    }

    pub fn udc(&self) -> &str {
        &self.udc
    }

    // Returns the character device node of a function, only valid once the gadget is bound.
    pub fn device(&self, function: &str) -> Result<PathBuf, Error> {
        // In the major:minor format.
        let device_number = fs::read_to_string(self.function_path(function).join("dev"))?;
        let uevent = fs::read_to_string(
            Path::new("/sys/dev/char")
                .join(device_number.trim())
                .join("uevent"),
        )?;

        let name = uevent
            .lines()
            .find_map(|line| line.strip_prefix("DEVNAME="))
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::NotFound,
                    format!("No device name for function {}", function),
                )
            })?;

        Ok(Path::new("/dev").join(name))
    }

    fn setup(&self, config: &Usb, functions: &[Function<'_>]) -> Result<(), Error> {
        let path = &self.path;

        fs::write(path.join("idVendor"), format!("{:#06x}", config.vendor_id))?;
        fs::write(
            path.join("idProduct"),
            format!("{:#06x}", config.product_id),
        )?;
        fs::write(path.join("bcdDevice"), "0x0100")?;
        fs::write(path.join("bcdUSB"), "0x0200")?;

        let strings = path.join("strings").join(LANGUAGE);
        fs::create_dir(&strings)?;
        fs::write(strings.join("manufacturer"), &config.manufacturer)?;
        fs::write(strings.join("product"), &config.product)?;
        if let Some(serial_number) = &config.serial_number {
            fs::write(strings.join("serialnumber"), serial_number)?;
        }

        // The configuration has no description string, so that only the strings above identify the gadget.
        let configuration = path.join("configs").join(CONFIGURATION);
        fs::create_dir(&configuration)?;
        fs::write(configuration.join("MaxPower"), "100")?;

        for function in functions {
            let function_path = self.function_path(function.name);
            fs::create_dir(&function_path)?;
            fs::write(
                function_path.join("subclass"),
                function.subclass.to_string(),
            )?;
            fs::write(
                function_path.join("protocol"),
                function.protocol.to_string(),
            )?;
            fs::write(
                function_path.join("report_length"),
                function.report_length.to_string(),
            )?;
            fs::write(
                function_path.join("report_desc"),
                function.report_descriptor,
            )?;

            unix::fs::symlink(
                &function_path,
                configuration.join(format!("hid.{}", function.name)),
            )?;
        }

        fs::write(path.join("UDC"), &self.udc).map_err(|err| {
            Error::new(
                err.kind(),
                format!("Failed to bind gadget to UDC {}: {}", self.udc, err),
            )
        })?;

        Ok(())
    }

    fn function_path(&self, function: &str) -> PathBuf {
        self.path
            .join("functions")
            .join(format!("hid.{}", function))
    }
}

impl Drop for Gadget {
    fn drop(&mut self) {
        if let Err(err) = remove(&self.path) {
            tracing::warn!(path = ?self.path, "Error removing USB gadget: {}", err);
        }
    }
}

fn find_udc() -> Result<String, Error> {
    let not_found = || {
        Error::new(
            ErrorKind::NotFound,
            "No USB device controller found, make sure your hardware supports USB device mode and it is enabled",
        )
    };

    let entries = match fs::read_dir(UDC_PATH) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Err(not_found()),
        Err(err) => return Err(err),
    };

    let mut udcs = entries
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<Vec<_>, _>>()?;
    udcs.sort();

    let udc = udcs.first().ok_or_else(not_found)?;
    if udcs.len() > 1 {
        tracing::warn!(
            ?udcs,
            ?udc,
            "Multiple USB device controllers found, using the first one"
        );
    }

    udc.clone()
        .into_string()
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid UDC name"))
}

// Removal has to be done in reverse order of creation, configfs doesn't allow removing non-empty directories.
fn remove(path: &Path) -> Result<(), Error> {
    // Unbind the gadget first, this fails harmlessly if it's not bound.
    let _ = fs::write(path.join("UDC"), "\n");

    for configuration in read_dir(&path.join("configs"))? {
        for entry in read_dir(&configuration)? {
            if fs::symlink_metadata(&entry)?.file_type().is_symlink() {
                fs::remove_file(&entry)?;
            }
        }

        for strings in read_dir(&configuration.join("strings"))? {
            fs::remove_dir(strings)?;
        }

        fs::remove_dir(configuration)?;
    }

    for function in read_dir(&path.join("functions"))? {
        fs::remove_dir(function)?;
    }

    for strings in read_dir(&path.join("strings"))? {
        fs::remove_dir(strings)?;
    }

    fs::remove_dir(path)
}

fn read_dir(path: &Path) -> Result<Vec<PathBuf>, Error> {
    fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect()
}
