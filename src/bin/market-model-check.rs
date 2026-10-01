use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

const MARKET_DIR: &str = "docs/market";
const MATRIX_STATES: &[&str] = &["present", "partial", "absent", "unknown"];
const DISPOSITIONS: &[&str] = &[
    "must",
    "should",
    "differentiation",
    "watch",
    "out_of_scope",
    "covered",
];
const ORTYO_STATUSES: &[&str] = &["implemented", "partial", "absent", "unknown", "planned"];
const SIGNAL_CLASSES: &[&str] = &["direct_demand", "demand_proxy", "market_motion"];
const DIRECT_DEMAND_DIRECTIONS: &[&str] = &["supports", "contradicts", "mixed"];
const NEXT_EVIDENCE: &[&str] = &[
    "market_research",
    "first_party_usage",
    "implementation",
    "watch",
];
const VECTOR_TYPES: &[&str] = &["depth", "adjacent", "option"];

#[derive(Debug, Default)]
struct ValidationReport {
    errors: Vec<String>,
    observations: usize,
    matrix_cells: usize,
}

#[derive(Debug, Clone, Default)]
struct Capability {
    id: String,
    disposition: String,
    ortyo_status: String,
}

#[derive(Debug, Default)]
struct ScopeModel {
    cohorts: HashSet<String>,
    products: HashMap<String, Vec<String>>,
}

#[derive(Debug, Default)]
struct MatrixModel {
    checked_at: String,
    products: Vec<String>,
    rows: HashMap<String, HashMap<String, String>>,
}

#[derive(Debug, Clone, Default)]
struct Observation {
    id: String,
    product: String,
    capability: String,
    state: String,
    observed_at: String,
    applicability: String,
    freshness: String,
    confidence: String,
    assertion: String,
    source_url: String,
}

#[derive(Debug, Clone, Default)]
struct Signal {
    id: String,
    capability: String,
    class: String,
    direction: String,
}

