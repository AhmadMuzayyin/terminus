//! `terminus-serial-engine`
//!
//! Koneksi SERIAL/console (ala `minicom` di Linux, atau PuTTY mode
//! "Serial" di Windows) — BEDA total dari SSH: bukan lewat jaringan,
//! tapi lewat kabel fisik RS-232/USB-to-serial yang biasanya nyolok ke
//! port console perangkat Cisco/network gear. Dipakai `PageConsole`
//! (`ui/pages/page-console.slint`), TIDAK ada hubungannya dengan tab
//! terminal SSH (`terminus-ssh-engine`) atau `terminus-cisco-driver`
//! (yang itu Cisco-lewat-SSH, kasus terpisah).
//!
//! SENGAJA TIDAK cuma dukung Linux (`/dev/ttyUSB0`) — aplikasi ini
//! multi-platform, jadi dipakai `tokio-serial` (wrapper async Tokio di
//! atas crate `serialport`, yang PORTABLE: Windows ("COM3", "COM4",
//! dst), Linux (`/dev/ttyUSB0`, `/dev/ttyACM0`), macOS
//! (`/dev/cu.usbserial-*`) — `list_ports()` di bawah otomatis
//! mengembalikan nama yang BENAR buat OS yang lagi jalan, UI tidak
//! perlu tahu bedanya sama sekali.

use thiserror::Error;
use tokio::sync::broadcast;
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, StopBits};

/// Kapasitas buffer broadcast output — sama pertimbangan dengan
/// `terminus-ssh-engine::SshOutputEvent` punya (lihat komentar di
/// sana): cukup besar biar output deras tidak langsung ke-drop kalau
/// subscriber (task reader di `crates/app`) sempat telat konsumsi.
const OUTPUT_BUFFER_CAPACITY: usize = 1024;

#[derive(Debug, Error)]
pub enum SerialError {
    #[error("gagal membaca daftar port serial: {0}")]
    ListPorts(String),

    #[error("gagal membuka port serial: {0}")]
    Open(String),

    #[error("gagal menulis ke port serial: {0}")]
    Write(String),
}

/// Satu port serial yang terdeteksi di sistem — dipakai isi dropdown
/// "Port" di `PageConsole` (chooser, sebelum connect). `label` sudah
/// jadi teks siap-tampil (nama port + info USB kalau ada), `path`
/// yang BENERAN dikirim ke `SerialSession::connect`.
#[derive(Debug, Clone)]
pub struct SerialPortEntry {
    pub path: String,
    pub label: String,
}

/// Scan port serial yang lagi tersambung ke sistem SEKARANG (dipanggil
/// ulang tiap buka halaman Console / klik "⟳ Refresh" — daftar bisa
/// berubah kapan saja, USB-to-serial adapter dicabut-colok). Nama
/// port-nya OTOMATIS beda tergantung OS (lihat komentar modul di
/// atas), tapi caller (UI/state.rs) tidak perlu peduli — cukup pakai
/// `path` apa adanya.
///
/// HANYA balikin port ber-tipe `UsbPort` (adaptor USB-to-serial
/// beneran, kabel console fisik ke Cisco/network gear pada dasarnya
/// SELALU lewat sini) — port lain (`PciPort`/`Unknown`, mis. COM
/// bawaan motherboard, atau di Linux `/dev/ttyS0`..`/dev/ttyS31`, 32
/// port legacy 8250/16550 yang SELALU muncul di daftar OS meski
/// TIDAK ada apa-apa yang nyambung ke situ) DISARING HABIS — daftarnya
/// tadinya penuh "sampah" port yang jelas bukan kabel console beneran.
///
/// PENTING (batasan jujur): ini BUKAN deteksi "port X lagi aktif
/// dipakai buat sesi console" — tidak ada cara pasif buat tahu itu
/// tanpa NGIRIM byte ke device-nya (invasif/beresiko ganggu device
/// yang sebenarnya lagi dipakai orang lain buat hal lain). Filter ini
/// cuma proxy PRAKTIS: "apakah ini adaptor USB-to-serial fisik yang
/// SECARA WAJAR bisa jadi kabel console" — bukan konfirmasi ada
/// perangkat network beneran di ujung satunya.
pub fn list_ports() -> Result<Vec<SerialPortEntry>, SerialError> {
    let ports = tokio_serial::available_ports().map_err(|e| SerialError::ListPorts(e.to_string()))?;
    Ok(ports
        .into_iter()
        .filter_map(|p| {
            let tokio_serial::SerialPortType::UsbPort(usb) = &p.port_type else {
                return None;
            };
            let name = usb.product.clone().or_else(|| usb.manufacturer.clone());
            let extra = match name {
                Some(n) => format!(" — {n}"),
                None => String::new(),
            };
            Some(SerialPortEntry { label: format!("{}{}", p.port_name, extra), path: p.port_name })
        })
        .collect())
}

