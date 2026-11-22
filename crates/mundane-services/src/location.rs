//! One portal-approved location fix. Both applications request it only on activation.
use anyhow::{Context, Result, bail, ensure};
use ashpd::desktop::location::{Accuracy, CreateSessionOptions, LocationProxy};
use futures_lite::{StreamExt, future};
use std::time::Duration;

pub async fn locate(city_accuracy: bool) -> Result<(f64, f64)> {
    let proxy = LocationProxy::new()
        .await
        .context("Location portal unavailable")?;
    let accuracy = if city_accuracy {
        Accuracy::City
    } else {
        Accuracy::Street
    };
    let session = proxy
        .create_session(CreateSessionOptions::default().set_accuracy(accuracy))
        .await?;
    let result = async {
        let mut updates = proxy.receive_location_updated().await?;
        // Session serializes to its D-Bus object path. Other clients' updates
        // or a late fix from another session must never move this view.
        let handle = serde_json::to_value(&session)?;
        proxy
            .start(&session, None, Default::default())
            .await?
            .response()?;
        future::race(
            async {
                while let Some(fix) = updates.next().await {
                    if Some(fix.session_handle().as_str()) == handle.as_str() {
                        let (latitude, longitude) = (fix.latitude(), fix.longitude());
                        ensure!(
                            (-90.0..=90.0).contains(&latitude)
                                && (-180.0..=180.0).contains(&longitude),
                            "The location service returned invalid coordinates."
                        );
                        return Ok((latitude, longitude));
                    }
                }
                bail!("Location session ended before a fix.")
            },
            async {
                async_io::Timer::after(Duration::from_secs(30)).await;
                bail!("The location service did not return a fix.")
            },
        )
        .await
    }
    .await;
    session.close().await?;
    result
}
