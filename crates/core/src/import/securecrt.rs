//! Parser `config.xml` export dari VanDyke SecureCRT.
//!
//! Formatnya: satu root `<VanDyke>` isinya banyak `<key name="...">`
//! (mirip registry Windows), sebagian besar cuma preferensi aplikasi
//! (warna, font, dsb) yang TIDAK kita pedulikan. Yang relevan cuma
//! subtree `<key name="Sessions">` — isinya tree `<key>` bersarang:
//! tiap `<key>` adalah FOLDER (kalau isinya `<key>` lain lagi) ATAU
//! SESI host beneran (ditandai anak `<dword name="Is Session">1</dword>`).
//!
//! Contoh (lihat `config.xml` di root repo buat sample asli):
//! ```xml
//! <key name="Sessions">
//!     <key name="ROUTER">
//!         <key name="BAROKAH">
//!             <key name="FRR-BAROKAH-FWD">
//!                 <dword name="Is Session">1</dword>
//!                 <string name="Protocol Name">SSH2</string>
//!                 <string name="Hostname">103.86.117.203</string>
//!                 <string name="Username">bro-noc</string>
//!                 <dword name="[SSH2] Port">14922</dword>
//!                 ...
//!             </key>
//!         </key>
//!     </key>
//! </key>
//! ```
//! Folder "ROUTER" > "BAROKAH" jadi `group_path: ["ROUTER", "BAROKAH"]`
//! — caller (lihat `crates/app/src/state.rs`) yang memutuskan cara
//! memetakan path bertingkat itu ke satu grup flat (join jadi satu
//! nama, mis. "ROUTER / BAROKAH"), karena model grup kita saat ini
//! sengaja tidak bertingkat.
//!
//! **Password TIDAK PERNAH diimpor.** `Password V2` di file SecureCRT
//! dienkripsi pakai skema proprietary VanDyke sendiri (terikat ke
//! passphrase config SecureCRT-nya, bukan sesuatu yang bisa/boleh kita
//! coba bongkar) — host hasil import selalu tanpa password tersimpan,
//! user WAJIB isi manual lewat panel Host Details sebelum bisa connect.

use roxmltree::{Document, Node};

/// Satu host hasil parse, sebelum diubah jadi `HostProfile` beneran
/// (itu tanggung jawab caller — perlu generate `credential_id`, resolve
/// `group_path` jadi `group_id` lewat vault, dsb, yang butuh I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedHost {
    pub label: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    /// Path folder SecureCRT dari root `Sessions` sampai sebelum nama
    /// sesi ini sendiri. Kosong = sesi ada langsung di root (bakal
    /// jadi host ungrouped).
    pub group_path: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ParsedImport {
    pub hosts: Vec<ImportedHost>,
    /// Jumlah entry sesi yang ketemu tapi DILEWATI — protokolnya bukan
    /// SSH2 (mis. "Serial", "Local Shell") atau field wajib (Hostname)
    /// kosong. Dilaporkan ke user biar tahu totalnya tidak "hilang
    /// diam-diam".
    pub skipped: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("XML tidak valid: {0}")]
    InvalidXml(#[from] roxmltree::Error),
    #[error("bukan file config SecureCRT yang dikenali — elemen <key name=\"Sessions\"> tidak ditemukan")]
    NoSessionsKey,
}

/// Parse isi `config.xml` (SecureCRT) jadi daftar host. Murni fungsi
/// `&str -> data` — TIDAK baca file sendiri (itu tanggung jawab
/// caller), biar gampang dites tanpa filesystem.
pub fn parse(xml: &str) -> Result<ParsedImport, ImportError> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();

    let sessions_key = find_child_key(root, "Sessions").ok_or(ImportError::NoSessionsKey)?;

    let mut result = ParsedImport::default();
    let mut path: Vec<String> = Vec::new();
    walk(sessions_key, &mut path, &mut result);
    Ok(result)
}

fn walk(node: Node, path: &mut Vec<String>, out: &mut ParsedImport) {
    for child in child_keys(node) {
        let Some(name) = child.attribute("name") else { continue };

        if is_session_node(child) {
            match extract_host(child, name, path) {
                Some(host) => out.hosts.push(host),
                None => out.skipped += 1,
            }
        } else {
            // Folder — turun satu level, telusuri isinya, naik lagi.
            path.push(name.to_string());
            walk(child, path, out);
            path.pop();
        }
    }
}

