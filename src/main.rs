use std::ffi::CString;
use std::time::SystemTime;
use raylib::ffi::{ColorFromHSV, IsKeyDown};
use std::ptr;
use std::mem;

use raylib::{
    consts::MaterialMapIndex::*, core::{math::*, texture::*}, ffi::{DrawModel, DrawModelWires, GenImageGradientLinear, GenImagePerlinNoise, GenMeshCube, GenMeshCylinder, GenMeshHeightmap, LoadModel, LoadModelFromMesh, LoadTextureFromImage, SetConfigFlags, SetMaterialTexture, UnloadImage}, prelude::*
};

enum State {
    Menu,
    Game,
    Paused
}

const FPS: u32 = 60;
const NOISE_SIZE: ffi::Vector2 = ffi::Vector2{x: 128.0, y: 128.0};
const MAP_SIZE: ffi::Vector2 = ffi::Vector2{x: 500.0, y: 500.0};
const MAP_SCALE: f32 = MAP_SIZE.x*0.05;

fn real_vec3_add (v1: ffi::Vector3, v2: ffi::Vector3) -> ffi::Vector3 {
    ffi::Vector3 {x: v1.x+v2.x, y: v1.y+v2.y, z: v1.z+v2.z}
}

fn main() {
    unsafe {
        SetConfigFlags(ConfigFlags::FLAG_WINDOW_RESIZABLE as u32)
    };
    let (mut window_x, mut window_y) = (1920, 1080);
    let (mut rl, thread) = raylib::init()
        .size(window_x, window_y)
        .title("Hello, World")
        .build();
    
    let mut camera= Camera3D::perspective(
        Vector3 { x: -500.0, y: 25.0, z: -500.0 }, 
        Vector3 { x: -100.0, y: 12.0, z: -100.0 },
        Vector3 {x: 0.0, y: 25.0, z: 0.0},
        45.0
    );
    rl.set_target_fps(FPS);
    let (x, y, z) = (5.0, 5.0, 5.0);
    let mut mesh: ffi::Mesh;
    let mut model: ffi::Model;
    let mut cylinder_mesh: ffi::Mesh;
    let mut cylinder_model: ffi::Model;
    let mut cylinder_image: ffi::Image;
    let mut cylinder_texture: ffi::Texture;
    let mut pyramid: ffi::Model;
    let pyramid_path = CString::new("./src/Pyramid.glb").expect("cstr failed");
    let pyramid_ptr: *const i8 = pyramid_path.as_ptr();
    let noise_image: ffi::Image;
    let noise_texture: ffi::Texture2D;
    let mut terrain_mesh: ffi::Mesh;
    let mut terrain_model: ffi::Model;
    let mut terrain_material: ffi::Material;
    let mut terrain_material_map: ffi::MaterialMap;
    let mut new_terrain_mesh: *mut ffi::Mesh;
    let mut new_terrain_model: ffi::Model;
    let mut new_terrain_material: *mut ffi::Material;
    let mut new_terrain_material_map: *mut ffi::MaterialMap;
    let mut terrain_color: ffi::Color; 
    let date = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .expect("Duration since UNIX_EPOCH failed");
    let mut player_direction: Vector3 = Vector3{ x: 0.0, y: 0.0, z: 0.0};
    let mut player_speed = 50.0;
    unsafe {
        mesh = GenMeshCube(1.0, 1.0, 1.0);
        cylinder_mesh = GenMeshCylinder(1.0, 2.0, 100);
        cylinder_model = LoadModelFromMesh(cylinder_mesh);
        model = LoadModelFromMesh(mesh);
        cylinder_image = GenImageGradientLinear(20, 20, 1, ffi::ColorFromHSV(0.2, 0.7, 1.0), ffi::ColorFromHSV(0.5, 1.0, 1.0));
        cylinder_texture = LoadTextureFromImage(cylinder_image);
        pyramid = LoadModel(pyramid_ptr);
        noise_image = GenImagePerlinNoise(NOISE_SIZE.x as i32, NOISE_SIZE.y as i32, 0, 0, MAP_SCALE);
        noise_texture = LoadTextureFromImage(noise_image);
        terrain_mesh = GenMeshHeightmap(noise_image, ffi::Vector3{ x: MAP_SIZE.x, y: MAP_SCALE, z: MAP_SIZE.y });
        terrain_model = LoadModelFromMesh(terrain_mesh);
        SetMaterialTexture(cylinder_model.materials.wrapping_add(0), MATERIAL_MAP_ALBEDO as i32, cylinder_texture);
        terrain_material = *terrain_model.materials.wrapping_add(0);
        terrain_material_map = *terrain_material.maps.wrapping_add(0);
        terrain_material_map.texture = noise_texture;
        new_terrain_model = terrain_model;
        (*(*terrain_model.materials.add(0)).maps.wrapping_add(MATERIAL_MAP_ALBEDO as usize)).texture = noise_texture;
        terrain_color = ColorFromHSV(130.0, 1.0, 1.0);
        UnloadImage(noise_image);
    }
    let mut terrain_position: ffi::Vector3 = ffi::Vector3 {x: -MAP_SIZE.x, y: 0.0, z: -MAP_SIZE.y};
    rl.disable_cursor();
    while !rl.window_should_close() {
        let mut dt = rl.get_frame_time();
        let mut d = rl.begin_drawing(&thread);
        // position.y += 2.0*dt;
        d.clear_background(Color::SKYBLUE);
        let mouse = d.get_mouse_position();
        camera.target = Vector3{x: -mouse.x, y: -mouse.y, z: mouse.x};
        let key = d.get_key_pressed();
        window_x = d.get_render_width();
        window_y = d.get_render_height();
        // match key {
        //     Some(KeyboardKey::KEY_A) => camera.position.x += x,
        //     Some(KeyboardKey::KEY_S) => camera.position.z -= z,
        //     Some(KeyboardKey::KEY_D) => camera.position.x -= x,
        //     Some(KeyboardKey::KEY_W) => camera.position.z += z,
        //     Some(KeyboardKey::KEY_SPACE) => camera.position.y += y,
        //     Some(KeyboardKey::KEY_LEFT_SHIFT) => camera.position.y -= y,
        //     _ => {}
        // }

        unsafe {
            player_direction.x = (IsKeyDown(ffi::KeyboardKey::KEY_A as i32) as i64 - IsKeyDown(ffi::KeyboardKey::KEY_D as i32) as i64) as f32;
            player_direction.y = (IsKeyDown(ffi::KeyboardKey::KEY_LEFT_SHIFT as i32) as i64 - IsKeyDown(ffi::KeyboardKey::KEY_SPACE as i32) as i64) as f32;
            player_direction.z = (IsKeyDown(ffi::KeyboardKey::KEY_W as i32) as i64 - IsKeyDown(ffi::KeyboardKey::KEY_S as i32) as i64) as f32;
        }
        
        camera.position.x += player_direction.x * player_speed * dt;
        camera.position.y += player_direction.y * player_speed * dt;
        camera.position.z += player_direction.z * player_speed * dt;

        {
            let mut d3= d.begin_mode3D(camera);
            d3.draw_grid(100, 1.0);
            unsafe {
                // for i in 0..25 {
                //     for j in 0..25 {
                //         DrawModel(terrain_model, real_vec3_add(terrain_position, ffi::Vector3{x: i as f32 * 10.0, y: 0.0, z: j as f32 * 10.0}), 1.0, terrain_color);
                //     }
                // }
                DrawModel(terrain_model, terrain_position, 1.0, terrain_color);
            }
            // d3.draw_line_3D(Vector3{x: -4.0, y: 0.0, z: -2.0}, Vector3{x: 5.0, y: 2.0, z: 3.0}, Color::LIME);
        }
        d.draw_text(&format!("{}, {}", window_x, window_y), 0, 0, 30, Color::LIME);
        d.draw_text(&format!("{}, {}, {}", camera.position.x, camera.position.y, camera.position.z), 0, 50, 30, Color::RED);
        d.draw_fps(0, 100);
    }
}