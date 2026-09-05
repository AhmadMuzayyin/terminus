//! Generator `config.xml` ala VanDyke SecureCRT dari host tersimpan —
//! kebalikan dari `crate::import::securecrt::parse`. Skema XML SAMA
//! (root `<VanDyke>`, subtree `<key name="Sessions">` isinya `<key>`
//! bertingkat per folder, satu sesi ditandai `<dword name="Is
//! Session">1</dword>`) supaya hasilnya bisa dibuka SecureCRT ASLI atau
//! diimpor balik lewat `crate::import::securecrt::parse` kita sendiri
//! (dibuktikan round-trip di test module ini).
//!
//! **Password TIDAK PERNAH ditulis ke file ini** — sama seperti Import,
//! yang di-export murni metadata koneksi (label/host/port/username),
//! bukan file backup kredensial. Kebutuhan backup lengkap TERMASUK
//! password ada di rencana terpisah ("Export JSON dengan password
//! terenkripsi") yang sengaja dipisah supaya file plaintext seperti ini
//! tidak pernah kebocoran kredensial.

/// Satu host yang mau di-export. Caller (`crates/app/src/state.rs`)
/// yang bertanggung jawab resolve `group_id` profil (kalau ada) jadi
/// nama grup lalu split by `" / "` — kebalikan persis dari cara
/// `import_parsed_hosts_into_vault` nge-flatten `group_path` jadi satu
/// nama grup waktu import (lihat `crate::import::securecrt`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportHost {
    pub label: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub group_path: Vec<String>,
}

/// Tree folder sementara buat nyusun host per `group_path` sebelum
/// dirender jadi XML bertingkat. `Vec` (bukan `HashMap`) sengaja —
/// jumlah folder per level realistis kecil, dan urutan insersi
/// (bukan alfabetis) yang dipertahankan lebih gampang diprediksi.
#[derive(Default)]
struct FolderNode {
    children: Vec<(String, FolderNode)>,
    hosts: Vec<ExportHost>,
}

impl FolderNode {
    fn insert(&mut self, mut path: std::slice::Iter<'_, String>, host: ExportHost) {
        match path.next() {
            None => self.hosts.push(host),
            Some(name) => {
                let idx = match self.children.iter().position(|(n, _)| n == name) {
                    Some(i) => i,
                    None => {
                        self.children.push((name.clone(), FolderNode::default()));
                        self.children.len() - 1
                    }
                };
                self.children[idx].1.insert(path, host);
            }
        }
    }

    fn render(&self, out: &mut String, indent: usize) {
        let pad = "    ".repeat(indent);
        for (name, node) in &self.children {
            out.push_str(&format!("{pad}<key name=\"{}\">\n", escape(name)));
            node.render(out, indent + 1);
            out.push_str(&format!("{pad}</key>\n"));
        }
        for host in &self.hosts {
            out.push_str(&format!("{pad}<key name=\"{}\">\n", escape(&host.label)));
            out.push_str(&format!("{pad}    <dword name=\"Is Session\">1</dword>\n"));
            out.push_str(&format!("{pad}    <string name=\"Protocol Name\">SSH2</string>\n"));
            out.push_str(&format!("{pad}    <string name=\"Hostname\">{}</string>\n", escape(&host.host)));
            out.push_str(&format!("{pad}    <string name=\"Username\">{}</string>\n", escape(&host.username)));
            out.push_str(&format!("{pad}    <dword name=\"[SSH2] Port\">{}</dword>\n", host.port));
            out.push_str(&format!("{pad}</key>\n"));
        }
    }
}

/// Escape 5 karakter spesial XML minimal (`&`, `<`, `>`, `"`, `'`) —
/// cukup buat text node/attribute value di sini (label/host/username
/// bebas user isi apa saja termasuk karakter itu), TIDAK bermaksud
/// jadi XML writer general-purpose. `&` WAJIB duluan supaya tidak
/// dobel-escape hasil replace lain.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// Render daftar host jadi isi `config.xml` SecureCRT-compatible.
/// Urutan folder/host di output cuma sejauh urutan pertama muncul di
/// `hosts` — caller yang butuh urutan tertentu (mis. alfabetis) sort
/// dulu sebelum manggil ini.
pub fn export(hosts: &[ExportHost]) -> String {
    let mut root = FolderNode::default();
    for host in hosts {
        root.insert(host.group_path.iter(), host.clone());
    }

    let mut sessions = String::new();
    root.render(&mut sessions, 2);

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<VanDyke version=\"3.0\">\n\
    <key name=\"Sessions\">\n\
{sessions}\
    </key>\n\
</VanDyke>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::securecrt::parse;

    #[test]
    fn export_lalu_import_ulang_hasilnya_sama_data() {
        let hosts = vec![
            ExportHost {
                label: "prod-web-01".to_string(),
                host: "10.0.0.5".to_string(),
                port: 22,
                username: "deploy".to_string(),
                group_path: vec![],
            },
            ExportHost {
                label: "FRR-BAROKAH-FWD".to_string(),
                host: "103.86.117.203".to_string(),
                port: 14922,
                username: "bro-noc".to_string(),
                group_path: vec!["ROUTER".to_string(), "BAROKAH".to_string()],
            },
        ];

        let xml = export(&hosts);
        let parsed = parse(&xml).expect("hasil export harus jadi XML valid & punya key Sessions");
        assert_eq!(parsed.hosts.len(), 2);
        assert_eq!(parsed.skipped, 0);

        let nested = parsed.hosts.iter().find(|h| h.label == "FRR-BAROKAH-FWD").unwrap();
        assert_eq!(nested.host, "103.86.117.203");
        assert_eq!(nested.username, "bro-noc");
        assert_eq!(nested.port, 14922);
        assert_eq!(nested.group_path, vec!["ROUTER".to_string(), "BAROKAH".to_string()]);

        let flat = parsed.hosts.iter().find(|h| h.label == "prod-web-01").unwrap();
        assert_eq!(flat.host, "10.0.0.5");
        assert!(flat.group_path.is_empty(), "host tanpa grup harus ada langsung di root Sessions");
    }

    #[test]
    fn karakter_spesial_xml_di_field_ke_escape_dengan_benar() {
        let hosts = vec![ExportHost {
            label: "R&D <lab>".to_string(),
            host: "10.0.0.9".to_string(),
            port: 22,
            username: "a\"b'c".to_string(),
            group_path: vec![],
        }];

        let xml = export(&hosts);
        let parsed = parse(&xml).unwrap();
        assert_eq!(parsed.hosts[0].label, "R&D <lab>");
        assert_eq!(parsed.hosts[0].username, "a\"b'c");
    }

    #[test]
    fn dua_host_grup_sama_tidak_bikin_folder_dobel() {
        let hosts = vec![
            ExportHost {
                label: "a".to_string(),
                host: "10.0.0.1".to_string(),
                port: 22,
                username: "u".to_string(),
                group_path: vec!["ROUTER".to_string()],
            },
            ExportHost {
                label: "b".to_string(),
                host: "10.0.0.2".to_string(),
                port: 22,
                username: "u".to_string(),
                group_path: vec!["ROUTER".to_string()],
            },
        ];

        let xml = export(&hosts);
        assert_eq!(xml.matches("<key name=\"ROUTER\">").count(), 1, "satu folder ROUTER harus dipakai ulang, bukan dobel");
        let parsed = parse(&xml).unwrap();
        assert_eq!(parsed.hosts.len(), 2);
    }
}
