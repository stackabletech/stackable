use clap::{Args, Subcommand};
use comfy_table::{
    presets::{NOTHING, UTF8_FULL},
    ContentArrangement, Row, Table,
};
use snafu::{ResultExt, Snafu};
use tracing::{debug, info, instrument};

use stackable_cockpit::{
    common::list,
    constants::{DEFAULT_OPERATOR_NAMESPACE, DEFAULT_PRODUCT_NAMESPACE},
    platform::{
        demo::{DemoError, DemoList},
        namespace::{self},
        release::ReleaseList,
        stack::StackList,
    },
    utils::path::PathOrUrlParseError,
    xfer::{cache::Cache, FileTransferClient, FileTransferError},
};

use crate::{
    args::{CommonClusterArgs, CommonClusterArgsError, CommonNamespaceArgs},
    cli::{Cli, OutputType},
};

#[derive(Debug, Args)]
pub struct DemoArgs {
    #[command(subcommand)]
    subcommand: DemoCommands,
}

#[derive(Debug, Subcommand)]
pub enum DemoCommands {
    /// List available demos
    #[command(alias("ls"))]
    List(DemoListArgs),

    /// Print out detailed demo information
    #[command(alias("desc"))]
    Describe(DemoDescribeArgs),

    /// Install a specific demo
    #[command(aliases(["i", "in"]))]
    Install(DemoInstallArgs),
}

#[derive(Debug, Args)]
pub struct DemoListArgs {
    #[arg(short, long = "output", value_enum, default_value_t = Default::default())]
    output_type: OutputType,
}

#[derive(Debug, Args)]
pub struct DemoDescribeArgs {
    /// Demo to describe
    #[arg(
        name = "DEMO",
        long_help = "Demo to describe

Use \"stackablectl demo list\" to display a list of available demos.
Use \"stackablectl demo install <DEMO>\" to install a specific demo."
    )]
    demo_name: String,

    #[arg(short, long = "output", value_enum, default_value_t = Default::default())]
    output_type: OutputType,
}

#[derive(Debug, Args)]
pub struct DemoInstallArgs {
    /// Demo to install
    #[arg(
        name = "DEMO",
        long_help = "Demo to install

Use \"stackablectl demo list\" to display a list of available demos.
Use \"stackablectl demo describe <DEMO>\" to display details about the specified demo."
    )]
    demo_name: String,

    /// Skip the installation of the release during the stack install process
    #[arg(
        long,
        long_help = "Skip the installation of the release during the stack install process

Use \"stackablectl operator install [OPTIONS] <OPERATORS>...\" to install
required operators manually. Operators MUST be installed in the correct version.

Use \"stackablectl operator install --help\" to display more information on how
to specify operator versions."
    )]
    skip_release: bool,

    /// List of parameters to use when installing the stack
    #[arg(long)]
    stack_parameters: Vec<String>,

    /// List of parameters to use when installing the demo
    #[arg(long)]
    parameters: Vec<String>,

    #[command(flatten)]
    local_cluster: CommonClusterArgs,

    #[command(flatten)]
    namespaces: CommonNamespaceArgs,
}

#[derive(Debug, Args)]
pub struct DemoUninstallArgs {}

#[derive(Debug, Snafu)]
pub enum CmdError {
    #[snafu(display("unable to format YAML output"))]
    YamlOutputFormatError { source: serde_yaml::Error },

    #[snafu(display("unable to format JSON output"))]
    JsonOutputFormatError { source: serde_json::Error },

    #[snafu(display("no demo with name '{name}'"))]
    NoSuchDemo { name: String },

    #[snafu(display("no stack with name '{name}'"))]
    NoSuchStack { name: String },

    #[snafu(display("list error"))]
    ListError { source: list::Error },

    #[snafu(display("demo error"))]
    DemoError { source: DemoError },

    #[snafu(display("path/url parse error"))]
    PathOrUrlParseError { source: PathOrUrlParseError },

    #[snafu(display("cluster argument error"))]
    CommonClusterArgsError { source: CommonClusterArgsError },

    #[snafu(display("file transfer error"))]
    TransferError { source: FileTransferError },

    #[snafu(display("failed to create namespace '{namespace}'"))]
    NamespaceError {
        source: namespace::Error,
        namespace: String,
    },
}

impl DemoArgs {
    #[instrument]
    pub async fn run(&self, cli: &Cli, cache: Cache) -> Result<String, CmdError> {
        debug!("Handle demo args");

        let transfer_client = FileTransferClient::new_with(cache);

        // Build demo list based on the (default) remote demo file, and additional files provided by the
        // STACKABLE_DEMO_FILES env variable or the --demo-files CLI argument.
        let files = cli.get_demo_files().context(PathOrUrlParseSnafu)?;

        let list = DemoList::build(&files, &transfer_client)
            .await
            .context(ListSnafu)?;

        match &self.subcommand {
            DemoCommands::List(args) => list_cmd(args, cli, list).await,
            DemoCommands::Describe(args) => describe_cmd(args, cli, list).await,
            DemoCommands::Install(args) => install_cmd(args, cli, list, &transfer_client).await,
        }
    }
}

