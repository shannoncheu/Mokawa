#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("The session expired. Sign in again.")]
    Unauthorized,
    #[error("PikPak requires verification. Complete verification with PikPak and sign in again.")]
    VerificationRequired,
    #[error("PikPak is rate limiting requests. Retry later.")]
    RateLimited,
    #[error("PikPak request failed (HTTP {0}).")]
    Http(u16),
    #[error("PikPak rejected this operation.")]
    Api,
    #[error("Network request failed. Check connectivity and retry.")]
    Network,
    #[error("PikPak returned an incompatible response.")]
    Protocol,
    #[error("Invalid input: {0}")]
    Invalid(&'static str),
    #[error("Operation cancelled.")]
    Cancelled,
    #[error("Confirmation is required.")]
    ConfirmationRequired,
    #[error("System credential storage is unavailable.")]
    Credentials,
    #[error(
        "A Mokawa-authorized PikPak client registration is required; no third-party client secret is bundled."
    )]
    ClientRegistrationRequired,
    #[error("Local file operation failed.")]
    Io,
}
pub type Result<T> = std::result::Result<T, Error>;
impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}
impl From<reqwest::Error> for Error {
    fn from(_: reqwest::Error) -> Self {
        Self::Network
    }
}
