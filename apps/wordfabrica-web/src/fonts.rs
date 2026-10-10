//! Install server fonts before creating the editor, so layout, font menus and exports share
//! one database from the first frame. A bounded failure retains the bundled fonts.
use wordfabrica_remote::{FontFile, MAX_FONT_BYTES, MAX_FONT_FILES, MAX_FONT_TOTAL, valid_id};

pub async fn load() -> Result<(), ()> {
    let controller = web_sys::AbortController::new().map_err(|_| ())?;
    let signal = controller.signal();
    let operation = async {
        let response = gloo_net::http::Request::get("/api/fonts").abort_signal(Some(&signal)).send().await.map_err(|_| ())?;
        if !response.ok() {
            return Err(());
        }
        let files: Vec<FontFile> = response.json().await.map_err(|_| ())?;
        if files.len() > MAX_FONT_FILES {
            return Err(());
        }
        let mut total = 0usize;
        for file in &files {
            total = total.saturating_add(file.bytes);
            if !valid_id(&file.id) || file.bytes == 0 || file.bytes > MAX_FONT_BYTES || total > MAX_FONT_TOTAL {
                return Err(());
            }
        }
        for file in files {
            let response =
                gloo_net::http::Request::get(&format!("/api/fonts/{}", file.id)).abort_signal(Some(&signal)).send().await.map_err(|_| ())?;
            if !response.ok() {
                return Err(());
            }
            let bytes = response.binary().await.map_err(|_| ())?;
            if bytes.len() != file.bytes || wordfabrica_fonts::font_names(&bytes).is_empty() {
                return Err(());
            }
            // Existing family/style pairs stay unchanged, including compiled-in fallbacks.
            wordfabrica_fonts::FontDb::global().add_font(bytes);
        }
        Ok(())
    };
    futures_util::pin_mut!(operation);
    let timeout = gloo_timers::future::TimeoutFuture::new(30_000);
    futures_util::pin_mut!(timeout);
    match futures_util::future::select(operation, timeout).await {
        futures_util::future::Either::Left((result, _)) => result,
        futures_util::future::Either::Right(_) => {
            controller.abort();
            Err(())
        }
    }
}
