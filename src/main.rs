use std::ffi::CString;
use std::time::SystemTime;
use raylib::ffi::{ColorFromHSV, IsKeyDown};

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
const GRID_SIZE: ffi::Vector2 = NOISE_SIZE;

fn real_vec3_add (v1: Vector3, v2: Vector3) -> Vector3 {
    Vector3 {x: v1.x+v2.x, y: v1.y+v2.y, z: v1.z+v2.z}
}

fn real_vec3_sub (v1: Vector3, v2: Vector3) -> Vector3 {
    Vector3 {x: v1.x-v2.x, y: v1.y-v2.y, z: v1.z-v2.z}
}

fn get_height_at(world_pos: Vector3, terrain_origin: Vector3, grid: &Vec<Vector3>, width: usize, depth: usize) -> Option<f32> {
    let local_x = ((world_pos.x - terrain_origin.x) / (GRID_SIZE.x / width as f32)).floor() as usize;
    let local_z = ((world_pos.z - terrain_origin.z) / (GRID_SIZE.y / depth as f32)).floor() as usize;

    if local_x < width && local_z < depth {
        let index = local_z * width + local_x;
        Some(grid[index].y + terrain_origin.y)
    } else {
        None
    }
}


fn get_movement_vector(camera: &Camera3D) -> Vector3 {
    unsafe {
        let mut move_dir = Vector3::zero();

        // Get forward and right vectors from camera orientation
        let mut forward = camera.target - camera.position;
        forward.y = 0.0; // Prevent flying with W/S
        Vector3::normalize(&mut forward);
        
        let mut right = forward.cross(Vector3::up());
        Vector3::normalize(&mut right);
        
        // Vector3::normalize(&mut move_dir);
        // move_dir

        // Movement: WASD
        if IsKeyDown(ffi::KeyboardKey::KEY_W as i32) == true {
            // move_dir += forward;
            move_dir = real_vec3_add(move_dir, forward);
        }
        if IsKeyDown(ffi::KeyboardKey::KEY_S as i32) == true {
            // move_dir -= forward;
            move_dir = real_vec3_sub(move_dir, forward);
        }
        if IsKeyDown(ffi::KeyboardKey::KEY_D as i32) == true {
            // move_dir += right;
            move_dir = real_vec3_add(move_dir, right);
        }
        if IsKeyDown(ffi::KeyboardKey::KEY_A as i32) == true {
            // move_dir -= right;
            move_dir = real_vec3_sub(move_dir, right);
        }

        // Vertical movement: SPACE / SHIFT
        if IsKeyDown(ffi::KeyboardKey::KEY_SPACE as i32) == true {
            move_dir.y += 1.0;
        }
        if IsKeyDown(ffi::KeyboardKey::KEY_LEFT_SHIFT as i32) == true {
            move_dir.y -= 1.0;
        }

        if move_dir.length() > 0.0 {
            Vector3::normalize(&mut move_dir);
            move_dir
        } else {
            move_dir
        }
    }
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
    let mut yaw: f32 = 0.0;
    let mut pitch: f32 = 0.0;

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
    let terrain_vertices;
    let terrain_vertex_count;
	let mut mesh_points ;
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
        terrain_vertices = (*terrain_model.meshes).vertices;
        terrain_vertex_count = (*terrain_model.meshes).vertexCount;
        terrain_color = ColorFromHSV(130.0, 1.0, 1.0);
        mesh_points = Vec::with_capacity(terrain_vertex_count as usize);
	    let vert_slice: &[f32] = std::slice::from_raw_parts(terrain_vertices, (terrain_vertex_count * 3) as usize);
	
	    for i in 0..terrain_vertex_count{
	        let x = vert_slice[(i * 3) as usize];
	        let y = vert_slice[(i * 3 + 1) as usize];
	        let z = vert_slice[(i * 3 + 2) as usize];
	        mesh_points.push(Vector3::new(x, y, z));
            println!("{} {} {}", x, y, z);
	    }
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
        
		//         unsafe {
		//     // Raw movement input
		//     let input_x = (IsKeyDown(ffi::KeyboardKey::KEY_D as i32) as i64 - IsKeyDown(ffi::KeyboardKey::KEY_A as i32) as i64) as f32;
		//     let input_z = (IsKeyDown(ffi::KeyboardKey::KEY_W as i32) as i64 - IsKeyDown(ffi::KeyboardKey::KEY_S as i32) as i64) as f32;
		//     let input_y = (IsKeyDown(ffi::KeyboardKey::KEY_SPACE as i32) as i64 - IsKeyDown(ffi::KeyboardKey::KEY_LEFT_SHIFT as i32) as i64) as f32;
		
		//     // Compute yaw from camera direction
		//     let delta_x = camera.target.x - camera.position.x;
		//     let delta_z = camera.target.z - camera.position.z;
		//     let yaw = delta_z.atan2(delta_x);
		
		//     // Apply yaw rotation to input to get camera-relative movement
		//     player_direction.x = input_x * yaw.cos() - input_z * yaw.sin();
		//     player_direction.z = input_x * yaw.sin() + input_z * yaw.cos();
		//     player_direction.y = input_y;
		// }
        // 

        if let Some(terrain_y) = get_height_at(camera.position, raylib::prelude::Vector3::from(terrain_position), &mesh_points, GRID_SIZE.x as usize, GRID_SIZE.y as usize) {
	        if camera.position.y < terrain_y+5.0 {
                camera.position.y = terrain_y+5.1;
	        }
        }
        let key = d.get_key_pressed();
        window_x = d.get_render_width();
        window_y = d.get_render_height();
           let mouse_delta = d.get_mouse_delta();
	    let sensitivity = 0.003;
	    yaw += mouse_delta.x * sensitivity;
	    pitch -= mouse_delta.y * sensitivity;
	    
	    let pitch_limit = std::f32::consts::FRAC_PI_2 - 0.01;
	    pitch = pitch.clamp(-pitch_limit, pitch_limit);
	    
	    let forward = Vector3 {
	        x: pitch.cos() * yaw.sin(),
	        y: pitch.sin(),
	        z: pitch.cos() * yaw.cos() * -1.0
	    };
		
        player_direction = get_movement_vector(&camera);
        camera.position += player_direction * player_speed * dt;
        
        camera.target = camera.position + forward;

        // match key {
        //     Some(KeyboardKey::KEY_A) => camera.position.x += x,
        //     Some(KeyboardKey::KEY_S) => camera.position.z -= z,
        //     Some(KeyboardKey::KEY_D) => camera.position.x -= x,
        //     Some(KeyboardKey::KEY_W) => camera.position.z += z,
        //     Some(KeyboardKey::KEY_SPACE) => camera.position.y += y,
        //     Some(KeyboardKey::KEY_LEFT_SHIFT) => camera.position.y -= y,
        //     _ => {}
        // }

        // unsafe {
        //     // player_direction.x = (IsKeyDown(ffi::KeyboardKey::KEY_A as i32) as i64 - IsKeyDown(ffi::KeyboardKey::KEY_D as i32) as i64) as f32;
        //     // player_direction.y = (IsKeyDown(ffi::KeyboardKey::KEY_SPACE as i32) as i64 - IsKeyDown(ffi::KeyboardKey::KEY_LEFT_SHIFT as i32) as i64) as f32;
        //     // player_direction.z = (IsKeyDown(ffi::KeyboardKey::KEY_W as i32) as i64 - IsKeyDown(ffi::KeyboardKey::KEY_S as i32) as i64) as f32;
        // }
        
        // camera.position.x += player_direction.x * player_speed * dt;
        // camera.position.y += player_direction.y * player_speed * dt;
        // camera.position.z += player_direction.z * player_speed * dt;
        
        

        {
            let mut d3= d.begin_mode3D(camera);
            // d3.draw_grid(100, 1.0);
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