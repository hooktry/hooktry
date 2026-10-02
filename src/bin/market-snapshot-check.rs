use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

const SNAPSHOT_DIR: &str = "docs/market/snapshots";
const MATRIX_STATES: &[&str] = &["present", "partial", "absent", "unknown"];
const DISPOSITIONS: &[&str] = &[
    "must",
    "should",
    "differentiation",
    "watch",
    "out_of_scope",
    "covered",
];
const HOOKTRY_STATUSES: &[&str] = &["implemented", "partial", "absent", "unknown", "planned"];
const SIGNAL_CLASSES: &[&str] = &["direct_demand", "demand_proxy", "market_motion"];
const DIRECT_DEMAND_DIRECTIONS: &[&str] = &["supports", "contradicts", "mixed"];

#[derive(Debug, Default)]
struct Counts {
    scope_products: usize,
    matrix_products: usize,
    capabilities: usize,
    matrix_rows: usize,
    matrix_cells: usize,
    observations: usize,
    signals: usize,
}

#[derive(Debug, Clone, Default)]
struct Capability {
    id: String,
    disposition: String,
    hooktry_status: String,
}

#[derive(Debug, Clone, Default)]
struct Signal {
    id: String,
    capability: String,
    class: String,
    direction: String,
    observed_at: String,
    confidence: String,
}

#[derive(Debug, Clone, Default)]
struct Observation {
    id: String,
    product: String,
    capability: String,
    state: String,
    observed_at: String,
    confidence: String,
    freshness: String,
    applicability: String,
}

#[derive(Debug, Default)]
struct Snapshot {
    schema_version: String,
    snapshot_id: String,
    captured_at: String,
    source_commit: String,
    scope_reviewed_at: String,
    matrix_checked_at: String,
    signals_checked_at: String,
    counts: Counts,
    scope_products: Vec<String>,
    matrix_products: Vec<String>,
    capabilities: Vec<Capability>,
    matrix: HashMap<String, HashMap<String, String>>,
    signals: Vec<Signal>,
    observations: Vec<Observation>,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if !(args.is_empty() || args == ["--check"]) {
        eprintln!("usage: market-snapshot-check [--check]");
        return ExitCode::from(2);
    }

    match validate_directory(Path::new(SNAPSHOT_DIR)) {
        Ok(errors) if errors.is_empty() => {
            println!("market snapshots OK");
            ExitCode::SUCCESS
        }
        Ok(errors) => {
            eprintln!("market snapshots invalid: {} error(s)", errors.len());
            for error in errors {
                eprintln!("- {error}");
            }
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("market snapshot check failed: {error}");
            ExitCode::from(2)
        }
    }
}

fn validate_directory(dir: &Path) -> Result<Vec<String>, String> {
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

    if paths.is_empty() {
        return Ok(vec!["no market snapshots found".to_owned()]);
    }

    let mut errors = Vec::new();
    let mut snapshot_ids = HashSet::new();
    let mut previous_date = String::new();

    for path in paths {
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        let snapshot = parse_snapshot(&text);

        let label = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("<snapshot>");

        if !snapshot_ids.insert(snapshot.snapshot_id.clone()) {
            errors.push(format!(
                "{label}: duplicate snapshot_id {}",
                snapshot.snapshot_id
            ));
        }
        if !previous_date.is_empty() && snapshot.captured_at < previous_date {
            errors.push(format!(
                "{label}: captured_at {} sorts before previous snapshot date {}",
                snapshot.captured_at, previous_date
            ));
        }
        previous_date = snapshot.captured_at.clone();

        validate_snapshot(label, &snapshot, &mut errors);
    }

    Ok(errors)
}

