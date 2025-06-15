use sesh::config;
use sesh::search;
use std::process;

fn main() {
    let config = match config::Config::new() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    };

    let repos = search::search(&config);
    if let Err(err) = repos {
        eprint!("{err}");
        process::exit(1);
    }
}
