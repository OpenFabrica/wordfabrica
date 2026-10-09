use super::*;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use tower::ServiceExt;

#[tokio::test]
async fn catalog_serves_validated_bytes_and_new_catalog_gets_new_ids() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    let bytes = wordcraft_fonts::bundled()[0];
    std::fs::write(dir.path().join("nested/font.TTF"), bytes).unwrap();
    std::fs::write(dir.path().join("license.txt"), "not a font").unwrap();
    let catalog = Catalog::load(Some(dir.path())).unwrap();
    assert_eq!(catalog.len(), 1);
    let metadata = &catalog.entries[0].0;
    assert_eq!(metadata.families, ["Source Sans 3"]);
    assert_eq!(metadata.bytes, bytes.len());
    let id = metadata.id.clone();
    let store = Arc::new(crate::store::Store::open(&dir.path().join("db")).unwrap());
    let app = crate::router_with_fonts(store, dir.path().into(), Arc::new(catalog));
    let response = app.clone().oneshot(Request::get("/api/fonts").body(Body::empty()).unwrap()).await.unwrap();
    let list: Vec<FontFile> = serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(list[0].id, id);
    let response = app.clone().oneshot(Request::get(format!("/api/fonts/{id}")).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(&to_bytes(response.into_body(), MAX_FONT_BYTES).await.unwrap()[..], bytes);
    let fresh = Catalog::load(Some(dir.path())).unwrap();
    assert_ne!(fresh.entries[0].0.id, id);
    let response = routes(Arc::new(fresh)).oneshot(Request::get(format!("/api/fonts/{id}")).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let blocked = app.clone().oneshot(Request::get("/api/fonts").header("sec-fetch-site", "cross-site").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(blocked.status(), StatusCode::FORBIDDEN);
    let response = app.oneshot(Request::get("/api/fonts/license.txt").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn invalid_fonts_and_resource_limits_fail_without_publishing() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Catalog::load(None).unwrap().is_empty());
    assert!(Catalog::load(Some(dir.path())).unwrap().is_empty());
    let path = dir.path().join("broken.otf");
    std::fs::write(&path, b"not a font").unwrap();
    assert!(Catalog::load(Some(dir.path())).is_err());
    std::fs::File::create(&path).unwrap().set_len(MAX_FONT_BYTES as u64 + 1).unwrap();
    assert!(Catalog::load(Some(dir.path())).is_err());
    std::fs::remove_file(path).unwrap();
    for i in 0..=MAX_FONT_FILES {
        std::fs::write(dir.path().join(format!("{i}.ttf")), []).unwrap();
    }
    assert!(Catalog::load(Some(dir.path())).is_err());
}

#[cfg(unix)]
#[test]
fn symbolic_links_do_not_publish_files_outside_the_directory() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let path = outside.path().join("private.ttf");
    std::fs::write(&path, wordcraft_fonts::bundled()[0]).unwrap();
    std::os::unix::fs::symlink(path, dir.path().join("link.ttf")).unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("directory")).unwrap();
    assert!(Catalog::load(Some(dir.path())).unwrap().is_empty());
}
