<div align="center">

# celeris
A powerful, git-aware session manager written in Rust with a dynamic control layer in lua

[Features](#features) • [Installation](#installation) • [Usage](#usage) •
[Configuration](#configuring-celeris)
</div>

![show case of the main functionality](https://github.com/user-attachments/assets/9814a1d9-5101-43e6-9daf-eee9a80a164b)

## Features
- Quickly switch between sessions
- Create pre-configured layouts with lua which grants a lot more freedom than a declarative config
- Create layouts from git repos detected on the system
- Use custom templates to apply a default configuration for all layouts
- Integrate celeris into the tmux status bar
- Quickly switch to the last loaded layout(is helpful for example to launch the last loaded layout whenever tmux opens)
- Assemble your own workflows thanks to the modular design of the cli

## Requirements
- tmux

## Installation
```sh
cargo install celeris
```

## Usage
### Creating a layout
Creating a layout is pretty simple, just pass the path:
```sh
celeris create <path>
```
Optionally a custom name can be supplied with the `-n` flag(will be deduced automatically otherwise).
The layout file will be opened in your `$EDITOR` or if you set the editor in the main config that will take precedence.
You can also disable opening the layout file in the editor and just rely on the template file which we'll cover in [next sections](#custom-template) with the `-d` flag.
<br>
Created layouts are located in `<config-dir>/celeris/layouts/`(which is most commonly `~/.config/celeris/layouts/`)

### Creating layouts from git repos
You can search for git repositories on your system by doing:
```sh
celeris search
```
> [!NOTE]  
> For this to work you have to specify roots from which the search should be started in the main config file.
> Please look at the [config section](#configuring-celeris) for exact info on how to do that.

Now this can be used in a number of ways. Firstly you can create layouts from all those repos with:
```sh
celeris search | celeris create-all
```
A default template will be used for all of them, of course it can be [changed](#custom-template).
Secondly you can combine it with `fzf` to get a nice picker of the repos you want to create:
```sh
celeris create "$(celeris search | fzf --tmux)"
```

### Configuring celeris
There will be a generated config usually at `~/.config/celeris/config.toml`.
```toml
depth = 10 # Set the default depth of search
search_subdirs = false # Search in subdirectories of repositories. Default is `false`. Note, enabling this can significantly lengthen the search.

# Search roots from which the search will begin
search_roots = [
    { path = "/home/sentience/sources/projects/", depth = 3 }, # optionally a depth on a per-root basis can be supplied
    { path = "/home/sentience/dotfiles", excludes = ["wallpapers"] } # optionally an exclude list on a per-root basis can be supplied
] 

excludes = ["_deps"] # Excludes supplied directory names from the search
disable_template = false # Don't generate a template for each layout created
editor = "nvim" # Overrides the $EDITOR environment variable
```

### Configuring the layout
The configuration of a layout as mentioned uses lua.
It serves excellently as a dynamic control layer for all tmux commands.
The interface is designed to be way nicer to the user than the default tmux experience.
Great effort was put into making the state explicit, instead of relying on the usual implicitness of tmux.
As an example let us look at a slightly modified version of the default template:
> [!NOTE]
> All options supplied to constructor functions(those `.new` functions) are optional, they're shown here to demonstrate the full functionality

```lua
local celeris = require("celeris")

local session_root = "/tmp/";

-- Create a session
local session = celeris.Session.new({
    root = session_root -- the root is the working directory in which the session will start in
})

-- Create a named window
local window = celeris.Window.new(session, {
    name = "editor" -- name of the window
    root = "/tmp" -- window's working directory
})

-- Runs a command on a pane
window:default_pane():run_command("nvim")

-- Splits a pane into two panes either vertically or horizontally. The direction argument can be either "horizontal" or "vertical"
local _another_pane = window:default_pane():split("horizontal", {
    size = "20%" -- the size of a pane can be a percentage or can be an absolute value(just omit the %)
    root = "/tmp" -- pane's working directory
})

-- Selects a window to be focused
window:select()

-- Finally attaches to a session
session:attach()
```
And that's pretty much all there is to it.

#### One more thing
We have all these nice functions, but what if there isn't a function that makes tmux to do what I want?
Well say no more my friend I got something for you.
```lua
local output = celeris.rawCommand({ "display-message", "-p", "hello world!" })
print(output)
```
As you may have figured out already the `rawCommand` function will execute something similar to `tmux display-message -p "hello world!"` under the hood.
Of course we will get a nice `hello world!` message back.
This by itself, should satisfy some usecases but not all. What if I'd like to interact with a pane that is managed by the lua interface?
Well here target methods come in:
```lua
local window = celeris.Window.new(session, { name = "foo" })

local window_target = window:target()
local window_name = celeris.rawCommand({ "display-message", "-p", "-t", window_target, "#{window_name}"})
assert(window_name == "foo")
```
Each tmux component(session, window, pane) has a `target` method that can be used for interoperability with custom tmux commands.

> [!WARNING]
> Remember <b>with great power comes great responsibility</b> if you interact with components managed by celeris without knowing exactly what you're doing all sorts of weird things may happen.
> For example if you delete the pane that celeris manages it will most likely error out on you when it will try to use it.

### Switching between layouts
Relevant commands for this section:
```sh
celeris list 
```
Lists running and configured sessions(can be tweaked).
```sh
celeris switch
```
If a session is running switches to it, if it's not then loads it from the layout file if exists.
There is the `-l`/`--last-session` flag which spawns the last layout loaded previously.
It is useful for automatically opening a workspace on which you were working previously.
<br>
Now when we've created our layouts we can switch between the sessions quickly by configuring a binding in tmux:
```tmux
bind 'j' run-shell "celeris switch `celeris list | fzf --tmux` || true"
```
> [!NOTE]
> Note the `|| true` at the end. It's there as a workaround because normally when a process exits with a non-zero exit code tmux shows it's output on the screen, which can be useful for debugging, but it's also incredibly annoying

Now you can use whichever picker you want here. The possibilities are endless.

### Integrating with tmux
```tmux
set -g status-right " #(celeris list --tmux-format --only-running)"
```
With this we get a nice status bar which shows us in which session we are and which other ones are running:
![status bar showing active sessions](https://github.com/user-attachments/assets/6467c303-d5ec-489e-85c0-0b1c2d7a1aba)

### Other obvious commands
Here are some helper commands which can be useful
```sh 
celeris edit <name>
```
Opens the layout with this name in the $EDITOR(or you can set it to something else it in the main config).
```sh
celeris remove <name/s>
```
Removes one or more layouts with supplied names:

### Custom template
This template will be automatically written in by default to every layout created.
To use a custom one create a file at `<config_dir>/template.lua`(which is usually `~/.config/celeris/template.lua`)
Here's an example file:
```lua
local celeris = require("celeris")

local session_root = "{{session_root}}" -- Gets replaced with the real path on creation
local session = celeris.Session.new({ root = session_root })

local nvim = celeris.Window.new(session, { name = "nvim" })
nvim:default_pane():run_command("nvim")

local build = celeris.Window.new(session, {})
build:default_pane():split("horizontal", {})

nvim:select()
session:attach()
```
The only notable thing here is `{{session_root}}` it will get replaced with the real path at creation.
The template file uses the [handlebars](https://handlebarsjs.com/) templating syntax.
Here is a list of patterns that will be replaced at runtime:
- {{session_root}}
- {{session_name}}

## Acknowledgments
- [tsman](https://github.com/TecuceanuGabriel/tsman) - took inspiration from the readme format because I can't make things pretty
- [tmux-sessionizer](https://github.com/jrmoulton/tmux-sessionizer)
