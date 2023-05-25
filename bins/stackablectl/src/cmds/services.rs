use clap::{Args, Subcommand};
use snafu::Snafu;

use crate::cli::OutputType;

#[derive(Debug, Args)]
pub struct ServicesArgs {
    #[command(subcommand)]
    subcommand: ServiceCommands,
}

#[derive(Debug, Subcommand)]
pub enum ServiceCommands {
    /// List deployed services
    #[command(alias("ls"))]
    List(ServiceListArgs),
}

#[derive(Debug, Args)]
pub struct ServiceListArgs {
    /// Will display services of all namespaces, not only the current one
    #[arg(short, long)]
    all_namespaces: bool,

    /// Display credentials and secrets in the output
    #[arg(short, long)]
    show_credentials: bool,

    /// Display product versions in the output
    #[arg(long)]
    show_versions: bool,

    #[arg(short, long = "output", value_enum, default_value_t = Default::default())]
    output_type: OutputType,
}

#[derive(Debug, Snafu)]
pub enum ServicesCmdError {
    #[snafu(display("unable to format yaml output:: {source}"))]
    YamlError { source: serde_yaml::Error },

    #[snafu(display("unable to format json output:: {source}"))]
    JsonError { source: serde_json::Error },
}

impl ServicesArgs {
    pub fn run(&self) -> Result<String, ServicesCmdError> {
        match &self.subcommand {
            ServiceCommands::List(args) => list_cmd(args),
        }
    }
}

fn list_cmd(_args: &ServiceListArgs) -> Result<String, ServicesCmdError> {
    todo!()
}