/// Print out a list of demos, either as a table (plain), JSON or YAML
#[instrument]
async fn list_cmd(args: &DemoListArgs, cli: &Cli, list: DemoList) -> Result<String, CmdError> {
    info!("Listing demos");

    match args.output_type {
        OutputType::Plain => {
            let mut table = Table::new();

            table
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec!["#", "NAME", "STACK", "DESCRIPTION"])
                .load_preset(UTF8_FULL);

            for (index, (demo_name, demo_spec)) in list.inner().iter().enumerate() {
                let row = Row::from(vec![
                    (index + 1).to_string(),
                    demo_name.clone(),
                    demo_spec.stack.clone(),
                    demo_spec.description.clone(),
                ]);
                table.add_row(row);
            }

            let mut result = cli.result();

            result
                .with_command_hint(
                    "stackablectl demo describe [OPTIONS] <DEMO>",
                    "display further information for the specified demo",
                )
                .with_command_hint(
                    "stackablectl demo install [OPTIONS] <DEMO>",
                    "install a demo",
                )
                .with_output(table.to_string());

            Ok(result.render())
        }
        OutputType::Json => serde_json::to_string(&list.inner()).context(JsonOutputFormatSnafu),
        OutputType::Yaml => serde_yaml::to_string(&list.inner()).context(YamlOutputFormatSnafu),
    }
}

/// Describe a specific demo by printing out a table (plain), JSON or YAML
#[instrument]
async fn describe_cmd(
    args: &DemoDescribeArgs,
    cli: &Cli,
    list: DemoList,
) -> Result<String, CmdError> {
    info!("Describing demo {}", args.demo_name);

    let demo = list.get(&args.demo_name).ok_or(CmdError::NoSuchDemo {
        name: args.demo_name.clone(),
    })?;

    match args.output_type {
        OutputType::Plain => {
            let mut table = Table::new();
            table
                .load_preset(NOTHING)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .add_row(vec!["DEMO", &args.demo_name])
                .add_row(vec!["DESCRIPTION", &demo.description])
                .add_row_if(
                    |_, _| demo.documentation.is_some(),
                    vec!["DOCUMENTATION", demo.documentation.as_ref().unwrap()],
                )
                .add_row(vec!["STACK", &demo.stack])
                .add_row(vec!["LABELS", &demo.labels.join(", ")]);

            // TODO (Techassi): Add parameter output

            let mut result = cli.result();

            result
                .with_command_hint(
                    format!("stackablectl demo install {}", args.demo_name),
                    "install the demo",
                )
                .with_command_hint("stackablectl demo list", "list all available demos")
                .with_output(table.to_string());

            Ok(result.render())
        }
        OutputType::Json => serde_json::to_string(&demo).context(JsonOutputFormatSnafu),
        OutputType::Yaml => serde_yaml::to_string(&demo).context(YamlOutputFormatSnafu),
    }
}

/// Install a specific demo
#[instrument(skip(list))]
async fn install_cmd(
    args: &DemoInstallArgs,
    cli: &Cli,
    list: DemoList,
    transfer_client: &FileTransferClient,
) -> Result<String, CmdError> {
    info!("Installing demo {}", args.demo_name);

    // Init result output and progress output
    let mut output = cli.result();
    output.enable_progress(format!("Installing demo '{}'", args.demo_name));

    let demo_spec = list.get(&args.demo_name).ok_or(CmdError::NoSuchDemo {
        name: args.demo_name.clone(),
    })?;

    // TODO (Techassi): Try to move all this boilerplate code to build the lists out of here
    let files = cli.get_stack_files().context(PathOrUrlParseSnafu)?;
    let stack_list = StackList::build(&files, transfer_client)
        .await
        .context(ListSnafu)?;

    let files = cli.get_release_files().context(PathOrUrlParseSnafu)?;

    let release_list = ReleaseList::build(&files, transfer_client)
        .await
        .context(ListSnafu)?;

    // Install local cluster if needed
    output.set_progress_message("Creating local cluster");
    args.local_cluster
        .install_if_needed(None)
        .await
        .context(CommonClusterArgsSnafu)?;

    let operator_namespace = args
        .namespaces
        .operator_namespace
        .clone()
        .unwrap_or(DEFAULT_OPERATOR_NAMESPACE.into());

    let product_namespace = args
        .namespaces
        .product_namespace
        .clone()
        .unwrap_or(DEFAULT_PRODUCT_NAMESPACE.into());

    if !args.skip_release {
        output.set_progress_message("Creating operator namespace");
        namespace::create_if_needed(operator_namespace.clone())
            .await
            .context(NamespaceSnafu {
                namespace: operator_namespace.clone(),
            })?;
    }

    output.set_progress_message("Creating product namespace");
    namespace::create_if_needed(product_namespace.clone())
        .await
        .context(NamespaceSnafu {
            namespace: product_namespace.clone(),
        })?;

    output.set_progress_message("Installing demo manifests");
    demo_spec
        .install(
            stack_list,
            release_list,
            &operator_namespace,
            &product_namespace,
            &args.stack_parameters,
            &args.parameters,
            transfer_client,
            args.skip_release,
        )
        .await
        .context(DemoSnafu)?;

    let operator_cmd = format!(
        "stackablectl operator installed{}",
        if args.namespaces.operator_namespace.is_some() {
            format!(" --operator-namespace {}", operator_namespace)
        } else {
            "".into()
        }
    );

    let stacklet_cmd = format!(
        "stackablectl stacklet list{}",
        if args.namespaces.product_namespace.is_some() {
            format!(" --product-namespace {}", product_namespace)
        } else {
            "".into()
        }
    );

    output
        .with_command_hint(operator_cmd, "display the installed operators")
        .with_command_hint(stacklet_cmd, "display the installed stacklets")
        .with_output(format!("Installed demo '{}'", args.demo_name));

    output.finish_progress("Done");
    Ok(output.render())
}