/// Satu event dari port serial — pola SAMA persis dengan
/// `terminus_ssh_engine::SshOutputEvent` (lihat komentar di sana
/// soal kenapa `Closed` dipisah dari byte data mentah): `Closed`
/// dikirim begitu task pembaca berhenti (device dicabut, atau port
/// ditutup lewat `SerialSession::disconnect`), supaya UI tahu kapan
/// harus balik ke status "disconnected" tanpa perlu polling.
#[derive(Debug, Clone)]
pub enum SerialOutputEvent {
    Data(Vec<u8>),
    Closed,
}

/// Satu sesi console yang lagi terbuka ke satu port. Beda dari
/// `SshSession` (yang bisa banyak sekaligus, satu per tab terminal),
/// Console v1 SENGAJA cuma satu sesi aktif dalam satu waktu — port
/// serial fisik memang EKSKLUSIF (tidak bisa dua proses buka port yang
/// sama bersamaan), jadi tidak ada gunanya pura-pura multi-sesi di
/// level UI.
pub struct SerialSession {
    write_half: tokio::io::WriteHalf<tokio_serial::SerialStream>,
    output_tx: broadcast::Sender<SerialOutputEvent>,
    // Task yang megang `read_half` (lihat `connect()` di bawah) —
    // DIPEGANG DI SINI (bukan cuma "lepas jalan sendiri") khusus supaya
    // `disconnect()` bisa `.abort()` dia secara eksplisit. Lihat
    // komentar panjang di `disconnect()` soal KENAPA ini wajib.
    read_task: tokio::task::JoinHandle<()>,
}

