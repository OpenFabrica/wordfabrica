//! Format crates registered with the I/O dispatcher.

use wordfabrica_doc::Document;

/// Open a format other than plain text / JSON.
pub fn open(ext: &str, bytes: &[u8]) -> Option<Result<Document, String>> {
    match ext {
        "docx" | "docm" | "dotx" | "dotm" => Some(wordfabrica_docx::read(bytes).map_err(|e| e.to_string())),
        other => wordfabrica_formats::import(other, bytes),
    }
}

/// Save to a format other than plain text / JSON.
pub fn save(ext: &str, doc: &Document) -> Option<Result<Vec<u8>, String>> {
    match ext {
        "png" => Some(render_png(doc, 0, 2.0)),
        "docx" | "docm" | "dotx" => Some(wordfabrica_docx::write(doc).map_err(|e| e.to_string())),
        "pdf" => Some(wordfabrica_pdf::export(doc, &Default::default()).map_err(|e| e.to_string())),
        other => wordfabrica_formats::export(other, doc),
    }
}

/// Render page `page` as PNG at `scale` px/pt.
pub fn render_png(doc: &Document, page: usize, scale: f32) -> Result<Vec<u8>, String> {
    let l = wordfabrica_layout::layout(doc, &mut wordfabrica_layout::LayoutCache::new(), &Default::default());
    let p = l.pages.get(page).ok_or_else(|| format!("no page {}", page + 1))?;
    let img = wordfabrica_render::render_page(doc, p, scale, &Default::default());
    Ok(img.to_png())
}
