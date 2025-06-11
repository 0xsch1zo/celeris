use sesh::config;

fn main() {
    let err = match config::Config::new() {
        Err(e) => println!("{e}"),
        Ok(_) => (),
    };
}
