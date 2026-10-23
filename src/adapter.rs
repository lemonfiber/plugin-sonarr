//! What this adapter says it is, and the kit that serves its contract.

use std::sync::Arc;

use adapter_kit_servarr::client::Servarr;
use adapter_kit_servarr::credential::Holding;
use adapter_kit_servarr::curator::{Curator, Files, NoMusic};
use lemonfiber_adapter_kit::{Kit, Readiness};
use lemonfiber_contract::adapter::{About, Release};
use lemonfiber_contract::capabilities::library;
use lemonfiber_ports::http::Http;

/// The upstream this adapter fronts, as its project names itself.
pub const UPSTREAM: &str = "Sonarr";

/// The Sonarr release this adapter's recordings were taken from.
pub const RELEASE: &str = "4.0.20.3014-ls327";

/// The digest of the image that release's recordings were taken from.
pub const DIGEST: &str = "sha256:dffc730adcb8b4f4342792fbb27fcad9c62fb8660523f2aeb082416980d7fe0c";

/// What every failure is blamed on.
pub const SERVICE: &str = "sonarr";

/// The version of Sonarr's API this adapter speaks.
pub const API_VERSION: u32 = 3;

/// What this adapter says it is: the contract it speaks and the release it supports.
#[must_use]
pub fn about() -> About {
    About {
        speaks: vec![lemonfiber_contract::spoken(
            library::curate::CAPABILITY,
            library::curate::MAJOR,
        )],
        upstream: UPSTREAM.to_owned(),
        releases: vec![Release {
            version: RELEASE.to_owned(),
            digest: DIGEST.to_owned(),
        }],
    }
}

/// The Sonarr reached at `base` over `http`, curating television with the key `held`
/// supplies.
#[must_use]
pub fn sonarr(http: Arc<dyn Http>, base: impl Into<String>, held: Arc<dyn Holding>) -> Curator {
    Curator::new(
        Servarr::new(http, base, held, SERVICE, API_VERSION),
        Files::TELEVISION,
        NoMusic,
    )
}

/// The contract, served from `sonarr` under `key`, ready while Sonarr answers.
#[must_use]
pub fn kit(key: String, sonarr: Arc<Curator>) -> Kit {
    let asked = Arc::clone(&sonarr);
    let ready: Readiness = Arc::new(move || {
        let asked = Arc::clone(&asked);
        Box::pin(async move { asked.servarr().answers().await })
    });
    Kit::new(key, about())
        .ready_when(ready)
        .serving(library::curate::served(sonarr))
}

#[cfg(test)]
mod tests;
