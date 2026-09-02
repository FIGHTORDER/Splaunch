mod campaign;
mod container;
mod customkey;
mod dds;
mod game;
mod install;
mod launch;
mod mapfile;
mod maps;
mod preset;
mod savefile;
mod scenario;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(launch::Game::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            launch::sp_locate_install,
            launch::sp_launch_preview,
            launch::sp_game_info,
            launch::sp_units,
            launch::sp_unit_icons,
            launch::sp_unit_marks,
            launch::sp_map_metal,
            maps::sp_maps,
            scenario::spsc_script,
            scenario::spsc_problems,
            scenario::spsc_warnings,
            preset::spsc_import_preset,
            preset::spsc_export_preset,
            scenario::spsc_test,
            scenario::spsc_save,
            scenario::spsc_open,
            scenario::spsc_example,
            campaign::spsc_export_campaign,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Splaunch");
}
