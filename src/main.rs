use raylib::{ffi::{SetConfigFlags, TextFormat}, prelude::*};

enum State {
    Menu,
    Game,
    Paused
}

fn main() {
    unsafe {
        SetConfigFlags(ConfigFlags::FLAG_WINDOW_RESIZABLE as u32)
    };
    let (mut window_x, mut window_y) = (1600, 900);
    let (mut rl, thread) = raylib::init()
        .size(window_x, window_y)
        .title("Hello, World")
        .build();

    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);
        window_x = d.get_render_width();
        window_y = d.get_render_height();

        d.clear_background(Color::BLACK);
        d.draw_text(&format!("{}, {}", window_x, window_y), 0, 0, 30, Color::LIME);
    }
}
