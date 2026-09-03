//! Representasi satu sesi SSH yang sudah terkoneksi.
//!
//! `SshSession` membungkus channel shell russh dan expose sebagai
//! stream byte async (read/write) yang nanti dikonsumsi oleh
//! `terminus-term-emulator` untuk di-parse jadi grid karakter terminal
//! (Tahap 2/3 — belum dipakai UI di putaran ini, tapi API-nya sudah
//! siap: `subscribe_output()`).

use crate::client::ClientHandler;
use crate::SshEngineError;
use russh::client::{Handle, Msg};
use russh::{Channel, ChannelMsg};
use tokio::sync::broadcast;

/// Kapasitas buffer broadcast channel output — cukup besar supaya
/// output deras (mis. `cat` file besar) tidak langsung ke-drop kalau
/// subscriber (term-emulator) sempat telat konsumsi.
const OUTPUT_BUFFER_CAPACITY: usize = 1024;

/// Satu event dari channel shell. Dipisah dari byte mentah (bukan cuma
/// `Vec<u8>`) supaya subscriber (mis. task terminal reader di
/// `crates/app`) bisa BEDAKAN "belum ada data baru" dari "sesi ini
/// SUDAH BERAKHIR" (remote kirim Eof/Close, atau kita sendiri yang
/// nutup) — tanpa ini, UI tidak akan pernah tahu kapan status host
/// harus balik ke "offline" waktu user ngetik `exit` di shell remote.
#[derive(Debug, Clone)]
pub enum SshOutputEvent {
    Data(Vec<u8>),
    Closed,
}

pub struct SshSession {
    /// Handle koneksi russh. Harus tetap hidup selama sesi berlangsung
    /// — kalau ini di-drop, socket ikut ketutup.
    _handle: Handle<ClientHandler>,
    write_half: russh::ChannelWriteHalf<Msg>,
    output_tx: broadcast::Sender<SshOutputEvent>,
}

impl SshSession {
    /// Dipanggil dari `connect()` di `lib.rs` setelah channel shell
    /// berhasil dibuka. Split channel jadi read/write half: write half
    /// disimpan langsung di struct, read half di-loop di task
    /// background yang forward byte ke `output_tx`.
    pub(crate) fn new(handle: Handle<ClientHandler>, channel: Channel<Msg>) -> Self {
        let (mut read_half, write_half) = channel.split();
        let (output_tx, _) = broadcast::channel(OUTPUT_BUFFER_CAPACITY);
        let tx_for_task = output_tx.clone();

        tokio::spawn(async move {
            loop {
                match read_half.wait().await {
                    Some(ChannelMsg::Data { data }) => {
                        // Abaikan error kirim — artinya belum ada
                        // subscriber (term-emulator belum wired), bukan
                        // kegagalan koneksi SSH itu sendiri.
                        let _ = tx_for_task.send(SshOutputEvent::Data(data.to_vec()));
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        let _ = tx_for_task.send(SshOutputEvent::Data(data.to_vec()));
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                    _ => {}
                }
            }
            // Sesi beneran berakhir (remote nutup channel, ATAU koneksi
            // putus) — beri tahu subscriber (mis. terminal reader task)
            // supaya status host bisa balik ke "offline" tanpa nunggu
            // user klik Disconnect manual.
            let _ = tx_for_task.send(SshOutputEvent::Closed);
        });

        Self { _handle: handle, write_half, output_tx }
    }

    /// Kirim byte ke stdin sesi (mis. input keyboard user).
    pub async fn write(&self, data: &[u8]) -> Result<(), SshEngineError> {
        self.write_half.data(data).await.map_err(|e| SshEngineError::Channel(e.to_string()))
    }

    /// Berlangganan output mentah sesi (stdout+stderr digabung, sesuai
    /// urutan diterima) DAN event "sesi berakhir". Bisa dipanggil
    /// berkali-kali — tiap subscriber dapat salinan independen lewat
    /// `tokio::sync::broadcast`.
    pub fn subscribe_output(&self) -> broadcast::Receiver<SshOutputEvent> {
        self.output_tx.subscribe()
    }

    /// Beri tahu server ukuran terminal berubah (dipanggil waktu window
    /// di-resize). `pix_width`/`pix_height` 0 = server pakai default.
    pub async fn resize_pty(&self, cols: u16, rows: u16) -> Result<(), SshEngineError> {
        self.write_half
            .window_change(cols as u32, rows as u32, 0, 0)
            .await
            .map_err(|e| SshEngineError::Channel(e.to_string()))
    }

    pub async fn disconnect(&mut self) -> Result<(), SshEngineError> {
        let _ = self.write_half.eof().await;
        let _ = self.write_half.close().await;
        let _ = self._handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
        Ok(())
    }
}
