use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("profil dengan id {0} tidak ditemukan")]
    ProfileNotFound(String),

    #[error("grup dengan id {0} tidak ditemukan")]
    GroupNotFound(String),

    #[error("data tidak valid: {0}")]
    Validation(String),
}