#[derive(Debug, Clone, Default)]
struct Priority {
    id: String,
    decision: String,
    next_evidence: String,
    capabilities: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct Scenario {
    id: String,
    capabilities: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct Vector {
    id: String,
    kind: String,
    from_cohort: String,
    target_cohorts: Vec<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if !(args.is_empty() || args == ["--check"]) {
        eprintln!("usage: market-model-check [--check]");
        return ExitCode::from(2);
    }

    match validate_repository(Path::new(".")) {
        Ok(report) if report.errors.is_empty() => {
            println!(
                "market model OK: {} observations, {} matrix cells",
                report.observations, report.matrix_cells
            );
            ExitCode::SUCCESS
        }
        Ok(report) => {
            eprintln!(
                "market model invalid: {} error(s), {} observations, {} matrix cells",
                report.errors.len(),
                report.observations,
                report.matrix_cells
            );
            for error in report.errors {
                eprintln!("- {error}");
            }
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("market model check failed: {error}");
            ExitCode::from(2)
        }
    }
}

fn validate_repository(root: &Path) -> Result<ValidationReport, String> {
    let market = root.join(MARKET_DIR);
    let capabilities = parse_capabilities(&read(&market.join("capabilities.yaml"))?);
    let scope = parse_scope(&read(&market.join("scope.yaml"))?);
    let matrix = parse_matrix(&read(&market.join("matrix.yaml"))?);
    let observations = load_observations(&market.join("observations"))?;
    let signals = parse_signals(&read(&market.join("signals.yaml"))?);
    let priorities = parse_priorities(&read(&market.join("priorities.yaml"))?);
    let scenarios = parse_scenarios(&read(&market.join("scenarios.yaml"))?);
    let vectors = parse_vectors(&read(&market.join("vectors.yaml"))?);
    let competitors = parse_competitor_ids(&read(&market.join("competitors.yaml"))?);

    let mut report = ValidationReport {
        observations: observations.len(),
        matrix_cells: matrix.rows.values().map(HashMap::len).sum(),
        ..ValidationReport::default()
    };

    let capability_map = validate_capabilities(&capabilities, &mut report.errors);
    validate_scope(&scope, &mut report.errors);
    validate_matrix(
        &matrix,
        &scope,
        &competitors,
        &capability_map,
        &observations,
        &mut report.errors,
    );
    validate_observations(
        &observations,
        &scope,
        &capability_map,
        &matrix.checked_at,
        &mut report.errors,
    );
    validate_signals(&signals, &capability_map, &mut report.errors);
    validate_priorities(&priorities, &capability_map, &mut report.errors);
    validate_scenarios(&scenarios, &capability_map, &mut report.errors);
    validate_vectors(&vectors, &scope, &mut report.errors);

    Ok(report)
}

fn validate_capabilities<'a>(
    capabilities: &'a [Capability],
    errors: &mut Vec<String>,
) -> HashMap<&'a str, &'a Capability> {
    let mut map = HashMap::new();
    for capability in capabilities {
        if capability.id.is_empty() {
            errors.push("capabilities.yaml contains a capability without id".to_owned());
            continue;
        }
        if map.insert(capability.id.as_str(), capability).is_some() {
            errors.push(format!("duplicate capability id: {}", capability.id));
        }
        if !DISPOSITIONS.contains(&capability.disposition.as_str()) {
            errors.push(format!(
                "capability {} has invalid disposition: {}",
                capability.id, capability.disposition
            ));
        }
        if !ORTYO_STATUSES.contains(&capability.ortyo_status.as_str()) {
            errors.push(format!(
                "capability {} has invalid ortyo_status: {}",
                capability.id, capability.ortyo_status
            ));
        }
    }
    map
}

fn validate_scope(scope: &ScopeModel, errors: &mut Vec<String>) {
    for (product, cohorts) in &scope.products {
        for cohort in cohorts {
            if !scope.cohorts.contains(cohort) {
                errors.push(format!(
                    "scope product {product} references unknown cohort: {cohort}"
                ));
            }
        }
    }
}

fn validate_matrix(
    matrix: &MatrixModel,
    scope: &ScopeModel,
    competitors: &HashSet<String>,
    capabilities: &HashMap<&str, &Capability>,
    observations: &[Observation],
    errors: &mut Vec<String>,
) {
    if matrix.checked_at.is_empty() {
        errors.push("matrix.yaml is missing checked_at".to_owned());
    }

    let mut declared = HashSet::new();
    for product in &matrix.products {
        if !declared.insert(product.as_str()) {
            errors.push(format!("matrix.yaml declares duplicate product: {product}"));
        }
        if !scope.products.contains_key(product) {
            errors.push(format!(
                "matrix.yaml references product outside scope: {product}"
            ));
        }
        if product != "ortyo" && !competitors.contains(product) {
            errors.push(format!(
                "matrix product {product} has no competitors.yaml entry"
            ));
        }
    }

    let evidence: HashSet<(&str, &str, &str)> = observations
        .iter()
        .filter(|observation| {
            observation.applicability == "current" && observation.freshness != "stale"
        })
        .map(|observation| {
            (
                observation.product.as_str(),
                observation.capability.as_str(),
                observation.state.as_str(),
            )
        })
        .collect();

    for (capability_id, cells) in &matrix.rows {
        let Some(capability) = capabilities.get(capability_id.as_str()) else {
            errors.push(format!(
                "matrix.yaml references unknown capability: {capability_id}"
            ));
            continue;
        };

        for product in &matrix.products {
            if !cells.contains_key(product) {
                errors.push(format!(
                    "matrix row {capability_id} is missing declared product: {product}"
                ));
            }
        }

        for (product, state) in cells {
            if !declared.contains(product.as_str()) {
                errors.push(format!(
                    "matrix row {capability_id} contains undeclared product: {product}"
                ));
            }
            if !MATRIX_STATES.contains(&state.as_str()) {
                errors.push(format!(
                    "matrix cell {product}/{capability_id} has invalid state: {state}"
                ));
                continue;
            }

            if product == "ortyo" {
                let expected = match capability.ortyo_status.as_str() {
                    "implemented" => "present",
                    "partial" => "partial",
                    "absent" => "absent",
                    "unknown" | "planned" => "unknown",
                    _ => continue,
                };
                if state != expected {
                    errors.push(format!(
                        "matrix cell ortyo/{capability_id} is {state}, but capabilities.yaml implies {expected}"
                    ));
                }
            } else if state != "unknown"
                && !evidence.contains(&(product.as_str(), capability_id.as_str(), state.as_str()))
            {
                errors.push(format!(
                    "matrix cell {product}/{capability_id}={state} has no current atomic observation with the same state"
                ));
            }
        }
    }
}

fn validate_observations(
    observations: &[Observation],
    scope: &ScopeModel,
    capabilities: &HashMap<&str, &Capability>,
    checked_at: &str,
    errors: &mut Vec<String>,
) {
    let mut ids = HashSet::new();
    for observation in observations {
        if observation.id.is_empty() {
            errors.push("observation without id".to_owned());
        } else if !ids.insert(observation.id.as_str()) {
            errors.push(format!("duplicate observation id: {}", observation.id));
        }
        if !scope.products.contains_key(&observation.product) {
            errors.push(format!(
                "observation {} references unknown product: {}",
                observation.id, observation.product
            ));
        }
        if !capabilities.contains_key(observation.capability.as_str()) {
            errors.push(format!(
                "observation {} references unknown capability: {}",
                observation.id, observation.capability
            ));
        }
        if !MATRIX_STATES.contains(&observation.state.as_str()) || observation.state == "unknown" {
            errors.push(format!(
                "observation {} has invalid evidentiary state: {}",
                observation.id, observation.state
            ));
        }
        if observation.observed_at.is_empty() {
            errors.push(format!(
                "observation {} is missing observed_at",
                observation.id
            ));
        } else if !checked_at.is_empty() && observation.observed_at.as_str() > checked_at {
            errors.push(format!(
                "observation {} is dated after matrix checked_at: {} > {}",
                observation.id, observation.observed_at, checked_at
            ));
        }
        if observation.applicability != "current" {
            errors.push(format!(
                "observation {} is not current: applicability={}",
                observation.id, observation.applicability
            ));
        }
        if observation.freshness.is_empty() || observation.freshness == "stale" {
            errors.push(format!(
                "observation {} is missing fresh evidence classification",
                observation.id
            ));
        }
        if observation.confidence.is_empty() {
            errors.push(format!(
                "observation {} is missing confidence",
                observation.id
            ));
        }
        if observation.assertion.is_empty() {
            errors.push(format!(
                "observation {} is missing assertion",
                observation.id
            ));
        }
        if observation.source_url.is_empty() {
            errors.push(format!(
                "observation {} is missing source URL",
                observation.id
            ));
        }
    }
}

fn validate_signals(
    signals: &[Signal],
    capabilities: &HashMap<&str, &Capability>,
    errors: &mut Vec<String>,
) {
    let mut ids = HashSet::new();
    for signal in signals {
        if !ids.insert(signal.id.as_str()) {
            errors.push(format!("duplicate signal id: {}", signal.id));
        }
        if !capabilities.contains_key(signal.capability.as_str()) {
            errors.push(format!(
                "signal {} references unknown capability: {}",
                signal.id, signal.capability
            ));
        }
        if !SIGNAL_CLASSES.contains(&signal.class.as_str()) {
            errors.push(format!(
                "signal {} has invalid class: {}",
                signal.id, signal.class
            ));
        }
        if signal.class == "direct_demand"
            && !DIRECT_DEMAND_DIRECTIONS.contains(&signal.direction.as_str())
        {
            errors.push(format!(
                "direct-demand signal {} has invalid direction: {}",
                signal.id, signal.direction
            ));
        }
    }
}

fn validate_priorities(
    priorities: &[Priority],
    capabilities: &HashMap<&str, &Capability>,
    errors: &mut Vec<String>,
) {
    let mut ids = HashSet::new();
    for priority in priorities {
        if !ids.insert(priority.id.as_str()) {
            errors.push(format!("duplicate priority id: {}", priority.id));
        }
        if !DISPOSITIONS.contains(&priority.decision.as_str()) {
            errors.push(format!(
                "priority {} has invalid decision: {}",
                priority.id, priority.decision
            ));
        }
        if !priority.next_evidence.is_empty()
            && !NEXT_EVIDENCE.contains(&priority.next_evidence.as_str())
        {
            errors.push(format!(
                "priority {} has invalid next_evidence: {}",
                priority.id, priority.next_evidence
            ));
        }
        for capability in &priority.capabilities {
            if !capabilities.contains_key(capability.as_str()) {
                errors.push(format!(
                    "priority {} references unknown capability: {capability}",
                    priority.id
                ));
            }
        }
    }
}

fn validate_scenarios(
    scenarios: &[Scenario],
    capabilities: &HashMap<&str, &Capability>,
    errors: &mut Vec<String>,
) {
    let mut ids = HashSet::new();
    for scenario in scenarios {
        if !ids.insert(scenario.id.as_str()) {
            errors.push(format!("duplicate scenario id: {}", scenario.id));
        }
        for capability in &scenario.capabilities {
            if !capabilities.contains_key(capability.as_str()) {
                errors.push(format!(
                    "scenario {} references unknown capability: {capability}",
                    scenario.id
                ));
            }
        }
    }
}

fn validate_vectors(vectors: &[Vector], scope: &ScopeModel, errors: &mut Vec<String>) {
    let mut ids = HashSet::new();
    for vector in vectors {
        if !ids.insert(vector.id.as_str()) {
            errors.push(format!("duplicate vector id: {}", vector.id));
        }
        if !VECTOR_TYPES.contains(&vector.kind.as_str()) {
            errors.push(format!(
                "vector {} has invalid type: {}",
                vector.id, vector.kind
            ));
        }
        if !scope.cohorts.contains(&vector.from_cohort) {
            errors.push(format!(
                "vector {} references unknown from_cohort: {}",
                vector.id, vector.from_cohort
            ));
        }
        for cohort in &vector.target_cohorts {
            if !scope.cohorts.contains(cohort) {
                errors.push(format!(
                    "vector {} references unknown target cohort: {cohort}",
                    vector.id
                ));
            }
        }
    }
}

fn parse_capabilities(text: &str) -> Vec<Capability> {
    let mut items = Vec::new();
    let mut current: Option<Capability> = None;

    for line in text.lines() {
        if let Some(value) = line.strip_prefix("  - id: ") {
            if let Some(item) = current.take() {
                items.push(item);
            }
            current = Some(Capability {
                id: scalar(value),
                ..Capability::default()
            });
        } else if let Some(item) = current.as_mut() {
            if let Some(value) = line.strip_prefix("    disposition: ") {
                item.disposition = scalar(value);
            } else if let Some(value) = line.strip_prefix("    ortyo_status: ") {
                item.ortyo_status = scalar(value);
            }
        }
    }

    if let Some(item) = current {
        items.push(item);
    }
    items
}

fn parse_scope(text: &str) -> ScopeModel {
    let mut scope = ScopeModel::default();
    let mut section = "";
    let mut current_product = String::new();

    for line in text.lines() {
        if !line.starts_with(' ') && line.ends_with(':') {
            section = line.trim_end_matches(':');
            current_product.clear();
            continue;
        }

        if section == "cohorts" {
            if let Some(value) = line.strip_prefix("  - id: ") {
                scope.cohorts.insert(scalar(value));
            }
        } else if section == "products" {
            if let Some(value) = line.strip_prefix("  - id: ") {
                current_product = scalar(value);
                scope.products.entry(current_product.clone()).or_default();
            } else if let Some(value) = line.strip_prefix("    cohorts: ")
                && !current_product.is_empty()
            {
                scope
                    .products
                    .insert(current_product.clone(), inline_list(value));
            }
        }
    }

    scope
}

fn parse_matrix(text: &str) -> MatrixModel {
    let mut model = MatrixModel::default();
    let mut section = "";
    let mut current_capability = String::new();

    for line in text.lines() {
        if let Some(value) = line.strip_prefix("checked_at: ") {
            model.checked_at = scalar(value);
            continue;
        }
        if !line.starts_with(' ') && line.ends_with(':') {
            section = line.trim_end_matches(':');
            current_capability.clear();
            continue;
        }

        if section == "products" {
            if let Some(value) = line.strip_prefix("  - ") {
                model.products.push(scalar(value));
            }
        } else if section == "matrix" {
            if line.starts_with("  ") && !line.starts_with("    ") && line.ends_with(':') {
                current_capability = scalar(line.trim().trim_end_matches(':'));
                model.rows.entry(current_capability.clone()).or_default();
            } else if line.starts_with("    ")
                && !current_capability.is_empty()
                && let Some((product, state)) = key_value(line.trim())
            {
                model
                    .rows
                    .entry(current_capability.clone())
                    .or_default()
                    .insert(product, state);
            }
        }
    }

    model
}

fn load_observations(dir: &Path) -> Result<Vec<Observation>, String> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|error| format!("read {}: {error}", dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("yaml" | "yml")
            )
        })
        .collect();
    paths.sort();

    let mut observations = Vec::new();
    for path in paths {
        observations.extend(parse_observations(&read(&path)?));
    }
    Ok(observations)
}

