use std::collections::HashMap;
use std::fmt::Display;
use std::str::{self, Utf8Error};

use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use tracing::{debug, error, info, instrument};
use url::Url;

use crate::constants::{HELM_DEFAULT_CHART_VERSION, HELM_REPO_INDEX_FILE};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelmRelease {
    pub name: String,
    pub version: String,
    pub namespace: String,
    pub status: String,
    pub last_updated: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelmChart {
    pub release_name: String,
    pub name: String,
    pub repo: HelmChartRepo,
    pub version: String,
    pub options: serde_yaml::Value,
}

#[derive(Debug, Deserialize)]
pub struct HelmChartRepo {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HelmRepo {
    pub entries: HashMap<String, Vec<HelmRepoEntry>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HelmRepoEntry {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Snafu)]
pub enum HelmError {
    #[snafu(display("failed to convert raw C string pointer to UTF-8 string"))]
    StrUtf8Error { source: Utf8Error },

    #[snafu(display("failed to parse URL"))]
    UrlParseError { source: url::ParseError },

    #[snafu(display("failed to deserialize JSON data"))]
    DeserializeJsonError { source: serde_json::Error },

    #[snafu(display("failed to deserialize YAML data"))]
    DeserializeYamlError { source: serde_yaml::Error },

    #[snafu(display("failed to retrieve remote content"))]
    RequestError { source: reqwest::Error },

    #[snafu(display("failed to add Helm repo ({error})"))]
    AddRepoError { error: String },

    #[snafu(display("failed to list Helm releases: {error}"))]
    ListReleasesError { error: String },

    #[snafu(display("failed to install Helm release"))]
    InstallReleaseError { source: HelmInstallReleaseError },

    #[snafu(display("failed to uninstall Helm release: {error}"))]
    UninstallReleaseError { error: String },
}

#[derive(Debug, Snafu)]
pub enum HelmInstallReleaseError {
    /// This error indicates that the Helm release was not found, instead of
    /// `check_release_exists` returning true.
    #[snafu(display("failed to find release {name}"))]
    NoSuchRelease { name: String },

    /// This error indicates that the Helm release is already installed at a
    /// different version than requested. Installation is skipped. Existing
    /// releases should be uninstalled with 'stackablectl op un \<NAME\>'.
    #[snafu(display("release {name} ({current_version}) already installed, skipping requested version {requested_version}"))]
    ReleaseAlreadyInstalled {
        name: String,
        current_version: String,
        requested_version: String,
    },

    /// This error indicates that there was an Helm error. The error it self
    /// is not typed, as the error is a plain string coming directly from the
    /// FFI bindings.
    #[snafu(display("helm error: {error}"))]
    HelmWrapperError { error: String },
}

#[derive(Debug)]
pub enum HelmInstallReleaseStatus {
    /// Indicates that a release is already installed with a different version
    /// than requested.
    ReleaseAlreadyInstalledWithVersion {
        release_name: String,
        current_version: String,
        requested_version: String,
    },

    /// Indicates that a release is already installed, but no specific version
    /// was requested.
    ReleaseAlreadyInstalledUnspecified {
        release_name: String,
        current_version: String,
    },

    /// Indicates that the release was installed successfully.
    Installed(String),
}

impl Display for HelmInstallReleaseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HelmInstallReleaseStatus::ReleaseAlreadyInstalledWithVersion {
                release_name,
                current_version,
                requested_version,
            } => {
                write!(
                    f,
                    "The release {} ({}) is already installed (requested {}), skipping.",
                    release_name, current_version, requested_version
                )
            }
            HelmInstallReleaseStatus::ReleaseAlreadyInstalledUnspecified {
                release_name,
                current_version,
            } => {
                write!(
                    f,
                    "The release {} ({}) is already installed and no specific version was requested, skipping.",
                    release_name,
                    current_version
                )
            }
            HelmInstallReleaseStatus::Installed(release_name) => {
                write!(
                    f,
                    "The release {} was successfully installed.",
                    release_name
                )
            }
        }
    }
}

#[derive(Debug)]
pub enum HelmUninstallReleaseStatus {
    NotInstalled(String),
    Uninstalled(String),
}

impl Display for HelmUninstallReleaseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HelmUninstallReleaseStatus::NotInstalled(release_name) => {
                write!(
                    f,
                    "The release {} is not installed, skipping.",
                    release_name
                )
            }
            HelmUninstallReleaseStatus::Uninstalled(release_name) => {
                write!(
                    f,
                    "The release {} was successfully uninstalled.",
                    release_name
                )
            }
        }
    }
}

pub struct ChartVersion<'a> {
    pub repo_name: &'a str,
    pub chart_name: &'a str,
    pub chart_version: Option<&'a str>,
}

