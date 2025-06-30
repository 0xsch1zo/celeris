let session = Session::build();
let window = session.new_window("test");
let default_pane = window.default_pane();
default_pane
    .split(Direction::Vertical)
    .split(Direction::Horizontal);
window.even_out(Direction::Horizontal);