fn parse_observations(text: &str) -> Vec<Observation> {
    let mut items = Vec::new();
    let mut current: Option<Observation> = None;

    for line in text.lines() {
        if let Some(value) = line.strip_prefix("  - id: ") {
            if let Some(item) = current.take() {
                items.push(item);
            }
            current = Some(Observation {
                id: scalar(value),
                ..Observation::default()
            });
            continue;
        }

        let Some(item) = current.as_mut() else {
            continue;
        };
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("product: ") {
            item.product = scalar(value);
        } else if let Some(value) = trimmed.strip_prefix("capability: ") {
            item.capability = scalar(value);
        } else if let Some(value) = trimmed.strip_prefix("state: ") {
            item.state = scalar(value);
        } else if let Some(value) = trimmed.strip_prefix("observed_at: ") {
            item.observed_at = scalar(value);
        } else if let Some(value) = trimmed.strip_prefix("applicability: ") {
            item.applicability = scalar(value);
        } else if let Some(value) = trimmed.strip_prefix("freshness: ") {
            item.freshness = scalar(value);
        } else if let Some(value) = trimmed.strip_prefix("confidence: ") {
            item.confidence = scalar(value);
        } else if let Some(value) = trimmed.strip_prefix("assertion: ") {
            item.assertion = scalar(value);
        } else if (trimmed.starts_with("source:") || trimmed.starts_with("url:"))
            && let Some(url) = http_token(trimmed)
        {
            item.source_url = url;
        }
    }

    if let Some(item) = current {
        items.push(item);
    }
    items
}

