pub type AddonCall = extern "Rust" fn(Vec<AddonValue>) -> AddonValue;

pub struct AddonFn {
    pub name: &'static str,
    pub call: AddonCall
}

#[cfg(feature = "addon")]
inventory::collect!(AddonFn);

pub enum AddonValue {
    String(String),
    Number(i32),
    Bool(bool),
    Void
}
