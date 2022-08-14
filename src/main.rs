mod gitgat;

extern crate clap;

use clap::{AppSettings, Arg, Command};

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

fn build_cli() -> Command<'static> {
    Command::new("gitgat")
        .version(VERSION.unwrap_or("(unversioned)"))
        .author("Johan McQuillan <johangmcquillan@gmail.com>")
        .about("Pipes stuff into fzf")
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::GlobalVersion)
        .arg(Arg::new("author").value_name("AUTHOR").required(true).help("Author name"))
        .arg(
            Arg::new("exclude")
                .short('e')
                .long("exclude")
                .value_name("EXCLUDE")
                .takes_value(true)
                .multiple_values(true)
                .use_value_delimiter(true)
                .require_delimiter(true)
                .help("Exclude changes to specified directories.\nMultiple directories are delimited by commas."),
        )
}

fn main() {
    let matches = build_cli().get_matches();
    gitgat::run(gitgat::Opts {
        author: matches.get_one::<String>("author").unwrap(),
        excluded_dirs: matches
            .get_many::<String>("exclude")
            .unwrap_or_default()
            .map(|o| o.as_str())
            .collect(),
    });
}