/// Installs a Helm release
#[instrument]
pub fn install_release_from_repo(
    operator_name: &str,
    release_name: &str,
    ChartVersion {
        repo_name,
        chart_name,
        chart_version,
    }: ChartVersion,
    values_yaml: Option<&str>,
    namespace: &str,
    suppress_output: bool,
) -> Result<HelmInstallReleaseStatus, HelmError> {
    debug!("Install Helm release from repo");

    if check_release_exists(release_name, namespace)? {
        let release =
            get_release(release_name, namespace)?.ok_or(HelmError::InstallReleaseError {
                source: HelmInstallReleaseError::NoSuchRelease {
                    name: release_name.to_owned(),
                },
            })?;

        let current_version = release.version;

        match chart_version {
            Some(chart_version) => {
                if chart_version == current_version {
                    return Ok(
                        HelmInstallReleaseStatus::ReleaseAlreadyInstalledWithVersion {
                            requested_version: chart_version.to_string(),
                            release_name: release_name.to_string(),
                            current_version,
                        },
                    );
                } else {
                    return Err(HelmError::InstallReleaseError {
                        source: HelmInstallReleaseError::ReleaseAlreadyInstalled {
                            requested_version: chart_version.into(),
                            name: release_name.into(),
                            current_version,
                        },
                    });
                }
            }
            None => {
                return Ok(
                    HelmInstallReleaseStatus::ReleaseAlreadyInstalledUnspecified {
                        release_name: release_name.to_string(),
                        current_version,
                    },
                )
            }
        }
    }

    let full_chart_name = format!("{repo_name}/{chart_name}");
    let chart_version = chart_version.unwrap_or(HELM_DEFAULT_CHART_VERSION);

    debug!(
        "Installing Helm release {} ({}) from chart {}",
        release_name, chart_version, full_chart_name
    );

    install_release(
        release_name,
        &full_chart_name,
        chart_version,
        values_yaml,
        namespace,
        suppress_output,
    )?;

    Ok(HelmInstallReleaseStatus::Installed(
        release_name.to_string(),
    ))
}

fn install_release(
    release_name: &str,
    chart_name: &str,
    chart_version: &str,
    values_yaml: Option<&str>,
    namespace: &str,
    suppress_output: bool,
) -> Result<(), HelmError> {
    let result = helm_sys::install_helm_release(
        release_name,
        chart_name,
        chart_version,
        values_yaml.unwrap_or(""),
        namespace,
        suppress_output,
    );

    if let Some(err) = helm_sys::to_helm_error(&result) {
        error!(
            "Go wrapper function go_install_helm_release encountered an error: {}",
            err
        );

        return Err(HelmError::InstallReleaseError {
            source: HelmInstallReleaseError::HelmWrapperError { error: err },
        });
    }

    Ok(())
}

/// Uninstall a Helm release
#[instrument]
pub fn uninstall_release(
    release_name: &str,
    namespace: &str,
    suppress_output: bool,
) -> Result<HelmUninstallReleaseStatus, HelmError> {
    debug!("Uninstall Helm release");

    if check_release_exists(release_name, namespace)? {
        let result = helm_sys::uninstall_helm_release(release_name, namespace, suppress_output);

        if let Some(err) = helm_sys::to_helm_error(&result) {
            error!(
                "Go wrapper function go_uninstall_helm_release encountered an error: {}",
                err
            );

            return Err(HelmError::UninstallReleaseError { error: err });
        }

        return Ok(HelmUninstallReleaseStatus::Uninstalled(
            release_name.to_string(),
        ));
    }

    info!(
        "The Helm release {} is not installed, skipping.",
        release_name
    );

    Ok(HelmUninstallReleaseStatus::NotInstalled(
        release_name.to_string(),
    ))
}

/// Returns if a Helm release exists
#[instrument]
pub fn check_release_exists(release_name: &str, namespace: &str) -> Result<bool, HelmError> {
    debug!("Check if Helm release exists");

    // TODO (Techassi): Handle error
    Ok(helm_sys::check_helm_release_exists(release_name, namespace))
}

/// Returns a list of Helm releases
#[instrument]
pub fn list_releases(namespace: &str) -> Result<Vec<HelmRelease>, HelmError> {
    debug!("List Helm releases");

    let result = helm_sys::list_helm_releases(namespace);

    if let Some(err) = helm_sys::to_helm_error(&result) {
        error!(
            "Go wrapper function go_helm_list_releases encountered an error: {}",
            err
        );

        return Err(HelmError::ListReleasesError { error: err });
    }

    serde_json::from_str(&result).context(DeserializeJsonSnafu)
}

/// Returns a single Helm release by `release_name`.
#[instrument]
pub fn get_release(release_name: &str, namespace: &str) -> Result<Option<HelmRelease>, HelmError> {
    debug!("Get Helm release");

    Ok(list_releases(namespace)?
        .into_iter()
        .find(|r| r.name == release_name))
}

/// Adds a Helm repo with `repo_name` and `repo_url`.
#[instrument]
pub fn add_repo(repository_name: &str, repository_url: &str) -> Result<(), HelmError> {
    debug!("Add Helm repo");

    let result = helm_sys::add_helm_repository(repository_name, repository_url);

    if let Some(err) = helm_sys::to_helm_error(&result) {
        error!(
            "Go wrapper function go_add_helm_repo encountered an error: {}",
            err
        );

        return Err(HelmError::AddRepoError { error: err });
    }

    Ok(())
}

/// Retrieves the Helm index file from the repository URL.
#[instrument]
pub async fn get_helm_index<T>(repo_url: T) -> Result<HelmRepo, HelmError>
where
    T: AsRef<str> + std::fmt::Debug,
{
    debug!("Get Helm repo index file");

    let url = Url::parse(repo_url.as_ref()).context(UrlParseSnafu)?;
    let url = url.join(HELM_REPO_INDEX_FILE).context(UrlParseSnafu)?;

    debug!("Using {} to retrieve Helm index file", url);

    // TODO (Techassi): Use the FileTransferClient for that
    let index_file_content = reqwest::get(url)
        .await
        .context(RequestSnafu)?
        .text()
        .await
        .context(RequestSnafu)?;

    serde_yaml::from_str(&index_file_content).context(DeserializeYamlSnafu)
}