fn validate_snapshot(label: &str, snapshot: &Snapshot, errors: &mut Vec<String>) {
    if snapshot.schema_version != "1" {
        errors.push(format!(
            "{label}: unsupported schema_version {}",
            snapshot.schema_version
        ));
    }
    if snapshot.snapshot_id.is_empty() {
        errors.push(format!("{label}: missing snapshot_id"));
    }
    if snapshot.captured_at.is_empty() {
        errors.push(format!("{label}: missing captured_at"));
    }
    if !snapshot.snapshot_id.starts_with(&snapshot.captured_at) {
        errors.push(format!(
            "{label}: snapshot_id {} must start with captured_at {}",
            snapshot.snapshot_id, snapshot.captured_at
        ));
    }
    if snapshot.source_commit.len() != 40
        || !snapshot
            .source_commit
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        errors.push(format!(
            "{label}: source_commit must be a 40-character Git SHA"
        ));
    }

    for (field, date) in [
        ("scope_reviewed_at", &snapshot.scope_reviewed_at),
        ("matrix_checked_at", &snapshot.matrix_checked_at),
        ("signals_checked_at", &snapshot.signals_checked_at),
    ] {
        if date.is_empty() {
            errors.push(format!("{label}: missing source_dates.{field}"));
        } else if !snapshot.captured_at.is_empty() && date > &snapshot.captured_at {
            errors.push(format!(
                "{label}: source_dates.{field}={date} is after captured_at={}",
                snapshot.captured_at
            ));
        }
    }

    let scope_products = unique_set(label, "scope product", &snapshot.scope_products, errors);
    let matrix_products = unique_set(label, "matrix product", &snapshot.matrix_products, errors);

    if !scope_products.contains("hooktry") {
        errors.push(format!("{label}: scope_products must contain hooktry"));
    }
    if !matrix_products.contains("hooktry") {
        errors.push(format!("{label}: matrix_products must contain hooktry"));
    }
    for product in &matrix_products {
        if !scope_products.contains(*product) {
            errors.push(format!(
                "{label}: matrix product {product} is outside scope_products"
            ));
        }
    }

    let mut capability_map = HashMap::new();
    for capability in &snapshot.capabilities {
        if capability.id.is_empty() {
            errors.push(format!("{label}: capability without id"));
            continue;
        }
        if capability_map
            .insert(capability.id.as_str(), capability)
            .is_some()
        {
            errors.push(format!(
                "{label}: duplicate capability id {}",
                capability.id
            ));
        }
        if !DISPOSITIONS.contains(&capability.disposition.as_str()) {
            errors.push(format!(
                "{label}: capability {} has invalid disposition {}",
                capability.id, capability.disposition
            ));
        }
        if !HOOKTRY_STATUSES.contains(&capability.hooktry_status.as_str()) {
            errors.push(format!(
                "{label}: capability {} has invalid hooktry_status {}",
                capability.id, capability.hooktry_status
            ));
        }
    }

    let mut matrix_cells = 0usize;
    for (capability_id, row) in &snapshot.matrix {
        matrix_cells += row.len();
        let Some(capability) = capability_map.get(capability_id.as_str()) else {
            errors.push(format!(
                "{label}: matrix references unknown capability {capability_id}"
            ));
            continue;
        };

        for product in &snapshot.matrix_products {
            if !row.contains_key(product) {
                errors.push(format!(
                    "{label}: matrix row {capability_id} is missing product {product}"
                ));
            }
        }

        for (product, state) in row {
            if !matrix_products.contains(product.as_str()) {
                errors.push(format!(
                    "{label}: matrix row {capability_id} contains undeclared product {product}"
                ));
            }
            if !MATRIX_STATES.contains(&state.as_str()) {
                errors.push(format!(
                    "{label}: matrix cell {product}/{capability_id} has invalid state {state}"
                ));
            }
        }

        if let Some(state) = row.get("hooktry") {
            let expected = match capability.hooktry_status.as_str() {
                "implemented" => "present",
                "partial" => "partial",
                "absent" => "absent",
                "unknown" | "planned" => "unknown",
                _ => "",
            };
            if !expected.is_empty() && state != expected {
                errors.push(format!(
                    "{label}: hooktry/{capability_id}={state}, expected {expected} from snapshot hooktry_status"
                ));
            }
        }
    }

    let mut signal_ids = HashSet::new();
    for signal in &snapshot.signals {
        if signal.id.is_empty() {
            errors.push(format!("{label}: signal without id"));
        } else if !signal_ids.insert(signal.id.as_str()) {
            errors.push(format!("{label}: duplicate signal id {}", signal.id));
        }
        if !capability_map.contains_key(signal.capability.as_str()) {
            errors.push(format!(
                "{label}: signal {} references unknown capability {}",
                signal.id, signal.capability
            ));
        }
        if !SIGNAL_CLASSES.contains(&signal.class.as_str()) {
            errors.push(format!(
                "{label}: signal {} has invalid class {}",
                signal.id, signal.class
            ));
        }
        if signal.class == "direct_demand"
            && !DIRECT_DEMAND_DIRECTIONS.contains(&signal.direction.as_str())
        {
            errors.push(format!(
                "{label}: direct-demand signal {} has invalid direction {}",
                signal.id, signal.direction
            ));
        }
        if signal.observed_at.is_empty() || signal.observed_at > snapshot.captured_at {
            errors.push(format!(
                "{label}: signal {} has invalid observed_at {}",
                signal.id, signal.observed_at
            ));
        }
        if signal.confidence.is_empty() {
            errors.push(format!(
                "{label}: signal {} is missing confidence",
                signal.id
            ));
        }
    }

    let mut observation_ids = HashSet::new();
    for observation in &snapshot.observations {
        if observation.id.is_empty() {
            errors.push(format!("{label}: observation without id"));
        } else if !observation_ids.insert(observation.id.as_str()) {
            errors.push(format!(
                "{label}: duplicate observation id {}",
                observation.id
            ));
        }
        if !scope_products.contains(observation.product.as_str()) {
            errors.push(format!(
                "{label}: observation {} references product outside snapshot scope: {}",
                observation.id, observation.product
            ));
        }
        if !capability_map.contains_key(observation.capability.as_str()) {
            errors.push(format!(
                "{label}: observation {} references unknown capability {}",
                observation.id, observation.capability
            ));
        }
        if !matches!(observation.state.as_str(), "present" | "partial" | "absent") {
            errors.push(format!(
                "{label}: observation {} has invalid evidentiary state {}",
                observation.id, observation.state
            ));
        }
        if observation.observed_at.is_empty() || observation.observed_at > snapshot.captured_at {
            errors.push(format!(
                "{label}: observation {} has invalid observed_at {}",
                observation.id, observation.observed_at
            ));
        }
        if observation.confidence.is_empty()
            || observation.freshness.is_empty()
            || observation.applicability.is_empty()
        {
            errors.push(format!(
                "{label}: observation {} is missing confidence/freshness/applicability",
                observation.id
            ));
        }
    }

    let expected = &snapshot.counts;
    for (name, actual, declared) in [
        (
            "scope_products",
            snapshot.scope_products.len(),
            expected.scope_products,
        ),
        (
            "matrix_products",
            snapshot.matrix_products.len(),
            expected.matrix_products,
        ),
        (
            "capabilities",
            snapshot.capabilities.len(),
            expected.capabilities,
        ),
        ("matrix_rows", snapshot.matrix.len(), expected.matrix_rows),
        ("matrix_cells", matrix_cells, expected.matrix_cells),
        (
            "observations",
            snapshot.observations.len(),
            expected.observations,
        ),
        ("signals", snapshot.signals.len(), expected.signals),
    ] {
        if actual != declared {
            errors.push(format!(
                "{label}: counts.{name}={declared}, actual={actual}"
            ));
        }
    }
}

