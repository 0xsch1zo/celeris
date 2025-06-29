use color_eyre::Result;
use color_eyre::eyre::eyre;
use std::fs;
//use sesh::config::Config;
//use sesh::manifest::Manifest;
//use sesh::tui::picker;
//use sesh::tui::repo_search::RepoModel;
use sesh::session_config;

fn main() -> Result<()> {
    color_eyre::config::HookBuilder::default()
        .display_env_section(false)
        .install()?;
    //let config = Config::new()?;
    //let mut manifest = Manifest::new()?;
    //manifest.update_diff(&repos)?;
    //manifest.serialize()?;

    //picker(RepoModel::new(config))?;
    /*let session = Session::new(String::from("test"))?;
    let nvim = session.new_window(Some("neovim"), None)?;
    nvim.default_pane().run_command("nvim")?;

    let build = session.new_window(Some("build"), None)?;
    build.default_pane().run_command("echo hello")?;

    let tests = build.default_pane().split(Direction::Horizontal)?;
    tests.run_command("cargo test")?;*/
    //window.even_out(Direction::Vertical)?;
    //session.windows()[0].panes()[0].run_command("echo deeez nuts");
    //t.split_window("3", Split::Vertical)?;
    //t.run_command("3", "1", "ls")?;
    //t.even_out("3", Direction::Horizontal)?;

    let script = fs::read_to_string("test.rhai")?;
    session_config::run_script(&script)?;
    Ok(())
}