fn is_session_node(node: Node) -> bool {
    node.children().any(|n| {
        n.is_element()
            && n.tag_name().name() == "dword"
            && n.attribute("name") == Some("Is Session")
            && n.text() == Some("1")
    })
}

fn extract_host(node: Node, label: &str, path: &[String]) -> Option<ImportedHost> {
    let protocol = find_string(node, "Protocol Name")?;
    if protocol != "SSH2" {
        return None; // Serial/Local Shell/Telnet dsb — di luar scope v1, lihat doc komentar modul.
    }
    let host = find_string(node, "Hostname").filter(|s| !s.is_empty())?;
    let username = find_string(node, "Username").unwrap_or_default();
    // Nama key port SecureCRT punya prefix protokol, mis. "[SSH2] Port".
    let port = find_dword(node, "[SSH2] Port").and_then(|p| u16::try_from(p).ok()).unwrap_or(22);

    Some(ImportedHost { label: label.to_string(), host, port, username, group_path: path.to_vec() })
}

fn child_keys<'a, 'input>(node: Node<'a, 'input>) -> impl Iterator<Item = Node<'a, 'input>> {
    node.children().filter(|n| n.is_element() && n.tag_name().name() == "key")
}

fn find_child_key<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    child_keys(node).find(|n| n.attribute("name") == Some(name))
}

fn find_string(node: Node, key: &str) -> Option<String> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == "string" && n.attribute("name") == Some(key))
        .and_then(|n| n.text())
        .map(|s| s.to_string())
}

fn find_dword(node: Node, key: &str) -> Option<i64> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == "dword" && n.attribute("name") == Some(key))
        .and_then(|n| n.text())
        .and_then(|s| s.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sample kecil yang meniru struktur ASLI `config.xml` SecureCRT
    /// (root `<VanDyke>` + banyak `<key>` preferensi yang harus
    /// DIABAIKAN, `Sessions` dengan folder bertingkat, satu sesi
    /// non-SSH2 yang harus DILEWATI, satu sesi tanpa Hostname yang
    /// juga harus dilewati).
    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
    <VanDyke version="3.0">
        <key name="Global">
            <dword name="Some Preference">1</dword>
        </key>
        <key name="Sessions">
            <key name="ROUTER">
                <key name="BAROKAH">
                    <key name="FRR-BAROKAH-FWD">
                        <dword name="Is Session">1</dword>
                        <string name="Protocol Name">SSH2</string>
                        <string name="Hostname">103.86.117.203</string>
                        <string name="Username">bro-noc</string>
                        <dword name="[SSH2] Port">14922</dword>
                    </key>
                </key>
            </key>
            <key name="prod-web-01">
                <dword name="Is Session">1</dword>
                <string name="Protocol Name">SSH2</string>
                <string name="Hostname">10.0.0.5</string>
                <string name="Username">deploy</string>
                <dword name="[SSH2] Port">22</dword>
            </key>
            <key name="serial-switch">
                <dword name="Is Session">1</dword>
                <string name="Protocol Name">Serial</string>
                <string name="Hostname"/>
            </key>
            <key name="EmptyFolder"/>
        </key>
    </VanDyke>"#;

    #[test]
    fn parse_host_di_root_dan_di_folder_bertingkat() {
        let result = parse(SAMPLE).unwrap();
        assert_eq!(result.hosts.len(), 2, "cuma 2 sesi SSH2 valid, sisanya dilewati");
        assert_eq!(result.skipped, 1, "sesi Serial harus kehitung skipped");

        let nested = result.hosts.iter().find(|h| h.label == "FRR-BAROKAH-FWD").unwrap();
        assert_eq!(nested.host, "103.86.117.203");
        assert_eq!(nested.username, "bro-noc");
        assert_eq!(nested.port, 14922);
        assert_eq!(nested.group_path, vec!["ROUTER".to_string(), "BAROKAH".to_string()]);

        let root_level = result.hosts.iter().find(|h| h.label == "prod-web-01").unwrap();
        assert_eq!(root_level.host, "10.0.0.5");
        assert!(root_level.group_path.is_empty(), "sesi di root Sessions harus tanpa group_path");
    }

    #[test]
    fn xml_tanpa_sessions_key_ditolak() {
        let err = parse("<VanDyke version=\"3.0\"><key name=\"Global\"/></VanDyke>").unwrap_err();
        assert!(matches!(err, ImportError::NoSessionsKey));
    }

    #[test]
    fn xml_rusak_ditolak_bukan_panik() {
        let err = parse("<not-even-xml").unwrap_err();
        assert!(matches!(err, ImportError::InvalidXml(_)));
    }
}

