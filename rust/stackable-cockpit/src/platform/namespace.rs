use snafu::{ResultExt, Snafu};

use crate::utils::k8s;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("failed to create kubernetes client"))]
    KubeClientCreateError { source: k8s::Error },

    #[snafu(display("permission denied - try to create the namespace manually or choose an already existing one to which you have access to"))]
    PermissionDenied,
}

/// Creates a namespace with `name` if needed (not already present in the
/// cluster).
pub async fn create_if_needed(name: String) -> Result<(), Error> {
    let client = k8s::Client::new().await.context(KubeClientCreateSnafu)?;

    client
        .create_namespace_if_needed(name)
        .await
        .map_err(|err| match err {
            k8s::Error::KubeError { source } => match source {
                kube::Error::Api(err) if err.code == 401 => Error::PermissionDenied,
                _ => Error::KubeClientCreateError {
                    source: k8s::Error::KubeError { source },
                },
            },
            _ => Error::KubeClientCreateError { source: err },
        })
}
