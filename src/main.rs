use std::collections::{HashMap, VecDeque};
use std::ffi::CString;
use std::sync::Arc;
use rand::Rng;

use tokio::sync::{mpsc, Mutex as TokioMutex};

use raylib::consts::MouseButton;
use raylib::ffi::{Color, ColorFromHSV, IsKeyDown, IsKeyReleased, KeyboardKey};
use raylib::{
    consts::MaterialMapIndex::*,
    core::math::*, // RaylibThread is in prelude, texture::* also generally covered
    ffi::{
        DrawModel, DrawModelEx, GenImagePerlinNoise, GenMeshHeightmap, LoadModel,
        LoadModelFromMesh, LoadTextureFromImage, SetConfigFlags, UnloadImage,
    },
    prelude::*, // Imports RaylibThread
};

use serde::{Deserialize, Serialize};

use futures_util::{SinkExt, StreamExt};
// Use the specific version of tokio-tungstenite the compiler is using if known, or a recent one
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};
use url::Url; // Keep this if you still want to parse URLs, but connect_async will take &str


// --- WebSocket and Game State Structures ---
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerState {
    id: String,
    position: (f32, f32, f32),
    rotation: (f32, f32, f32),
}

struct GameState {
    local_player_id: Option<String>,
    other_players: HashMap<String, PlayerState>,
    join_messages: VecDeque<String>,
}

const MAX_JOIN_MESSAGES: usize = 5;
const FPS: u32 = 60;
const NOISE_SIZE: raylib::ffi::Vector2 = raylib::ffi::Vector2 { x: 128.0, y: 128.0 };
const MAP_SIZE: raylib::ffi::Vector2 = raylib::ffi::Vector2 { x: 500.0, y: 500.0 };
const MAP_SCALE: f32 = MAP_SIZE.x * 0.05;
const PLAYER_HEIGHT: f32 = 5.0;

fn real_vec3_add(v1: Vector3, v2: Vector3) -> Vector3 {
    Vector3 { x: v1.x + v2.x, y: v1.y + v2.y, z: v1.z + v2.z }
}

fn real_vec3_sub(v1: Vector3, v2: Vector3) -> Vector3 {
    Vector3 { x: v1.x - v2.x, y: v1.y - v2.y, z: v1.z - v2.z }
}

fn get_closest_vertex_height(
    world_pos: Vector3,
    terrain_origin: Vector3,
    vertices: &[Vector3],
    width: usize,
    depth: usize,
) -> Option<f32> {
    if width == 0 || depth == 0 || vertices.is_empty() {
        return None;
    }
    let grid_cell_width = MAP_SIZE.x / (width.saturating_sub(1) as f32).max(1.0);
    let grid_cell_depth = MAP_SIZE.y / (depth.saturating_sub(1) as f32).max(1.0);

    let local_x_float = (world_pos.x - terrain_origin.x) / grid_cell_width;
    let local_z_float = (world_pos.z - terrain_origin.z) / grid_cell_depth;

    let local_x = local_x_float.round() as usize;
    let local_z = local_z_float.round() as usize;

    if local_x >= width || local_z >= depth {
        return None;
    }

    let index = local_z * width + local_x;
    if index < vertices.len() {
        Some(vertices[index].y + terrain_origin.y)
    } else {
        None
    }
}

fn check_collision(
    position: Vector3,
    terrain_origin: Vector3,
    vertices: &[Vector3],
    width: usize,
    depth: usize,
) -> bool {
    if let Some(ground_height) =
        get_closest_vertex_height(position, terrain_origin, vertices, width, depth)
    {
        position.y < ground_height + PLAYER_HEIGHT
    } else {
        false
    }
}

fn adjust_position(
    position: Vector3,
    terrain_origin: Vector3,
    vertices: &[Vector3],
    width: usize,
    depth: usize,
) -> Vector3 {
    let mut adjusted_pos = position;
    if let Some(ground_height) =
        get_closest_vertex_height(position, terrain_origin, vertices, width, depth)
    {
        if position.y < ground_height + PLAYER_HEIGHT {
            adjusted_pos.y = ground_height + PLAYER_HEIGHT;
        }
    }
    adjusted_pos
}

