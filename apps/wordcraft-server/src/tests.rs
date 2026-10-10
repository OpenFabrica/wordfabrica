use crate::{
    router,
    store::{Error, Store},
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use std::sync::Arc;
use tower::ServiceExt;
use wordcraft_doc::{Document, StoryRef};
use wordcraft_remote::{Metadata, Snapshot, WriteRequest};

const ID: &str = "12345678-1234-1234-1234-123456789abc";
fn write(revision: u64, text: &str) -> WriteRequest {
    WriteRequest { revision, title: "测试文档".into(), document: Document::from_text(text), commit: false }
}

#[test]
fn durable_snapshots_and_conflicts_preserve_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite3");
    {
        let store = Store::open(&path).unwrap();
        assert_eq!(store.put(ID, write(0, "first")).unwrap().revision, 1);
        assert!(matches!(store.put(ID, write(0, "stale")), Err(Error::Conflict)));
        assert!(matches!(store.put(ID, write(8, "future")), Err(Error::Conflict)));
        assert_eq!(store.get(ID).unwrap().document.plain_text(StoryRef::Body), "first");
        assert_eq!(store.put(ID, write(1, "saved 中文")).unwrap().revision, 2);
        assert!(matches!(store.put("../../file", write(0, "attack")), Err(Error::Invalid)));
        let mut invalid = write(2, "must not replace");
        invalid.title = "\n".into();
        assert!(matches!(store.put(ID, invalid), Err(Error::Invalid)));
    }
    let reopened = Store::open(&path).unwrap();
    let saved = reopened.get(ID).unwrap();
    assert_eq!(saved.metadata.revision, 2);
    assert_eq!(saved.document.plain_text(StoryRef::Body), "saved 中文");
    assert_eq!(reopened.list().unwrap().len(), 1);
}

#[test]
fn two_connections_cannot_overwrite_the_same_revision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite3");
    let first = Store::open(&path).unwrap();
    first.put(ID, write(0, "base")).unwrap();
    let second = Store::open(&path).unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let other = barrier.clone();
    let worker = std::thread::spawn(move || {
        other.wait();
        second.put(ID, write(1, "second"))
    });
    barrier.wait();
    let result = first.put(ID, write(1, "first"));
    let competing = worker.join().unwrap();
    assert_ne!(result.is_ok(), competing.is_ok());
    assert_eq!(first.get(ID).unwrap().metadata.revision, 2);
}

