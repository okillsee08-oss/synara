use synara_providers::ProviderRegistry;

#[test]
fn builtins_cover_supported_provider_kinds() {
    let mut registry = ProviderRegistry::new();
    registry.register_builtins();
    let kinds: Vec<_> = registry.list().into_iter().map(|m| m.kind).collect();

    for expected in [
        "codex",
        "claudeAgent",
        "cursor",
        "devin",
        "antigravity",
        "grok",
        "droid",
        "opencode",
        "pi",
    ] {
        assert!(
            kinds.iter().any(|kind| kind == expected),
            "missing provider {expected}"
        );
    }
}
