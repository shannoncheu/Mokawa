use mokawa::{credentials, error::Result};
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Mokawa: {error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<()> {
    if !cfg!(windows) {
        return Err(mokawa::error::Error::Credentials);
    }
    if std::env::args().any(|a| a == "--logout") {
        credentials::delete()?;
        println!("Mokawa: local session removed.");
        return Ok(());
    }
    // Official PikPak CLI documentation now states that password login is unsupported.
    // Preserve the legacy protocol implementation for Mock research, but do not solicit
    // user passwords before a documented Mokawa desktop OAuth contract is available.
    Err(mokawa::error::Error::AuthorizationPending)
}