fn unique_set<'a>(
    label: &str,
    kind: &str,
    values: &'a [String],
    errors: &mut Vec<String>,
) -> HashSet<&'a str> {
    let mut set = HashSet::new();
    for value in values {
        if !set.insert(value.as_str()) {
            errors.push(format!("{label}: duplicate {kind}: {value}"));
        }
    }
    set
}

fn parse_snapshot(text: &str) -> Snapshot {
    let mut snapshot = Snapshot::default();
    let mut section = "";
    let mut current_capability: Option<Capability> = None;
    let mut current_signal: Option<Signal> = None;
    let mut current_observation: Option<Observation> = None;
    let mut matrix_capability = String::new();

    for raw in text.lines() {
        if !raw.starts_with(' ') {
            if let Some(item) = current_capability.take() {
                snapshot.capabilities.push(item);
            }
            if let Some(item) = current_signal.take() {
                snapshot.signals.push(item);
            }
            if let Some(item) = current_observation.take() {
                snapshot.observations.push(item);
            }

            if raw.ends_with(':') {
                section = raw.trim_end_matches(':');
                matrix_capability.clear();
            } else if let Some((key, value)) = key_value(raw) {
                match key.as_str() {
                    "schema_version" => snapshot.schema_version = value,
                    "snapshot_id" => snapshot.snapshot_id = value,
                    "captured_at" => snapshot.captured_at = value,
                    "source_commit" => snapshot.source_commit = value,
                    _ => {}
                }
                section = "";
            }
            continue;
        }

        match section {
            "source_dates" => {
                if let Some((key, value)) = key_value(raw.trim()) {
                    match key.as_str() {
                        "scope_reviewed_at" => snapshot.scope_reviewed_at = value,
                        "matrix_checked_at" => snapshot.matrix_checked_at = value,
                        "signals_checked_at" => snapshot.signals_checked_at = value,
                        _ => {}
                    }
                }
            }
            "counts" => {
                if let Some((key, value)) = key_value(raw.trim()) {
                    let number = value.parse::<usize>().unwrap_or_default();
                    match key.as_str() {
                        "scope_products" => snapshot.counts.scope_products = number,
                        "matrix_products" => snapshot.counts.matrix_products = number,
                        "capabilities" => snapshot.counts.capabilities = number,
                        "matrix_rows" => snapshot.counts.matrix_rows = number,
                        "matrix_cells" => snapshot.counts.matrix_cells = number,
                        "observations" => snapshot.counts.observations = number,
                        "signals" => snapshot.counts.signals = number,
                        _ => {}
                    }
                }
            }
            "scope_products" => {
                if let Some(value) = raw.strip_prefix("  - ") {
                    snapshot.scope_products.push(canonical_product(value));
                }
            }
            "matrix_products" => {
                if let Some(value) = raw.strip_prefix("  - ") {
                    snapshot.matrix_products.push(canonical_product(value));
                }
            }
            "capabilities" => {
                if let Some(value) = raw.strip_prefix("  - id: ") {
                    if let Some(item) = current_capability.take() {
                        snapshot.capabilities.push(item);
                    }
                    current_capability = Some(Capability {
                        id: scalar(value),
                        ..Capability::default()
                    });
                } else if let Some(item) = current_capability.as_mut()
                    && let Some((key, value)) = key_value(raw.trim())
                {
                    match key.as_str() {
                        "disposition" => item.disposition = value,
                        "hooktry_status" | "ortyo_status" => item.hooktry_status = value,
                        _ => {}
                    }
                }
            }
            "matrix" => {
                if raw.starts_with("  ") && !raw.starts_with("    ") && raw.ends_with(':') {
                    matrix_capability = scalar(raw.trim().trim_end_matches(':'));
                    snapshot
                        .matrix
                        .entry(matrix_capability.clone())
                        .or_default();
                } else if raw.starts_with("    ")
                    && !matrix_capability.is_empty()
                    && let Some((product, state)) = key_value(raw.trim())
                {
                    snapshot
                        .matrix
                        .entry(matrix_capability.clone())
                        .or_default()
                        .insert(canonical_product(&product), state);
                }
            }
            "signals" => {
                if let Some(value) = raw.strip_prefix("  - id: ") {
                    if let Some(item) = current_signal.take() {
                        snapshot.signals.push(item);
                    }
                    current_signal = Some(Signal {
                        id: scalar(value),
                        ..Signal::default()
                    });
                } else if let Some(item) = current_signal.as_mut()
                    && let Some((key, value)) = key_value(raw.trim())
                {
                    match key.as_str() {
                        "capability" => item.capability = value,
                        "class" => item.class = value,
                        "direction" => item.direction = value,
                        "observed_at" => item.observed_at = value,
                        "confidence" => item.confidence = value,
                        _ => {}
                    }
                }
            }
            "observations" => {
                if let Some(value) = raw.strip_prefix("  - id: ") {
                    if let Some(item) = current_observation.take() {
                        snapshot.observations.push(item);
                    }
                    current_observation = Some(Observation {
                        id: scalar(value),
                        ..Observation::default()
                    });
                } else if let Some(item) = current_observation.as_mut()
                    && let Some((key, value)) = key_value(raw.trim())
                {
                    match key.as_str() {
                        "product" => item.product = canonical_product(&value),
                        "capability" => item.capability = value,
                        "state" => item.state = value,
                        "observed_at" => item.observed_at = value,
                        "confidence" => item.confidence = value,
                        "freshness" => item.freshness = value,
                        "applicability" => item.applicability = value,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(item) = current_capability {
        snapshot.capabilities.push(item);
    }
    if let Some(item) = current_signal {
        snapshot.signals.push(item);
    }
    if let Some(item) = current_observation {
        snapshot.observations.push(item);
    }

    snapshot
}

fn key_value(value: &str) -> Option<(String, String)> {
    let (key, value) = value.split_once(':')?;
    Some((scalar(key), scalar(value)))
}

fn canonical_product(value: &str) -> String {
    let value = scalar(value);
    if value == "ortyo" {
        "hooktry".to_owned()
    } else {
        value
    }
}

fn scalar(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_snapshots_are_valid() {
        let errors = validate_directory(&Path::new(env!("CARGO_MANIFEST_DIR")).join(SNAPSHOT_DIR))
            .expect("snapshots should be readable");
        assert!(
            errors.is_empty(),
            "snapshot validation errors:\n{}",
            errors.join("\n")
        );
    }

    #[test]
    fn canonicalizes_legacy_ortyo_snapshot_brand() {
        let snapshot = parse_snapshot(
            "schema_version: 1\nsnapshot_id: 2026-10-01-legacy\ncaptured_at: 2026-10-01\nsource_commit: 0123456789012345678901234567890123456789\nscope_products:\n  - ortyo\nmatrix_products:\n  - ortyo\ncapabilities:\n  - id: replay\n    disposition: must\n    ortyo_status: implemented\nmatrix:\n  replay:\n    ortyo: present\nobservations:\n  - id: legacy-observation\n    product: ortyo\n    capability: replay\n    state: present\n",
        );

        assert_eq!(snapshot.scope_products, ["hooktry"]);
        assert_eq!(snapshot.matrix_products, ["hooktry"]);
        assert_eq!(snapshot.capabilities[0].hooktry_status, "implemented");
        assert_eq!(snapshot.matrix["replay"]["hooktry"], "present");
        assert_eq!(snapshot.observations[0].product, "hooktry");
    }

    #[test]
    fn parses_baseline_header() {
        let snapshot = parse_snapshot(
            "schema_version: 1\nsnapshot_id: 2026-10-01-baseline\ncaptured_at: 2026-10-01\nsource_commit: 0123456789012345678901234567890123456789\n",
        );
        assert_eq!(snapshot.schema_version, "1");
        assert_eq!(snapshot.snapshot_id, "2026-10-01-baseline");
        assert_eq!(snapshot.captured_at, "2026-10-01");
    }
}