fn get_movement_vector(rl: &RaylibHandle, camera: &Camera3D, current_yaw: f32) -> Vector3 {
    let mut move_dir = Vector3::zero();
    let mut forward = camera.target - camera.position;
    forward.y = 0.0;

    if forward.length() < 0.0001 { 
        forward = Vector3::new(current_yaw.sin(), 0.0, current_yaw.cos() * -1.0);
    }
    // Only normalize if length is not zero, to avoid NaN issues with Vector3::zero().normalize()
    if forward.length() > 0.0001 { // Check again after potential modification
        Vector3::normalize(&mut forward);
    }


    let mut right = forward.cross(Vector3::up());
    if right.length() > 0.0001 { // Check before normalizing
       Vector3::normalize(&mut right);
    }


    if rl.is_key_down(KeyboardKey::KEY_W) {
        move_dir = real_vec3_add(move_dir, forward);
    }
    if rl.is_key_down(KeyboardKey::KEY_S) {
        move_dir = real_vec3_sub(move_dir, forward);
    }
    if rl.is_key_down(KeyboardKey::KEY_D) {
        move_dir = real_vec3_add(move_dir, right);
    }
    if rl.is_key_down(KeyboardKey::KEY_A) {
        move_dir = real_vec3_sub(move_dir, right);
    }
    if rl.is_key_down(KeyboardKey::KEY_SPACE) {
        move_dir.y += 1.0;
    }
    if rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT) {
        move_dir.y -= 1.0;
    }

    if move_dir.length() > 0.0001 {
        Vector3::normalize(&mut move_dir);
    }
    move_dir
}

async fn connect_and_manage_websocket(
    mut local_player_state_rx: mpsc::UnboundedReceiver<PlayerState>,
    server_updates_tx: mpsc::UnboundedSender<Vec<PlayerState>>,
    player_id_confirmation_tx: mpsc::UnboundedSender<String>,
    game_state_accessor: Arc<TokioMutex<GameState>>,
) {
    let url_str = "ws://127.0.0.1:8080/ws"; // Use &str directly

    match connect_async(url_str).await { // CORRECTED: Pass &str
        Ok((ws_stream, _)) => {
            println!("CLIENT: Successfully connected to WebSocket server.");
            let (mut write, mut read) = ws_stream.split();

            let send_task = tokio::spawn(async move {
                while let Some(player_state) = local_player_state_rx.recv().await {
                    if let Ok(json_state) = serde_json::to_string(&player_state) {
                        // CORRECTED: Use .into() for WsMessage::Text
                        if write.send(WsMessage::Text(json_state.into())).await.is_err() {
                            eprintln!("CLIENT: Failed to send player state to server.");
                            break;
                        }
                    }
                }
                println!("CLIENT: Send task finished.");
            });

            let receive_task = tokio::spawn(async move {
                let mut local_id_determined = false;
                loop {
                    tokio::select! {
                        Some(msg_result) = read.next() => {
                            match msg_result {
                                Ok(WsMessage::Text(text)) => {
                                    if let Ok(all_player_states) = serde_json::from_str::<Vec<PlayerState>>(&text) {
                                        if !local_id_determined {
                                            let mut gs_lock = game_state_accessor.lock().await;
                                            if gs_lock.local_player_id.is_none() {
                                                for r_state in &all_player_states {
                                                    let is_initial_pos = r_state.position.0.abs() < 0.1 &&
                                                                           (r_state.position.1 - 5.0).abs() < 0.1 &&
                                                                           r_state.position.2.abs() < 0.1;
                                                    if !gs_lock.other_players.contains_key(&r_state.id) {
                                                        if is_initial_pos {
                                                            println!("CLIENT: Deduced my ID by initial position: {}", r_state.id);
                                                            gs_lock.local_player_id = Some(r_state.id.clone());
                                                            let _ = player_id_confirmation_tx.send(r_state.id.clone());
                                                            local_id_determined = true;
                                                            break; 
                                                        }
                                                        if gs_lock.local_player_id.is_none() { 
                                                            println!("CLIENT: Tentatively deduced my ID (new unknown): {}", r_state.id);
                                                            gs_lock.local_player_id = Some(r_state.id.clone());
                                                            let _ = player_id_confirmation_tx.send(r_state.id.clone());
                                                        }
                                                    }
                                                }
                                                if gs_lock.local_player_id.is_some() {
                                                    local_id_determined = true;
                                                } else if all_player_states.len() == 1 {
                                                    println!("CLIENT: Deduced my ID (only player): {}", all_player_states[0].id);
                                                    gs_lock.local_player_id = Some(all_player_states[0].id.clone());
                                                    let _ = player_id_confirmation_tx.send(all_player_states[0].id.clone());
                                                    local_id_determined = true;
                                                }
                                            } else {
                                                local_id_determined = true;
                                            }
                                        }

                                        if local_id_determined {
                                            if server_updates_tx.send(all_player_states.clone()).is_err() {
                                                eprintln!("CLIENT: Receiver for server updates dropped.");
                                                break; 
                                            }
                                        } else {
                                             println!("CLIENT: Waiting to determine local player ID from server broadcast...");
                                        }
                                    } else {
                                        eprintln!("CLIENT: Failed to parse server message into Vec<PlayerState>: {}", text);
                                    }
                                }
                                Ok(WsMessage::Close(_)) => {
                                    println!("CLIENT: WebSocket connection closed by server.");
                                    break; 
                                }
                                Err(e) => {
                                    eprintln!("CLIENT: WebSocket read error: {}", e);
                                    break; 
                                }
                                _ => { /* Ignore other message types */ }
                            }
                        }
                        else => { 
                            println!("CLIENT: WebSocket read stream ended.");
                            break;
                        }
                    }
                }
                println!("CLIENT: Receive task finished.");
            });

            tokio::select! {
                _ = send_task => {},
                _ = receive_task => {},
            }
            println!("CLIENT: WebSocket connection handler finished.");
        }
        Err(e) => {
            eprintln!("CLIENT: Failed to connect to WebSocket: {}", e);
        }
    }
}

