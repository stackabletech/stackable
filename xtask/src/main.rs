use clap::CommandFactory;
use clap_complete::{generate, Shell};
use clap_mangen::Man;
use snafu::{ResultExt, Snafu};
use stackablectl::cli::Cli;
use stackabled::api_doc::{ApiDoc, OpenApi};

use std::{env, fs};

#[derive(Debug, Snafu)]
enum TaskError {
    #[snafu(display("io error"))]
    Io { source: std::io::Error },
    #[snafu(display("error serializing openapi"))]
    SerializeOpenApi { source: serde_json::Error },
}

fn main() -> Result<(), TaskError> {
    let task = env::args().nth(1);

    match task {
        None => eprintln!("No task specified, available commands: 'gen-man' and 'gen-comp'"),
        Some(t) => match t.as_str() {
            "gen-man" => {
                let cmd = Cli::command();

                fs::create_dir_all("extra/man").context(IoSnafu {})?;
                let mut f = fs::File::create("extra/man/stackablectl.1").context(IoSnafu {})?;

                let man = Man::new(cmd);
                man.render(&mut f).context(IoSnafu {})?
            }
            "gen-comp" => {
                let mut cmd = Cli::command();
                let name = cmd.get_name().to_string();

                fs::create_dir_all("extra/completions").context(IoSnafu {})?;

                // Bash completions
                let mut f =
                    fs::File::create("extra/completions/stackablectl.bash").context(IoSnafu {})?;
                generate(Shell::Bash, &mut cmd, name.clone(), &mut f);

                // Fish completions
                let mut f =
                    fs::File::create("extra/completions/stackablectl.fish").context(IoSnafu {})?;
                generate(Shell::Fish, &mut cmd, name.clone(), &mut f);

                // ZSH completions
                let mut f =
                    fs::File::create("extra/completions/_stackablectl").context(IoSnafu {})?;
                generate(Shell::Zsh, &mut cmd, name, &mut f);
            }
            "gen-openapi" => {
                let openapi_json = ApiDoc::openapi().to_json().context(SerializeOpenApiSnafu)?;
                println!("{openapi_json}");
            }
            _ => panic!("Invalid task"),
        },
    }

    Ok(())
}
