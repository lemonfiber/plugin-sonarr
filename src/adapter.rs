//! What this adapter says it is, and the kit that serves its contract.

use std::path::PathBuf;
use std::sync::Arc;

use adapter_kit_servarr::client::Servarr;
use adapter_kit_servarr::curator::{Curator, Files, NoMusic};
use lemonfiber_adapter_kit::credential::Holding;
use lemonfiber_adapter_kit::{Kit, Readiness};
use lemonfiber_contract::adapter::{About, Release};
use lemonfiber_contract::capabilities::{indexer, library};
use lemonfiber_ports::http::Http;

/// Where Sonarr answers, by its service id and the port it listens on.
pub const ADDRESS: &str = "http://sonarr:8989";

/// The upstream this adapter fronts, as its project names itself.
pub const UPSTREAM: &str = "Sonarr";

include!(concat!(env!("OUT_DIR"), "/fronted.rs"));

/// What every failure is blamed on.
pub const SERVICE: &str = "sonarr";

/// The version of Sonarr's API this adapter speaks.
pub const API_VERSION: u32 = 3;

/// What this adapter says it is: the contracts it speaks and the release it supports.
#[must_use]
pub fn about() -> About {
    About {
        speaks: vec![
            lemonfiber_contract::spoken(indexer::pull::CAPABILITY, indexer::pull::MAJOR),
            lemonfiber_contract::spoken(library::curate::CAPABILITY, library::curate::MAJOR),
        ],
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

/// The contracts, served from `sonarr` under `key` and to the request gate under each key
/// the core wrote for it beside `configuration`, ready while Sonarr answers.
#[must_use]
pub fn kit(key: String, configuration: impl Into<PathBuf>, sonarr: Arc<Curator>) -> Kit {
    let asked = Arc::clone(&sonarr);
    let ready: Readiness = Arc::new(move || {
        let asked = Arc::clone(&asked);
        Box::pin(async move { asked.servarr().answers().await })
    });
    Kit::new(key, about())
        .asked_by_the_gate(configuration)
        .ready_when(ready)
        .serving(indexer::pull::served(Arc::clone(&sonarr)))
        .serving(library::curate::served(sonarr))
}

#[cfg(test)]
mod tests;
