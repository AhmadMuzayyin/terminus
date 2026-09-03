//! Parser format import host dari aplikasi lain. Satu submodule per
//! format (baru ada satu sekarang: SecureCRT) — kalau nanti nambah
//! format lain (mis. mRemoteNG, PuTTY, OpenSSH `~/.ssh/config`), taruh
//! di submodule terpisah, jangan dicampur di sini.

pub mod securecrt;
