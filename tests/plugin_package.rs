use serde_json::Value;

#[test]
fn portable_plugin_package_points_to_canonical_remote_mcp() {
    let plugin: Value = serde_json::from_str(include_str!("../plugin/plugin.json")).unwrap();
    let mcp: Value = serde_json::from_str(include_str!("../plugin/mcp.json")).unwrap();

    assert_eq!(plugin["name"], "hooktry");
    let version = plugin["version"].as_str().unwrap();
    let parts: Vec<_> = version.split('.').collect();
    assert_eq!(parts.len(), 3);
    assert!(
        parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
    );
    assert_eq!(
        plugin["extensions"]["com.openai"]["onboardingSkill"],
        "./skills/get-started/SKILL.md"
    );
    assert!(
        plugin["extensions"]["com.openai"].get("apps").is_none(),
        "Hooktry uses bundled MCP configuration and must not declare an app mapping"
    );
    assert!(
        !std::path::Path::new("plugin/.app.json").exists(),
        "Hooktry source package must not contain a registered app mapping"
    );
    assert_eq!(
        plugin["extensions"]["com.openai"]["interface"]["category"],
        "Developer Tools"
    );

    let short_description = plugin["extensions"]["com.openai"]["interface"]["shortDescription"]
        .as_str()
        .unwrap();
    assert!(short_description.chars().count() <= 30);

    let interface = &plugin["extensions"]["com.openai"]["interface"];
    assert_eq!(interface["logo"], "./assets/hooktry-mark-light.svg");
    assert_eq!(interface["logoDark"], "./assets/hooktry-mark-dark.svg");
    assert_eq!(interface["composerIcon"], "./assets/hooktry-mark-light.svg");
    assert_eq!(
        interface["composerIconDark"],
        "./assets/hooktry-mark-dark.svg"
    );

    let long_description = interface["longDescription"].as_str().unwrap();
    assert!(long_description.contains("without requiring an explicit @Hooktry tag"));

    assert_eq!(mcp["mcpServers"]["hooktry"]["type"], "streamable-http");
    assert_eq!(
        mcp["mcpServers"]["hooktry"]["url"],
        "https://mcp.hooktry.com/mcp"
    );
}

#[test]
fn remote_onboarding_skill_describes_only_the_shipped_surface() {
    let skill = include_str!("../plugin/skills/get-started/SKILL.md");

    assert!(skill.contains("name: get-started"));
    assert!(skill.contains("create_webhook_endpoint"));
    assert!(skill.contains("endpoint creation only"));
    assert!(skill.contains("a short-lived one-time browser handoff URL"));
    assert!(skill.contains("does not return the claim capability"));
    assert!(skill.contains("Open in Hooktry"));
    assert!(skill.contains("Do not label it **Handoff**"));
    assert!(skill.contains("The user does not need to mention or tag Hooktry explicitly"));
    assert!(skill.contains("final user-facing response MUST include all three"));
    assert!(skill.contains("Keep `hook_url` visible as a raw URL"));
    assert!(skill.contains("Prefer labeled Markdown links"));
}
