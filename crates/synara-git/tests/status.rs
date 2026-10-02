#[test]
fn status_api_is_constructible() {
    let root = std::env::temp_dir().join(format!("synara-git-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    let repo = git2::Repository::init(&root).unwrap();
    let path = root.join("README.md");
    std::fs::write(&path, "test").unwrap();

    let changes = synara_git::status(&root).unwrap();
    assert!(changes.iter().any(|change| change.path == std::path::PathBuf::from("README.md")));

    drop(repo);
    let _ = std::fs::remove_dir_all(&root);
}
