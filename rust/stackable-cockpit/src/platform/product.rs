use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductSpec {
    #[serde(rename = "operatorVersion")]
    pub version: Version,
}
