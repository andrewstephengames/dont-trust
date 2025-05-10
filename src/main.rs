use std::ffi::CString;
use noise::{NoiseFn, Perlin, Seedable};
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
const NOISE_SIZE: ffi::Vector2 = ffi::Vector2{x: 256.0, y: 256.0};
const MAP_SIZE: ffi::Vector2 = ffi::Vector2{x: 100.0, y: 100.0};
const MAP_SCALE: f32 = MAP_SIZE.x*0.1;

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
        Vector3 { x: 5.0, y: 5.0, z: 5.0 }, 
        Vector3 { x: 0.0, y: 0.0, z: 0.0 },
        Vector3 {x: 0.0, y: 5.0, z: 0.0},
        45.0
    );
    let perlin = Perlin::new(1);
    let val = perlin.get([0.1, 0.0, 0.0]);
    println!("{}", val);

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
        terrain_mesh = GenMeshHeightmap(noise_image, ffi::Vector3{ x: MAP_SIZE.x, y: 8.0, z: MAP_SIZE.y });
        terrain_model = LoadModelFromMesh(terrain_mesh);
        SetMaterialTexture(cylinder_model.materials.wrapping_add(0), MATERIAL_MAP_ALBEDO as i32, cylinder_texture);
        terrain_material = *terrain_model.materials.wrapping_add(0);
        terrain_material_map = *terrain_material.maps.wrapping_add(0);
        terrain_material_map.texture = noise_texture;
        new_terrain_model = terrain_model;
        (*(*terrain_model.materials.add(0)).maps.wrapping_add(MATERIAL_MAP_ALBEDO as usize)).texture = noise_texture;
        UnloadImage(noise_image);
    }
    let mut terrain_position: ffi::Vector3 = ffi::Vector3 {x: -MAP_SIZE.x, y: 0.0, z: -MAP_SIZE.y};
    while !rl.window_should_close() {
        let mut dt = rl.get_frame_time();
        let mut d = rl.begin_drawing(&thread);
        // position.y += 2.0*dt;
        d.clear_background(Color::RAYWHITE);
        let mouse = d.get_mouse_position();
        let key = d.get_key_pressed();
        window_x = d.get_render_width();
        window_y = d.get_render_height();
        match key {
            Some(KeyboardKey::KEY_W) => camera.position.x += x,
            Some(KeyboardKey::KEY_A) => camera.position.z -= z,
            Some(KeyboardKey::KEY_S) => camera.position.x -= x,
            Some(KeyboardKey::KEY_D) => camera.position.z += z,
            Some(KeyboardKey::KEY_SPACE) => camera.position.y += y,
            Some(KeyboardKey::KEY_LEFT_SHIFT) => camera.position.y -= y,
            _ => {}
        }

        {
            let mut d3= d.begin_mode3D(camera);
            d3.draw_grid(100, 1.0);
            unsafe {
                DrawModel(terrain_model, terrain_position, 1.0, raylib::ffi::ColorFromHSV(130.0, 1.0, 1.0));
            }
            // d3.draw_line_3D(Vector3{x: -4.0, y: 0.0, z: -2.0}, Vector3{x: 5.0, y: 2.0, z: 3.0}, Color::LIME);
        }
        d.draw_text(&format!("{}, {}", window_x, window_y), 0, 0, 30, Color::LIME);
        d.draw_text(&format!("{}, {}, {}", camera.position.x, camera.position.y, camera.position.z), 0, 50, 30, Color::RED);
        d.draw_fps(0, 100);
    }
}