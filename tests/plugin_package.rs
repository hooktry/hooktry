use serde_json::Value;

#[test]
fn portable_plugin_package_points_to_canonical_remote_mcp() {
    let plugin: Value = serde_json::from_str(include_str!("../plugin/plugin.json")).unwrap();
    let mcp: Value = serde_json::from_str(include_str!("../plugin/mcp.json")).unwrap();

    assert_eq!(plugin["name"], "hooktry");
    assert_eq!(plugin["version"], "0.1.0");
    assert_eq!(
        plugin["extensions"]["com.openai"]["onboardingSkill"],
        "./skills/get-started/SKILL.md"
    );
    assert_eq!(
        plugin["extensions"]["com.openai"]["interface"]["category"],
        "Developer Tools"
    );

    let short_description = plugin["extensions"]["com.openai"]["interface"]["shortDescription"]
        .as_str()
        .unwrap();
    assert!(short_description.chars().count() <= 30);

    assert_eq!(
        mcp["mcpServers"]["hooktry"]["type"],
        "streamable-http"
    );
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
    assert!(skill.contains("does not return Hooktry viewer"));
}
