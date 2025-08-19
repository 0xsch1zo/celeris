local celeris = require("celeris")

local session_root = "{{session_root}}"

local session = celeris.Session.new({ root = session_root })

local window = celeris.Window.new(session, {
    name = "test",
    root = "{{session_root}}",
    raw_command = "'cat'", -- to hang indefinitely
})

local window2 = celeris.Window.new(session, {
    name = "test2",
    root = "{{session_root}}",
})

window2:select()

local pane = window2:default_pane():split("horizontal", {
    root = "{{session_root}}",
    size = "3",
})

window2:default_pane():split("vertical", {
    root = "{{session_root}}",
    size = "20%",
})

pane:select()
pane:run_command("echo test")

window2:even_out("horizontal")

local session_name = celeris.rawCommand({ "display-message", "-p", "-t", session:target(), "#{session_name}" })
    :gsub("%s+", "")
assert(session_name == "{{session_name}}", "Query for session name should match the session_name")


local window_name = celeris.rawCommand({ "display-message", "-p", "-t", window:target(), "#{window_name}" })
    :gsub("%s+", "")
assert(window_name == "test", "Query for window name should match")


local pane_id = celeris.rawCommand({ "display-message", "-p", "-t", pane:target(), "#{pane_id}" })
    :gsub("%s+", "")
print(pane:target())
print(pane_id)
assert(string.find(pane:target(), pane_id, 1, true) ~= nil,
    "Query for pane id should contain itself wthin the pane target returned by function")
