local celeris = require("celeris")

local session_root = "{{session_root}}"

local session = celeris.Session.new({ root = session_root })

celeris.Window.new(session, {
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
    size = { type = "absolute", value = 4 },
})

window2:default_pane():split("vertical", {
    root = "{{session_root}}",
    size = { type = "percentage", value = 20 },
})

pane:select()
pane:run_command("echo test")

window2:even_out("horizontal")