#[tokio::test]
async fn http_create_load_list_conflict_and_input_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let app = router(Arc::new(Store::open(&dir.path().join("db")).unwrap()), dir.path().into());
    let request = || {
        Request::put(format!("/api/documents/{ID}"))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&write(0, "browser content")).unwrap()))
            .unwrap()
    };
    let response = app.clone().oneshot(request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let metadata: Metadata = serde_json::from_slice(&to_bytes(response.into_body(), 1024).await.unwrap()).unwrap();
    assert_eq!(metadata.revision, 1);
    assert_eq!(app.clone().oneshot(request()).await.unwrap().status(), StatusCode::OK);
    let stale = Request::put(format!("/api/documents/{ID}"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&write(0, "stale version")).unwrap()))
        .unwrap();
    assert_eq!(app.clone().oneshot(stale).await.unwrap().status(), StatusCode::CONFLICT);
    let response = app.clone().oneshot(Request::get(format!("/api/documents/{ID}")).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let snapshot: Snapshot = serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap();
    assert_eq!(snapshot.document.plain_text(StoryRef::Body), "browser content");
    let response = app.clone().oneshot(Request::get("/api/documents").body(Body::empty()).unwrap()).await.unwrap();
    let list: Vec<Metadata> = serde_json::from_slice(&to_bytes(response.into_body(), 1024).await.unwrap()).unwrap();
    assert_eq!(list.len(), 1);
    let response = app
        .clone()
        .oneshot(Request::put(format!("/api/documents/{ID}")).header("content-type", "application/json").body(Body::from("{bad")).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response =
        app.clone().oneshot(Request::get("/api/documents/00000000-0000-0000-0000-000000000000").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = app
        .clone()
        .oneshot(
            Request::put(format!("/api/documents/{ID}"))
                .header("content-type", "application/json")
                .body(Body::from(vec![b' '; wordcraft_remote::MAX_BYTES + 1]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let mut cross_origin = request();
    cross_origin.headers_mut().insert("origin", "https://unrelated.example".parse().unwrap());
    cross_origin.headers_mut().insert("host", "localhost:8771".parse().unwrap());
    assert_eq!(app.clone().oneshot(cross_origin).await.unwrap().status(), StatusCode::FORBIDDEN);
    let response = app.oneshot(Request::get("/api/documents").header("sec-fetch-site", "cross-site").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

const TAGS: &str =
    r#"{"id":"tags","name":"模版标签","kind":"tags","categories":[{"name":"客户","tags":[{"label":"客户名称","text":"{{客户名称}}"}]}]}"#;

#[test]
fn plugin_catalog_rejects_bad_files_at_startup() {
    use crate::plugins::Catalog;
    assert!(Catalog::load(None).unwrap().is_empty());
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("b.json"), TAGS).unwrap();
    std::fs::write(dir.path().join("readme.txt"), "not a plugin").unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    std::fs::write(dir.path().join("nested/ignored.json"), "{bad").unwrap();
    assert_eq!(Catalog::load(Some(dir.path())).unwrap().len(), 1);
    std::fs::write(dir.path().join("a.json"), TAGS).unwrap();
    assert!(Catalog::load(Some(dir.path())).is_err(), "duplicate ids");
    std::fs::write(dir.path().join("a.json"), TAGS.replace(r#""id":"tags""#, r#""id":"../x""#)).unwrap();
    assert!(Catalog::load(Some(dir.path())).is_err(), "invalid id");
    std::fs::write(dir.path().join("a.json"), "{bad").unwrap();
    assert!(Catalog::load(Some(dir.path())).is_err(), "malformed JSON");
    std::fs::write(dir.path().join("a.json"), vec![b' '; wordcraft_remote::MAX_PLUGIN_BYTES + 1]).unwrap();
    assert!(Catalog::load(Some(dir.path())).is_err(), "oversized");
    assert!(Catalog::load(Some(&dir.path().join("missing"))).is_err(), "a configured directory must exist");
}

#[tokio::test]
async fn http_lists_plugins() {
    let dir = tempfile::tempdir().unwrap();
    let plugins = dir.path().join("plugins");
    std::fs::create_dir(&plugins).unwrap();
    std::fs::write(plugins.join("tags.json"), TAGS).unwrap();
    let app = crate::router_with_catalogs(
        Arc::new(Store::open(&dir.path().join("db")).unwrap()),
        dir.path().into(),
        Arc::new(crate::fonts::Catalog::default()),
        Arc::new(crate::plugins::Catalog::load(Some(&plugins)).unwrap()),
    );
    let response = app.clone().oneshot(Request::get("/api/plugins").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let list: Vec<wordcraft_remote::Plugin> = serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "模版标签");
    // The default router publishes none.
    let app = router(Arc::new(Store::open(&dir.path().join("db2")).unwrap()), dir.path().into());
    let response = app.oneshot(Request::get("/api/plugins").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(&to_bytes(response.into_body(), 1024).await.unwrap()[..], b"[]");
}

#[tokio::test]
async fn http_saves_editable_tags_with_revisions() {
    let dir = tempfile::tempdir().unwrap();
    let plugins = dir.path().join("plugins");
    std::fs::create_dir(&plugins).unwrap();
    std::fs::write(plugins.join("a.json"), TAGS.replace(r#""id":"tags""#, r#""id":"tags","editable":true,"revision":7"#)).unwrap();
    std::fs::write(plugins.join("b.json"), TAGS.replace(r#""id":"tags""#, r#""id":"fixed""#)).unwrap();
    let db = dir.path().join("db");
    let app = |store: Store| {
        crate::router_with_catalogs(
            Arc::new(store),
            dir.path().into(),
            Arc::new(crate::fonts::Catalog::default()),
            Arc::new(crate::plugins::Catalog::load(Some(&plugins)).unwrap()),
        )
    };
    let put = |id: &str, body: String| {
        Request::put(format!("/api/plugins/{id}/tags")).header("content-type", "application/json").body(Body::from(body)).unwrap()
    };
    let list = |app: axum::Router| async move {
        let response = app.oneshot(Request::get("/api/plugins").body(Body::empty()).unwrap()).await.unwrap();
        serde_json::from_slice::<Vec<wordcraft_remote::Plugin>>(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
    };
    let router = app(Store::open(&db).unwrap());
    assert_eq!(list(router.clone()).await[0].revision, 0, "a revision in the file is ignored");
    let edit = r#"{"revision":0,"categories":[{"name":"新分类","tags":[{"label":"新标签","text":"{{新}}"}]}]}"#;
    let response = router.clone().oneshot(put("tags", edit.into())).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let saved: wordcraft_remote::Plugin = serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(router.clone().oneshot(put("tags", edit.into())).await.unwrap().status(), StatusCode::CONFLICT, "stale revision");
    assert_eq!(router.clone().oneshot(put("fixed", edit.into())).await.unwrap().status(), StatusCode::NOT_FOUND, "read-only plugin");
    assert_eq!(router.clone().oneshot(put("nope", edit.into())).await.unwrap().status(), StatusCode::NOT_FOUND);
    let bad = edit.replace(r#""revision":0"#, r#""revision":1"#).replace("{{新}}", "a\\nb");
    assert_eq!(router.clone().oneshot(put("tags", bad)).await.unwrap().status(), StatusCode::BAD_REQUEST, "invalid tags");
    let cleared = r#"{"revision":1,"categories":[]}"#;
    assert_eq!(router.clone().oneshot(put("tags", cleared.into())).await.unwrap().status(), StatusCode::OK);
    drop(router);
    // Saved tags survive a restart and win over the file; the read-only plugin keeps the file's.
    let listed = list(app(Store::open(&db).unwrap())).await;
    assert_eq!(listed[0].revision, 2);
    assert_eq!(listed[0].kind, wordcraft_remote::PluginKind::Tags { categories: vec![] });
    let wordcraft_remote::PluginKind::Tags { categories } = &listed[1].kind;
    assert_eq!(categories[0].tags[0].label, "客户名称");
}

#[tokio::test]
async fn links_from_other_sites_open_the_editor_but_not_the_api() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "<!doctype html>").unwrap();
    let app = router(Arc::new(Store::open(&dir.path().join("db")).unwrap()), dir.path().into());
    let get = |path: &str, mode: &str| {
        Request::get(path).header("sec-fetch-site", "cross-site").header("sec-fetch-mode", mode).body(Body::empty()).unwrap()
    };
    assert_eq!(app.clone().oneshot(get("/", "navigate")).await.unwrap().status(), StatusCode::OK);
    assert_eq!(app.clone().oneshot(get("/?doc=x", "navigate")).await.unwrap().status(), StatusCode::OK);
    assert_eq!(app.clone().oneshot(get("/", "no-cors")).await.unwrap().status(), StatusCode::FORBIDDEN, "subresource loads");
    assert_eq!(app.clone().oneshot(get("/api/documents", "navigate")).await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(app.clone().oneshot(get("/api/plugins", "cors")).await.unwrap().status(), StatusCode::FORBIDDEN);
    let post = Request::post("/").header("sec-fetch-site", "cross-site").header("sec-fetch-mode", "navigate").body(Body::empty()).unwrap();
    assert_eq!(app.oneshot(post).await.unwrap().status(), StatusCode::FORBIDDEN, "form posts");
}
