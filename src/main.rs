#![feature(rust_2018_preview)]
#![warn(rust_2018_idioms)]
#![feature(rust_2018_preview, use_extern_macros)]

use clap::{App, AppSettings, load_yaml};

mod precmd;

fn main() {
    // Load our CLI args from the yaml file
    let yaml = load_yaml!("../cli_definitions.yml");
    let matches = App::from_yaml(yaml)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .get_matches();

    // Print the line corresponding to the subcommand
    match matches.subcommand() {
        ("precmd", Some(sub_matchings)) => precmd::render(sub_matchings),
        // ("prompt", Some(sub_matchings)) => prompt::render(sub_matchings),
        _ => (),
    }
}