fn parse_signals(text: &str) -> Vec<Signal> {
    let mut items = Vec::new();
    let mut current: Option<Signal> = None;

    for line in text.lines() {
        if let Some(value) = line.strip_prefix("  - id: ") {
            if let Some(item) = current.take() {
                items.push(item);
            }
            current = Some(Signal {
                id: scalar(value),
                ..Signal::default()
            });
        } else if let Some(item) = current.as_mut() {
            let trimmed = line.trim();
            if let Some(value) = trimmed.strip_prefix("capability: ") {
                item.capability = scalar(value);
            } else if let Some(value) = trimmed.strip_prefix("class: ") {
                item.class = scalar(value);
            } else if let Some(value) = trimmed.strip_prefix("direction: ") {
                item.direction = scalar(value);
            }
        }
    }

    if let Some(item) = current {
        items.push(item);
    }
    items
}

fn parse_priorities(text: &str) -> Vec<Priority> {
    let mut items = Vec::new();
    let mut current: Option<Priority> = None;

    for line in text.lines() {
        if let Some(value) = line.strip_prefix("  - id: ") {
            if let Some(item) = current.take() {
                items.push(item);
            }
            current = Some(Priority {
                id: scalar(value),
                ..Priority::default()
            });
        } else if let Some(item) = current.as_mut() {
            let trimmed = line.trim();
            if let Some(value) = trimmed.strip_prefix("decision: ") {
                item.decision = scalar(value);
            } else if let Some(value) = trimmed.strip_prefix("next_evidence: ") {
                item.next_evidence = scalar(value);
            } else if let Some(value) = trimmed.strip_prefix("capabilities: ") {
                item.capabilities = inline_list(value);
            }
        }
    }

    if let Some(item) = current {
        items.push(item);
    }
    items
}