#[tokio::main]
async fn main() {
    unsafe { SetConfigFlags(ConfigFlags::FLAG_WINDOW_RESIZABLE as u32) };
    let (mut window_x, mut window_y) = (1920, 1080);
    let (mut rl, thread) = raylib::init()
        .size(window_x, window_y)
        .title("Multiplayer Client")
        .build();

    let mut camera = Camera3D::perspective(
        Vector3 { x: -250.0, y: PLAYER_HEIGHT + 20.0, z: -250.0 },
        Vector3 { x: -100.0, y: PLAYER_HEIGHT + 12.0, z: -100.0 },
        Vector3::up(),
        45.0,
    );
    let mut yaw: f32 = 0.0;
    let mut pitch: f32 = 0.0;

    rl.set_target_fps(FPS);

    let game_state = Arc::new(TokioMutex::new(GameState {
        local_player_id: None,
        other_players: HashMap::new(),
        join_messages: VecDeque::with_capacity(MAX_JOIN_MESSAGES + 1),
    }));

    let (local_update_tx, local_update_rx) = mpsc::unbounded_channel::<PlayerState>();
    let (server_update_tx, mut server_update_rx) = mpsc::unbounded_channel::<Vec<PlayerState>>();
    let (player_id_confirmation_tx, mut player_id_confirmation_rx) = mpsc::unbounded_channel::<String>();

    let game_state_clone_ws = game_state.clone();
    tokio::spawn(connect_and_manage_websocket(
        local_update_rx,
        server_update_tx,
        player_id_confirmation_tx,
        game_state_clone_ws,
    ));

    let player_model_path = CString::new("./src/Soldier1.glb").expect("CString for player model failed");
    let mut player_model: raylib::ffi::Model;

    let noise_image: raylib::ffi::Image;
    let noise_texture: raylib::ffi::Texture2D;
    let mut terrain_mesh: raylib::ffi::Mesh;
    let mut terrain_model: raylib::ffi::Model;
    let terrain_vertices_vec: Vec<Vector3>;

    unsafe {
        player_model = LoadModel(player_model_path.as_ptr());
        if player_model.meshCount == 0 {
            eprintln!("CLIENT: Failed to load player model!");
        }

        noise_image = GenImagePerlinNoise(NOISE_SIZE.x as i32, NOISE_SIZE.y as i32, 0, 0, MAP_SCALE);
        noise_texture = LoadTextureFromImage(noise_image);
        terrain_mesh = GenMeshHeightmap(noise_image, raylib::ffi::Vector3{ x: MAP_SIZE.x, y: MAP_SCALE, z: MAP_SIZE.y });
        terrain_model = LoadModelFromMesh(terrain_mesh);
        
        if terrain_model.materialCount > 0 && !terrain_model.materials.is_null() {
             if !(*terrain_model.materials.add(0)).maps.is_null() {
                (*(*terrain_model.materials.add(0)).maps.wrapping_add(MATERIAL_MAP_ALBEDO as usize)).texture = noise_texture;
            }
        }

        if terrain_model.meshCount > 0 && !terrain_model.meshes.is_null() { // Check mesh pointer
            let raw_vertices_ptr = (*terrain_model.meshes).vertices; 
            let vertex_count = (*terrain_model.meshes).vertexCount as usize;
            if !raw_vertices_ptr.is_null() && vertex_count > 0 { // Check vertices pointer
                let vert_slice: &[f32] = std::slice::from_raw_parts(raw_vertices_ptr, vertex_count * 3);
                
                let mut temp_verts = Vec::with_capacity(vertex_count);
                for i in 0..vertex_count {
                    temp_verts.push(Vector3::new(
                        vert_slice[i * 3],
                        vert_slice[i * 3 + 1],
                        vert_slice[i * 3 + 2],
                    ));
                }
                terrain_vertices_vec = temp_verts;
            } else {
                eprintln!("CLIENT: Terrain mesh vertices are null or count is zero.");
                terrain_vertices_vec = Vec::new(); // Initialize to empty to prevent crash
            }
        } else {
            eprintln!("CLIENT: Terrain model has no meshes or mesh pointer is null.");
            terrain_vertices_vec = Vec::new(); // Initialize to empty
        }
        UnloadImage(noise_image);
    }
    let terrain_position = raylib::ffi::Vector3 {x: -MAP_SIZE.x, y: 0.0, z: -MAP_SIZE.y};
    let terrain_color_val: raylib::ffi::Color = unsafe { ColorFromHSV(130.0, 1.0, 1.0) };


    rl.disable_cursor();
    let mut player_speed = 50.0;

    while !rl.window_should_close() {
        let dt = rl.get_frame_time();

        if rl.is_cursor_on_screen() && rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
        }
        if rl.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
        }
        
        if rl.is_cursor_hidden() {
            let mouse_delta = rl.get_mouse_delta();
            let sensitivity = 0.003;
            yaw += mouse_delta.x * sensitivity;
            pitch -= mouse_delta.y * sensitivity;

            let pitch_limit = std::f32::consts::FRAC_PI_2 - 0.01;
            pitch = pitch.clamp(-pitch_limit, pitch_limit);
        }
        
        let camera_forward_vector = Vector3 {
            x: pitch.cos() * yaw.sin(),
            y: pitch.sin(),
            z: pitch.cos() * yaw.cos() * -1.0,
        };
        let movement_input = get_movement_vector(&rl, &camera, yaw);
        // player_speed += 10.0;
        let desired_position = camera.position + movement_input * player_speed * dt;
        
        let mut new_position = desired_position;
        let terrain_grid_width = NOISE_SIZE.x as usize;
        let terrain_grid_depth = NOISE_SIZE.y as usize;

        if !terrain_vertices_vec.is_empty() { // Only do collision if terrain vertices exist
            if check_collision(desired_position, terrain_position.into(), &terrain_vertices_vec, terrain_grid_width, terrain_grid_depth) {
                new_position = adjust_position(desired_position, terrain_position.into(), &terrain_vertices_vec, terrain_grid_width, terrain_grid_depth);
                if movement_input.y < 0.0 {
                     let mut horizontal_movement = movement_input;
                     horizontal_movement.y = 0.0;
                     if horizontal_movement.length() > 0.0001 {
                        Vector3::normalize(&mut horizontal_movement);
                     }
                     let corrected_horizontal_pos = camera.position + horizontal_movement * player_speed * dt;
                     new_position = adjust_position(corrected_horizontal_pos, terrain_position.into(), &terrain_vertices_vec, terrain_grid_width, terrain_grid_depth);
                }
            }
        }
        camera.position = new_position;
        camera.target = camera.position + camera_forward_vector;


        if let Ok(confirmed_id) = player_id_confirmation_rx.try_recv() {
            let mut gs = game_state.lock().await;
            if gs.local_player_id.is_none() {
                println!("MAIN_LOOP: Received and set local player ID: {}", confirmed_id);
                gs.local_player_id = Some(confirmed_id);
            }
        }

        while let Ok(all_states_update) = server_update_rx.try_recv() {
            let mut gs = game_state.lock().await;
            let mut current_other_players = HashMap::new();
            if let Some(local_id) = &gs.local_player_id.clone() {
                for state in all_states_update {
                    if &state.id != local_id {
                        if !gs.other_players.contains_key(&state.id) {
                            let join_msg = format!("Player {}... joined", state.id.chars().take(6).collect::<String>());
                            println!("CLIENT: {}", join_msg);
                            gs.join_messages.push_back(join_msg);
                            if gs.join_messages.len() > MAX_JOIN_MESSAGES {
                                gs.join_messages.pop_front();
                            }
                        }
                        current_other_players.insert(state.id.clone(), state);
                    }
                }
                gs.other_players = current_other_players;
            }
        }

        {
            let gs = game_state.lock().await;
            if let Some(local_id) = &gs.local_player_id {
                let local_player_state = PlayerState {
                    id: local_id.clone(),
                    position: (camera.position.x, camera.position.y, camera.position.z),
                    rotation: (pitch, yaw, 0.0),
                };
                let _ = local_update_tx.send(local_player_state);
            }
        }
        
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(raylib::prelude::Color::SKYBLUE);
        let mut orange_toggle: bool = false;
        
        unsafe {
	        if IsKeyDown(71) {
	            orange_toggle = !orange_toggle;
            }
        }

        {
            let mut d3 = d.begin_mode3D(camera);
            unsafe {
                DrawModel(terrain_model, terrain_position, 1.0, terrain_color_val);
            }
            // CORRECTED: Use Ok() for try_lock() result
            if let Ok(locked_gs) = game_state.try_lock() {
                for player_state in locked_gs.other_players.values() {
                    let mut pos = Vector3 {
                        x: player_state.position.0,
                        y: player_state.position.1 - PLAYER_HEIGHT,
                        z: player_state.position.2,
                    };
                    let rot_axis = Vector3::up();
                    let rot_angle_rad = player_state.rotation.1;
                    let rot_angle_deg = rot_angle_rad.to_degrees();
                    let model_scale = raylib::ffi::Vector3 { x: 50.0, y: 50.0, z: 50.0 };

                    // if player_model.meshCount > 0 {
                         unsafe {
                            // println! ("{:3.2} {:3.2} {:3.2}", pos.x, pos.y, pos.z);
                            // println! ("{:3.2} {:3.2} {:3.2}", rot_axis.x, rot_axis.y, rot_axis.z);
                            // pos = Vector3{x: rng.random_range(-50..-450) as f32, y: rng.random_range(5..20) as f32, z: rng.random_range(-50..-450) as f32 };
                            
                            DrawModelEx(player_model, pos.into(), rot_axis.into(), rot_angle_deg, model_scale, ColorFromHSV(0.0 as f32, 1.0, 0.0));
                        }
                        d3.draw_sphere(pos, 1.0, raylib::prelude::Color::RED);
                    // }
                }
            }
             d3.draw_sphere_ex(Vector3{x: 0.0, y: 150.0, z: -800.0}, 15.0, 10, 10, raylib::prelude::Color::YELLOW);
             if !orange_toggle {
                d3.draw_sphere_ex(Vector3{x: 0.0, y: 150.0, z: -800.0}, 12.0, 10, 10, raylib::prelude::Color::ORANGE);
             }
        }

        window_x = d.get_render_width();
        window_y = d.get_render_height();
        d.draw_text(&format!("Screen: {}x{}", window_x, window_y), 10, 10, 20, raylib::prelude::Color::LIME);
        d.draw_text(&format!("Pos: {:.1}, {:.1}, {:.1}", camera.position.x, camera.position.y, camera.position.z), 10, 40, 20, raylib::prelude::Color::RED);
        d.draw_fps(10, 70);

        // CORRECTED: Use Ok() for try_lock() result
        if let Ok(locked_gs) = game_state.try_lock() {
            let mut y_offset = 100;
            for msg in locked_gs.join_messages.iter() {
                d.draw_text(msg, 10, y_offset, 20, raylib::prelude::Color::YELLOW);
                y_offset += 25;
            }
        }
    }
}