use std::time::Duration;

use anyhow::Result;
use futures::Future;
use reqwest::Client;
use tokio::signal::unix::signal;
use tokio::signal::unix::SignalKind;

/// Blocks until SIGINT/SIGTERM is received from OS or provided future completes.
pub async fn os_signal_or_completion_of(future: impl Future<Output = Result<()>>) -> Result<()> {
    let mut sig_int = signal(SignalKind::interrupt())?;
    let mut sig_term = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = sig_int.recv() => { Ok(()) }
        _ = sig_term.recv() => { Ok(()) }
        result = future => { result }
    }
}

pub fn get_reqwest_client() -> Result<Client> {
    Ok(
        reqwest::ClientBuilder::new()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(15))
            .https_only(true)
            .use_rustls_tls()
            .build()?
        )
}