fn parse_scenarios(text: &str) -> Vec<Scenario> {
    let mut items = Vec::new();
    let mut current: Option<Scenario> = None;

    for line in text.lines() {
        if let Some(value) = line.strip_prefix("  - id: ") {
            if let Some(item) = current.take() {
                items.push(item);
            }
            current = Some(Scenario {
                id: scalar(value),
                ..Scenario::default()
            });
        } else if let Some(item) = current.as_mut()
            && let Some(value) = line.trim().strip_prefix("capabilities: ")
        {
            item.capabilities = inline_list(value);
        }
    }

    if let Some(item) = current {
        items.push(item);
    }
    items
}

fn parse_vectors(text: &str) -> Vec<Vector> {
    let mut items = Vec::new();
    let mut current: Option<Vector> = None;

    for line in text.lines() {
        if let Some(value) = line.strip_prefix("  - id: ") {
            if let Some(item) = current.take() {
                items.push(item);
            }
            current = Some(Vector {
                id: scalar(value),
                ..Vector::default()
            });
        } else if let Some(item) = current.as_mut() {
            let trimmed = line.trim();
            if let Some(value) = trimmed.strip_prefix("type: ") {
                item.kind = scalar(value);
            } else if let Some(value) = trimmed.strip_prefix("from_cohort: ") {
                item.from_cohort = scalar(value);
            } else if let Some(value) = trimmed.strip_prefix("target_cohorts: ") {
                item.target_cohorts = inline_list(value);
            }
        }
    }

    if let Some(item) = current {
        items.push(item);
    }
    items
}

