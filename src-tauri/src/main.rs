// Windows release builds must not open a console behind the window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    splaunch_lib::run()
}
