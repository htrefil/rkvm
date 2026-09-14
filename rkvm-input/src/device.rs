use serde::Deserialize;

/// Describes parts of a device
#[derive(Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct DeviceSpec {
    pub name: Option<std::ffi::CString>,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
}

impl DeviceSpec {
    /// Compares the given values to this DeviceSpec
    ///
    /// A None value means we skip that comparison
    pub fn matches(
        &self,
        other_name: &std::ffi::CStr,
        other_vendor_id: &u16,
        other_product_id: &u16,
    ) -> bool {
        if let Some(name) = &self.name {
            if name.as_c_str() != other_name {
                return false;
            }
        }

        if let Some(vendor_id) = &self.vendor_id {
            if vendor_id != other_vendor_id {
                return false;
            }
        }

        if let Some(product_id) = &self.product_id {
            if product_id != other_product_id {
                return false;
            }
        }

        true
    }
}