fn parse_competitor_ids(text: &str) -> HashSet<String> {
    let mut ids = HashSet::new();
    let mut in_products = false;

    for line in text.lines() {
        if line == "products:" {
            in_products = true;
            continue;
        }
        if !in_products {
            continue;
        }
        if line.starts_with("  ") && !line.starts_with("    ") && line.ends_with(':') {
            ids.insert(scalar(line.trim().trim_end_matches(':')));
        }
    }

    ids
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))
}

fn key_value(value: &str) -> Option<(String, String)> {
    let (key, value) = value.split_once(':')?;
    Some((scalar(key), scalar(value)))
}

fn inline_list(value: &str) -> Vec<String> {
    let value = value.trim();
    let Some(inner) = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    else {
        return Vec::new();
    };
    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner.split(',').map(scalar).collect()
}

fn scalar(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_owned()
}

fn http_token(value: &str) -> Option<String> {
    let start = value.find("http")?;
    let tail = &value[start..];
    let end = tail.find(['"', '\'', '}', ' ', ',']).unwrap_or(tail.len());
    Some(tail[..end].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_market_model_is_valid() {
        let report = validate_repository(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("market model should be readable");
        assert!(
            report.errors.is_empty(),
            "market model validation errors:\n{}",
            report.errors.join("\n")
        );
    }

    #[test]
    fn parses_inline_lists() {
        assert_eq!(
            inline_list("[replay, deterministic-ci, contracts]"),
            ["replay", "deterministic-ci", "contracts"]
        );
        assert!(inline_list("[]").is_empty());
    }
}
