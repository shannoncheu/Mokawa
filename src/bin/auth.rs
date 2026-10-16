use mokawa::{api::Api, auth::Registration, credentials, error::Result};
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
    let registration = Registration::from_env()?;
    println!("Mokawa — PikPak sign in. Credentials go directly to PikPak.");
    let username = rpassword::prompt_password("PikPak account (hidden): ")?;
    let password = rpassword::prompt_password("Password (hidden): ")?;
    let device_id =
        rpassword::prompt_password("Stable device ID for this authorized registration: ")?;
    let session = Api::new()?
        .login(&registration, &username, &password.into(), &device_id)
        .await?;
    credentials::save(&session)?;
    println!("Mokawa: session saved in Windows Credential Manager.");
    Ok(())
}
