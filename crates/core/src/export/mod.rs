//! Exporter format host ke aplikasi lain. Satu submodule per format
//! (baru ada satu sekarang: SecureCRT) — kebalikan dari `crate::import`,
//! taruh format baru (mis. JSON backup terenkripsi) di submodule
//! terpisah, jangan dicampur di sini.

pub mod securecrt;