impl SerialSession {
    /// Buka port serial dan mulai baca di background. `baud_rate`
    /// dipilih user (default 9600 — standar kabel console Cisco),
    /// data/parity/stop bits DIKUNCI 8-N-1 (settingan console RS-232
    /// yang dipakai hampir semua perangkat network — v1 sengaja tidak
    /// expose kontrol itu di UI, cukup baud rate saja).
    ///
    /// Balikin RECEIVER (bukan cuma `Self`, beda dari `SshSession` yang
    /// punya `subscribe_output()` terpisah dipanggil BELAKANGAN oleh
    /// caller) — SENGAJA, biar subscribe ke `output_tx` PASTI terjadi
    /// SEBELUM task pembaca di-`spawn` (satu fungsi ini, tanpa ada
    /// `.await` lain di antaranya), bukan `race` dua langkah terpisah.
    /// Kalau subscribe menyusul BELAKANGAN (setelah `connect()` return
    /// ke caller, caller baru manggil `subscribe_output()`), byte yang
    /// device kirim SUPER CEPAT (banner/prompt awal, sering langsung
    /// nongol begitu port kebuka — beda dari SSH yang punya jeda
    /// network round-trip alami) bisa keburu ke-`send()` ke
    /// `broadcast::channel` SEBELUM ada subscriber sama sekali —
    /// `tokio::sync::broadcast` TIDAK menyimpan histori buat subscriber
    /// yang baru gabung belakangan, jadi byte itu PERMANEN HILANG.
    /// Inilah akar masalah laporan user: "teks terminal tidak tampil,
    /// seperti hilang dulu, baru muncul setelah beberapa Enter" —
    /// bukan device-nya yang diam, tapi banner/prompt awalnya kelewat
    /// sebelum kita mulai "dengar".
    pub async fn connect(path: &str, baud_rate: u32) -> Result<(Self, broadcast::Receiver<SerialOutputEvent>), SerialError> {
        let builder = tokio_serial::new(path, baud_rate)
            .data_bits(DataBits::Eight)
            .parity(Parity::None)
            .stop_bits(StopBits::One)
            .flow_control(FlowControl::None);
        let stream = builder.open_native_async().map_err(|e| SerialError::Open(e.to_string()))?;

        let (mut read_half, write_half) = tokio::io::split(stream);
        // `broadcast::channel()` balikin Sender+Receiver SEKALIGUS,
        // satu panggilan atomik — receiver ini (`initial_rx`) DIJAMIN
        // "sudah dengar" SEBELUM baris `tokio::spawn` di bawah mulai
        // jalan, tidak ada celah waktu apa pun di antaranya.
        let (output_tx, initial_rx) = broadcast::channel(OUTPUT_BUFFER_CAPACITY);
        let tx_for_task = output_tx.clone();

        let read_task = tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = [0u8; 4096];
            loop {
                match read_half.read(&mut buf).await {
                    Ok(0) => break, // EOF — device dicabut/port ketutup dari sisi lain
                    Ok(n) => {
                        // Abaikan error kirim — artinya belum ada
                        // subscriber, bukan kegagalan port itu sendiri.
                        let _ = tx_for_task.send(SerialOutputEvent::Data(buf[..n].to_vec()));
                    }
                    Err(_) => break,
                }
            }
            let _ = tx_for_task.send(SerialOutputEvent::Closed);
        });

        Ok((Self { write_half, output_tx, read_task }, initial_rx))
    }

    /// Kirim byte ke device (mis. input keyboard user).
    pub async fn write(&mut self, data: &[u8]) -> Result<(), SerialError> {
        use tokio::io::AsyncWriteExt;
        self.write_half.write_all(data).await.map_err(|e| SerialError::Write(e.to_string()))
    }

    /// Berlangganan TAMBAHAN ke output mentah device (subscriber
    /// PERTAMA/dijamin-tidak-ketinggalan sudah didapat langsung dari
    /// `connect()`, lihat komentar di sana) — dipakai kalau suatu saat
    /// butuh lebih dari satu pendengar sekaligus.
    pub fn subscribe_output(&self) -> broadcast::Receiver<SerialOutputEvent> {
        self.output_tx.subscribe()
    }

    /// Tutup port secara SUNGGUHAN (klik "Disconnect" di UI, ATAU
    /// sebelum sesi lama di-drop waktu connect ulang) — permintaan
    /// eksplisit user: disconnect harus BENERAN memutus, bukan cuma
    /// kosmetik, supaya connect ulang WAJIB login lagi ke device
    /// (bukan nyambung ke sesi CLI yang ternyata masih nyangkut).
    ///
    /// `read_task.abort()` WAJIB ada di sini — tanpa ini, task
    /// pembaca (lihat `connect()`) masih pegang `read_half` dan TERUS
    /// nunggu `read()` selamanya (serial port FISIK, beda dari socket
    /// TCP: shutdown separuh WRITE-nya saja TIDAK bikin sisi READ
    /// dapat EOF/error — tidak ada konsep "half-close" di situ). Kalau
    /// `read_half` tidak ikut dilepas, file descriptor port TIDAK
    /// PERNAH benar-benar tertutup di level OS meski `write_half`
    /// sudah di-shutdown — device tidak pernah "melihat" ini sebagai
    /// disconnect fisik beneran (baris DTR/RTS tidak toggle), sesi CLI
    /// yang lagi login di device itu TETAP nyangkut jalan terus. Abis
    /// `.abort()`, `read_task` (makanya BUKAN cuma dibiarkan lepas
    /// jalan sendiri tanpa dipegang) di-drop begitu `SerialSession`
    /// sendiri di-drop — itu yang beneran ngelepas `read_half`.
    pub async fn disconnect(&mut self) {
        self.read_task.abort();
        use tokio::io::AsyncWriteExt;
        let _ = self.write_half.shutdown().await;
    }
}

impl Drop for SerialSession {
    // Jaring pengaman TAMBAHAN di luar `disconnect()` eksplisit — kalau
    // `SerialSession` di-drop lewat jalur MANAPUN (mis. di-replace
    // waktu connect ulang tanpa sempat panggil `disconnect().await`
    // dulu, atau error path lain nanti) `read_task` TETAP wajib
    // berhenti di sini juga, bukan cuma waktu `disconnect()` dipanggil
    // eksplisit — jaga-jaga kebocoran port yang sama persis dilaporkan
    // user (device tidak pernah "melihat" disconnect fisik beneran).
    fn drop(&mut self) {
        self.read_task.abort();
    }
}
