local sesh = require("sesh")

local session_root = "{{session_root}}";

-- Create a session
local session = sesh.Session.new({
    root = session_root -- the root is the working directory in which the session will start in
})

-- Create a named window
local window = sesh.Window.new(session, {
    -- name = "<window_name>" -- name of the window
    -- root = "<window_root>" -- window's working directory
})

-- Runs a command on a pane
-- window:default_pane():run_command("nvim")

-- Splits a pane into two panes either vertically or horizontally. The direction argument can be either "horizonal" or "vertical"
--[[ local _another_pane = window:default_pane():split("horizontal", {
    -- size = { type = "<size_type>", value = <size_val> } -- type can be either "absolute" or "percentage". Size of the pane either absolute or in percentages.
    -- root = <pane_root> -- pane's working directory
}) --]]

-- Selects a window to be focused
window:select()

-- Finally attaches to a session
session:attach()
