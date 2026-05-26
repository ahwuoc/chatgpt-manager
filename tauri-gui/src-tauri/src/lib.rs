mod app_commands;
mod app_state;
mod auth;
mod automation_commands;
mod confirm_paypal_impl;
mod file_store;
pub mod js_helpers;
mod mail_otp;
mod make_payment_impl;
mod otp;
mod paths;
mod paypal_approve_impl;
mod plus_scan;
pub mod selectors;
pub mod sms_service;
mod us_browser_proxy;
pub mod utils;

use app_state::AppState;
use std::sync::{Arc, Mutex};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            running_task: Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            app_commands::get_initial_data,
            app_commands::get_stats,
            app_commands::save_file_content,
            app_commands::save_settings,
            app_commands::cleanup_chrome_profiles,
            app_commands::open_folder,
            app_commands::open_account_browser,
            automation_commands::start_automation,
            automation_commands::stop_automation,
            plus_scan::scan_plus_status,
            plus_scan::scan_plus_mail_status,
            mail_otp::get_otp,
            sms_service::get_sms_config,
            sms_service::save_sms_config,
            us_browser_proxy::get_us_browser_proxy_config,
            us_browser_proxy::save_us_browser_proxy_config,
            us_browser_proxy::get_us_browser_proxy_status,
            us_browser_proxy::change_us_browser_proxy_ip
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
