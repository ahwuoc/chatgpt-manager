mod account;
mod flow;
mod page;
mod token;

use anyhow::Result;
use futures::StreamExt;
use tauri::Emitter;

pub(crate) fn emit_log(app: &tauri::AppHandle, message: impl Into<String>) {
    let _ = app.emit("automation-log", message.into());
}

pub async fn run(app: tauri::AppHandle, emails: Vec<String>, threads: u32) -> Result<()> {
    let accounts = account::load_accounts(&emails)?;

    if accounts.is_empty() {
        emit_log(&app, "❌ Không có tài khoản nào được chọn để đăng nhập!");
        return Ok(());
    }

    emit_log(
        &app,
        format!(
            "🚀 Khởi chạy đăng nhập trực tiếp trên Tauri cho {} tài khoản (Đa luồng, tối đa {} tài khoản)...",
            accounts.len(),
            threads
        ),
    );

    let concurrency_limit = (threads as usize).max(1);
    let mut stream = futures::stream::iter(accounts.into_iter().enumerate())
        .map(|(index, acc)| {
            let app_clone = app.clone();
            let app_err = app.clone();
            async move {
                let email = acc.email.clone();
                if index > 0 {
                    let delay = (index % concurrency_limit) as u64 * 3;
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                }
                let slot_index = index % concurrency_limit;
                if let Err(e) = flow::run_login(app_clone, acc, slot_index, concurrency_limit).await
                {
                    emit_log(&app_err, format!("❌ Lỗi [{}]: {}", email, e));
                }
            }
        })
        .buffer_unordered(concurrency_limit);

    while stream.next().await.is_some() {}

    emit_log(&app, "\n✨ HOÀN TẤT BƯỚC 1!");
    Ok(())
}
