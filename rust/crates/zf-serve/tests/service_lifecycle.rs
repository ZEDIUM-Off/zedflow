mod support;

#[tokio::test]
async fn exclusive_http_owner_releases_only_after_explicit_shutdown() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    let home = root.path().join("home");
    let data = root.path().join("data");
    std::fs::create_dir_all(&workspace).unwrap();
    let (router, service) =
        support::open_router(data.clone(), workspace.clone(), vec![], home.clone())
            .await
            .unwrap();
    assert!(
        support::open_router(data.clone(), workspace.clone(), vec![], home.clone())
            .await
            .is_err()
    );
    assert!(zf_storage::migration::lock(&data).is_err());
    service.shutdown().await.unwrap();
    drop(router);
    drop(service);
    let (router, service) = support::open_router(data.clone(), workspace, vec![], home)
        .await
        .unwrap();
    assert!(zf_storage::migration::lock(&data).is_err());
    service.shutdown().await.unwrap();
    drop(router);
    drop(service);
    assert!(zf_storage::migration::lock(&data).is_ok());
}
