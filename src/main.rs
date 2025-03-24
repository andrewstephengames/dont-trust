use raylib::{ffi::{SetConfigFlags, CameraProjection::CAMERA_PERSPECTIVE}, prelude::*};

enum State {
    Menu,
    Game,
    Paused
}

const FPS: u32 = 60;

fn main() {
    unsafe {
        SetConfigFlags(ConfigFlags::FLAG_WINDOW_RESIZABLE as u32)
    };
    let (mut window_x, mut window_y) = (1600, 900);
    let (mut rl, thread) = raylib::init()
        .size(window_x, window_y)
        .title("Hello, World")
        .build();
    
    let mut camera= Camera3D::perspective(
        Vector3 { x: 0.0, y: 0.0, z: 0.0 }, 
        Vector3 { x: 1.0, y: 1.0, z: 0.0 },
        Vector3 {x: 0.0, y: 0.0, z: 1.0},
        70.0
    );
    rl.set_target_fps(FPS);
    let (x, y, z) = (5.0, 5.0, 5.0);
    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);
        let mouse = d.get_mouse_position();
        let key = d.get_key_pressed();
        window_x = d.get_render_width();
        window_y = d.get_render_height();
        match key {
            Some(KeyboardKey::KEY_W) => camera.position.x += x,
            Some(KeyboardKey::KEY_A) => camera.position.y -= y,
            Some(KeyboardKey::KEY_S) => camera.position.x -= x,
            Some(KeyboardKey::KEY_D) => camera.position.y += y,
            Some(KeyboardKey::KEY_SPACE) => camera.position.z += z,
            Some(KeyboardKey::KEY_LEFT_SHIFT) => camera.position.z -= z,
            _ => {}
        }

        d.clear_background(Color::BLACK);
        d.draw_text(&format!("{}, {}", window_x, window_y), 0, 0, 30, Color::LIME);
        d.draw_text(&format!("{}, {}, {}", camera.position.x, camera.position.y, camera.position.z), 0, 50, 30, Color::RED);
        d.draw_fps(0, 100);
    }
}
