use axum::response::IntoResponse;

pub const LLMS_TXT: &str = include_str!("../llms.txt");
pub const SKILL_MD: &str = include_str!("../skills/ortyo/SKILL.md");
const README_MD: &str = include_str!("../README.md");

pub async fn llms_txt() -> impl IntoResponse {
    ([("content-type", "text/plain; charset=utf-8")], LLMS_TXT)
}

pub async fn llms_full_txt() -> impl IntoResponse {
    let body = format!(
        "{LLMS_TXT}\n\n---\n\n# Canonical README\n\n{README_MD}\n\n---\n\n# ORTYO Agent Skill\n\n{SKILL_MD}"
    );
    ([("content-type", "text/plain; charset=utf-8")], body)
}

pub async fn skill_md() -> impl IntoResponse {
    ([("content-type", "text/markdown; charset=utf-8")], SKILL_MD)
}
