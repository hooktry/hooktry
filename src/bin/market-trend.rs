use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::Path,
    process::ExitCode,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Capability {
    id: String,
    disposition: String,
    ortyo_status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Signal {
    id: String,
    capability: String,
    class: String,
    direction: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Observation {
    id: String,
    product: String,
    capability: String,
    state: String,
}

#[derive(Debug, Clone, Default)]
struct Snapshot {
    snapshot_id: String,
    captured_at: String,
    scope_products: BTreeSet<String>,
    matrix_products: BTreeSet<String>,
    capabilities: BTreeMap<String, Capability>,
    matrix: BTreeMap<String, BTreeMap<String, String>>,
    signals: BTreeMap<String, Signal>,
    observations: BTreeMap<String, Observation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValueChange {
    id: String,
    from: String,
    to: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MatrixChange {
    product: String,
    capability: String,
    from: String,
    to: String,
    kind: String,
    disposition: String,
    decision_relevant: bool,
}

#[derive(Debug, Clone, Default)]
struct TrendReport {
    from_snapshot: String,
    to_snapshot: String,
    scope_products_added: Vec<String>,
    scope_products_removed: Vec<String>,
    matrix_products_added: Vec<String>,
    matrix_products_removed: Vec<String>,
    capabilities_added: Vec<String>,
    capabilities_removed: Vec<String>,
    disposition_changes: Vec<ValueChange>,
    ortyo_status_changes: Vec<ValueChange>,
    matrix_changes: Vec<MatrixChange>,
    signals_added: Vec<Signal>,
    signals_removed: Vec<Signal>,
    observations_added: Vec<Observation>,
    observations_removed: Vec<Observation>,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(config) = parse_args(&args) else {
        eprintln!("usage: market-trend --from SNAPSHOT --to SNAPSHOT [--json]");
        return ExitCode::from(2);
    };

    let from = match load_snapshot(Path::new(&config.from)) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("market trend failed: {error}");
            return ExitCode::from(2);
        }
    };
    let to = match load_snapshot(Path::new(&config.to)) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("market trend failed: {error}");
            return ExitCode::from(2);
        }
    };

    if from.captured_at > to.captured_at {
        eprintln!(
            "market trend failed: from snapshot {} is newer than to snapshot {}",
            from.snapshot_id, to.snapshot_id
        );
        return ExitCode::from(2);
    }

    let report = diff_snapshots(&from, &to);
    if config.json {
        println!("{}", render_json(&report));
    } else {
        print!("{}", render_text(&report));
    }

    ExitCode::SUCCESS
}

struct Config {
    from: String,
    to: String,
    json: bool,
}

fn parse_args(args: &[String]) -> Option<Config> {
    let mut from = None;
    let mut to = None;
    let mut json = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--from" => {
                index += 1;
                from = args.get(index).cloned();
            }
            "--to" => {
                index += 1;
                to = args.get(index).cloned();
            }
            "--json" => json = true,
            _ => return None,
        }
        index += 1;
    }

    Some(Config {
        from: from?,
        to: to?,
        json,
    })
}

fn load_snapshot(path: &Path) -> Result<Snapshot, String> {
    let text =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let snapshot = parse_snapshot(&text);
    if snapshot.snapshot_id.is_empty() || snapshot.captured_at.is_empty() {
        return Err(format!(
            "{} is not a valid market snapshot header",
            path.display()
        ));
    }
    Ok(snapshot)
}

fn diff_snapshots(from: &Snapshot, to: &Snapshot) -> TrendReport {
    let mut report = TrendReport {
        from_snapshot: from.snapshot_id.clone(),
        to_snapshot: to.snapshot_id.clone(),
        scope_products_added: set_added(&from.scope_products, &to.scope_products),
        scope_products_removed: set_added(&to.scope_products, &from.scope_products),
        matrix_products_added: set_added(&from.matrix_products, &to.matrix_products),
        matrix_products_removed: set_added(&to.matrix_products, &from.matrix_products),
        ..TrendReport::default()
    };

    let from_capabilities: BTreeSet<_> = from.capabilities.keys().cloned().collect();
    let to_capabilities: BTreeSet<_> = to.capabilities.keys().cloned().collect();
    report.capabilities_added = set_added(&from_capabilities, &to_capabilities);
    report.capabilities_removed = set_added(&to_capabilities, &from_capabilities);

    for id in from_capabilities.intersection(&to_capabilities) {
        let before = &from.capabilities[id];
        let after = &to.capabilities[id];

        if before.disposition != after.disposition {
            report.disposition_changes.push(ValueChange {
                id: id.clone(),
                from: before.disposition.clone(),
                to: after.disposition.clone(),
            });
        }
        if before.ortyo_status != after.ortyo_status {
            report.ortyo_status_changes.push(ValueChange {
                id: id.clone(),
                from: before.ortyo_status.clone(),
                to: after.ortyo_status.clone(),
            });
        }
    }

    let mut capability_ids: BTreeSet<String> = from.matrix.keys().cloned().collect();
    capability_ids.extend(to.matrix.keys().cloned());
    let mut products: BTreeSet<String> = from.matrix_products.clone();
    products.extend(to.matrix_products.iter().cloned());

    for capability in capability_ids {
        for product in &products {
            let before = matrix_state(from, &capability, product);
            let after = matrix_state(to, &capability, product);

            if before == after {
                continue;
            }

            let disposition = to
                .capabilities
                .get(&capability)
                .or_else(|| from.capabilities.get(&capability))
                .map(|value| value.disposition.clone())
                .unwrap_or_else(|| "unknown".to_owned());

            report.matrix_changes.push(MatrixChange {
                product: product.clone(),
                capability: capability.clone(),
                kind: classify_matrix_change(product, before, after),
                decision_relevant: product == "ortyo"
                    || (disposition != "out_of_scope"
                        && !matches!((before, after), ("unknown", "unknown"))),
                disposition,
                from: before.to_owned(),
                to: after.to_owned(),
            });
        }
    }

    report.signals_added = map_added(&from.signals, &to.signals);
    report.signals_removed = map_added(&to.signals, &from.signals);
    report.observations_added = map_added(&from.observations, &to.observations);
    report.observations_removed = map_added(&to.observations, &from.observations);

    report
}

fn matrix_state<'a>(snapshot: &'a Snapshot, capability: &str, product: &str) -> &'a str {
    snapshot
        .matrix
        .get(capability)
        .and_then(|row| row.get(product))
        .map(String::as_str)
        .unwrap_or("not_in_projection")
}

fn classify_matrix_change(product: &str, from: &str, to: &str) -> String {
    if product == "ortyo" {
        return "ortyo_motion".to_owned();
    }
    match (from, to) {
        ("unknown", "present" | "partial" | "absent") => "research_resolution",
        ("present" | "partial" | "absent", "unknown") => "research_regression",
        ("not_in_projection", _) | (_, "not_in_projection") => "projection_scope_change",
        _ => "observed_state_change",
    }
    .to_owned()
}

fn set_added(from: &BTreeSet<String>, to: &BTreeSet<String>) -> Vec<String> {
    to.difference(from).cloned().collect()
}

fn map_added<T: Clone>(from: &BTreeMap<String, T>, to: &BTreeMap<String, T>) -> Vec<T> {
    to.iter()
        .filter(|(id, _)| !from.contains_key(*id))
        .map(|(_, value)| value.clone())
        .collect()
}

fn render_text(report: &TrendReport) -> String {
    let market_motion_added = report
        .signals_added
        .iter()
        .filter(|signal| signal.class == "market_motion")
        .count();
    let direct_demand_added = report
        .signals_added
        .iter()
        .filter(|signal| signal.class == "direct_demand")
        .count();
    let research_resolutions = report
        .matrix_changes
        .iter()
        .filter(|change| change.kind == "research_resolution")
        .count();
    let observed_state_changes = report
        .matrix_changes
        .iter()
        .filter(|change| change.kind == "observed_state_change")
        .count();
    let ortyo_matrix_changes = report
        .matrix_changes
        .iter()
        .filter(|change| change.kind == "ortyo_motion")
        .count();
    let decision_relevant_matrix_changes = report
        .matrix_changes
        .iter()
        .filter(|change| change.decision_relevant)
        .count();
    let decision_relevant_signals = report
        .signals_added
        .iter()
        .filter(|signal| matches!(signal.class.as_str(), "direct_demand" | "market_motion"))
        .count();

    let mut output = String::new();
    output.push_str(&format!(
        "MCIF trend: {} -> {}\n\n",
        report.from_snapshot, report.to_snapshot
    ));
    output.push_str("Scope motion\n");
    output.push_str(&format!(
        "- scope products: +{} -{}\n",
        report.scope_products_added.len(),
        report.scope_products_removed.len()
    ));
    output.push_str(&format!(
        "- matrix products: +{} -{}\n",
        report.matrix_products_added.len(),
        report.matrix_products_removed.len()
    ));
    output.push_str(&format!(
        "- capabilities: +{} -{}\n\n",
        report.capabilities_added.len(),
        report.capabilities_removed.len()
    ));

    output.push_str("Evidence and market motion\n");
    output.push_str(&format!("- research resolutions: {research_resolutions}\n"));
    output.push_str(&format!(
        "- observed external state changes: {observed_state_changes}\n"
    ));
    output.push_str(&format!(
        "- observations: +{} -{}\n",
        report.observations_added.len(),
        report.observations_removed.len()
    ));
    output.push_str(&format!(
        "- signals: +{} -{}\n",
        report.signals_added.len(),
        report.signals_removed.len()
    ));
    output.push_str(&format!(
        "- new market_motion signals: {market_motion_added}\n"
    ));
    output.push_str(&format!(
        "- new direct_demand signals: {direct_demand_added}\n\n"
    ));

    output.push_str("Ortyo motion\n");
    output.push_str(&format!(
        "- capability status changes: {}\n",
        report.ortyo_status_changes.len()
    ));
    output.push_str(&format!("- matrix state changes: {ortyo_matrix_changes}\n"));
    output.push_str(&format!(
        "- disposition changes: {}\n\n",
        report.disposition_changes.len()
    ));

    output.push_str("Decision impact\n");
    output.push_str(&format!(
        "- decision-relevant matrix changes: {decision_relevant_matrix_changes}\n"
    ));
    output.push_str(&format!(
        "- decision-relevant new signals: {decision_relevant_signals}\n"
    ));
    output.push_str(&format!(
        "- disposition changes: {}\n",
        report.disposition_changes.len()
    ));

    if !report.matrix_changes.is_empty() {
        output.push_str("\nMatrix transitions\n");
        for change in &report.matrix_changes {
            output.push_str(&format!(
                "- {}/{}: {} -> {} [{}{}]\n",
                change.product,
                change.capability,
                change.from,
                change.to,
                change.kind,
                if change.decision_relevant {
                    ", decision-relevant"
                } else {
                    ""
                }
            ));
        }
    }

    if !report.signals_added.is_empty() {
        output.push_str("\nNew signals\n");
        for signal in &report.signals_added {
            output.push_str(&format!(
                "- {}: {} / {}{}\n",
                signal.id,
                signal.class,
                signal.capability,
                if signal.direction.is_empty() {
                    String::new()
                } else {
                    format!(" / {}", signal.direction)
                }
            ));
        }
    }

    output
}

fn render_json(report: &TrendReport) -> String {
    let mut fields = Vec::new();
    fields.push(format!(
        "\"from_snapshot\":\"{}\"",
        json_escape(&report.from_snapshot)
    ));
    fields.push(format!(
        "\"to_snapshot\":\"{}\"",
        json_escape(&report.to_snapshot)
    ));
    fields.push(format!(
        "\"scope_products_added\":{}",
        json_string_array(&report.scope_products_added)
    ));
    fields.push(format!(
        "\"scope_products_removed\":{}",
        json_string_array(&report.scope_products_removed)
    ));
    fields.push(format!(
        "\"matrix_products_added\":{}",
        json_string_array(&report.matrix_products_added)
    ));
    fields.push(format!(
        "\"matrix_products_removed\":{}",
        json_string_array(&report.matrix_products_removed)
    ));
    fields.push(format!(
        "\"capabilities_added\":{}",
        json_string_array(&report.capabilities_added)
    ));
    fields.push(format!(
        "\"capabilities_removed\":{}",
        json_string_array(&report.capabilities_removed)
    ));
    fields.push(format!(
        "\"disposition_changes\":{}",
        json_value_changes(&report.disposition_changes)
    ));
    fields.push(format!(
        "\"ortyo_status_changes\":{}",
        json_value_changes(&report.ortyo_status_changes)
    ));
    fields.push(format!(
        "\"matrix_changes\":{}",
        json_matrix_changes(&report.matrix_changes)
    ));
    fields.push(format!(
        "\"signals_added\":{}",
        json_signals(&report.signals_added)
    ));
    fields.push(format!(
        "\"signals_removed\":{}",
        json_signals(&report.signals_removed)
    ));
    fields.push(format!(
        "\"observations_added\":{}",
        json_observations(&report.observations_added)
    ));
    fields.push(format!(
        "\"observations_removed\":{}",
        json_observations(&report.observations_removed)
    ));

    format!("{{{}}}", fields.join(","))
}

fn json_string_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", json_escape(value)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_value_changes(values: &[ValueChange]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!(
                "{{\"id\":\"{}\",\"from\":\"{}\",\"to\":\"{}\"}}",
                json_escape(&value.id),
                json_escape(&value.from),
                json_escape(&value.to)
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_matrix_changes(values: &[MatrixChange]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!(
                "{{\"product\":\"{}\",\"capability\":\"{}\",\"from\":\"{}\",\"to\":\"{}\",\"kind\":\"{}\",\"disposition\":\"{}\",\"decision_relevant\":{}}}",
                json_escape(&value.product),
                json_escape(&value.capability),
                json_escape(&value.from),
                json_escape(&value.to),
                json_escape(&value.kind),
                json_escape(&value.disposition),
                value.decision_relevant
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_signals(values: &[Signal]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!(
                "{{\"id\":\"{}\",\"capability\":\"{}\",\"class\":\"{}\",\"direction\":\"{}\"}}",
                json_escape(&value.id),
                json_escape(&value.capability),
                json_escape(&value.class),
                json_escape(&value.direction)
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_observations(values: &[Observation]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!(
                "{{\"id\":\"{}\",\"product\":\"{}\",\"capability\":\"{}\",\"state\":\"{}\"}}",
                json_escape(&value.id),
                json_escape(&value.product),
                json_escape(&value.capability),
                json_escape(&value.state)
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
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
            flush_items(
                &mut snapshot,
                &mut current_capability,
                &mut current_signal,
                &mut current_observation,
            );

            if raw.ends_with(':') {
                section = raw.trim_end_matches(':');
                matrix_capability.clear();
            } else if let Some((key, value)) = key_value(raw) {
                match key.as_str() {
                    "snapshot_id" => snapshot.snapshot_id = value,
                    "captured_at" => snapshot.captured_at = value,
                    _ => {}
                }
                section = "";
            }
            continue;
        }

        match section {
            "scope_products" => {
                if let Some(value) = raw.strip_prefix("  - ") {
                    snapshot.scope_products.insert(scalar(value));
                }
            }
            "matrix_products" => {
                if let Some(value) = raw.strip_prefix("  - ") {
                    snapshot.matrix_products.insert(scalar(value));
                }
            }
            "capabilities" => {
                if let Some(value) = raw.strip_prefix("  - id: ") {
                    if let Some(item) = current_capability.take() {
                        snapshot.capabilities.insert(item.id.clone(), item);
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
                        "ortyo_status" => item.ortyo_status = value,
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
                        .insert(product, state);
                }
            }
            "signals" => {
                if let Some(value) = raw.strip_prefix("  - id: ") {
                    if let Some(item) = current_signal.take() {
                        snapshot.signals.insert(item.id.clone(), item);
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
                        _ => {}
                    }
                }
            }
            "observations" => {
                if let Some(value) = raw.strip_prefix("  - id: ") {
                    if let Some(item) = current_observation.take() {
                        snapshot.observations.insert(item.id.clone(), item);
                    }
                    current_observation = Some(Observation {
                        id: scalar(value),
                        ..Observation::default()
                    });
                } else if let Some(item) = current_observation.as_mut()
                    && let Some((key, value)) = key_value(raw.trim())
                {
                    match key.as_str() {
                        "product" => item.product = value,
                        "capability" => item.capability = value,
                        "state" => item.state = value,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    flush_items(
        &mut snapshot,
        &mut current_capability,
        &mut current_signal,
        &mut current_observation,
    );
    snapshot
}

fn flush_items(
    snapshot: &mut Snapshot,
    capability: &mut Option<Capability>,
    signal: &mut Option<Signal>,
    observation: &mut Option<Observation>,
) {
    if let Some(item) = capability.take() {
        snapshot.capabilities.insert(item.id.clone(), item);
    }
    if let Some(item) = signal.take() {
        snapshot.signals.insert(item.id.clone(), item);
    }
    if let Some(item) = observation.take() {
        snapshot.observations.insert(item.id.clone(), item);
    }
}

fn key_value(value: &str) -> Option<(String, String)> {
    let (key, value) = value.split_once(':')?;
    Some((scalar(key), scalar(value)))
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

    fn snapshot(text: &str) -> Snapshot {
        parse_snapshot(text)
    }

    #[test]
    fn self_diff_is_empty() {
        let value = snapshot(
            "snapshot_id: 2026-10-01-a\ncaptured_at: 2026-10-01\nscope_products:\n  - ortyo\nmatrix_products:\n  - ortyo\ncapabilities:\n  - id: replay\n    disposition: must\n    ortyo_status: implemented\nmatrix:\n  replay:\n    ortyo: present\nsignals:\nobservations:\n",
        );
        let report = diff_snapshots(&value, &value);
        assert!(report.matrix_changes.is_empty());
        assert!(report.signals_added.is_empty());
        assert!(report.observations_added.is_empty());
        assert!(report.ortyo_status_changes.is_empty());
    }

    #[test]
    fn unknown_to_present_is_research_resolution_not_market_motion() {
        let from = snapshot(
            "snapshot_id: 2026-10-01-a\ncaptured_at: 2026-10-01\nscope_products:\n  - ortyo\n  - hookdeck\nmatrix_products:\n  - ortyo\n  - hookdeck\ncapabilities:\n  - id: replay\n    disposition: must\n    ortyo_status: implemented\nmatrix:\n  replay:\n    ortyo: present\n    hookdeck: unknown\nsignals:\nobservations:\n",
        );
        let to = snapshot(
            "snapshot_id: 2026-10-02-b\ncaptured_at: 2026-10-02\nscope_products:\n  - ortyo\n  - hookdeck\nmatrix_products:\n  - ortyo\n  - hookdeck\ncapabilities:\n  - id: replay\n    disposition: must\n    ortyo_status: implemented\nmatrix:\n  replay:\n    ortyo: present\n    hookdeck: present\nsignals:\nobservations:\n  - id: hookdeck-replay\n    product: hookdeck\n    capability: replay\n    state: present\n",
        );

        let report = diff_snapshots(&from, &to);
        assert_eq!(report.matrix_changes.len(), 1);
        assert_eq!(report.matrix_changes[0].kind, "research_resolution");
        assert!(report.matrix_changes[0].decision_relevant);
    }

    #[test]
    fn ortyo_status_change_is_product_motion() {
        let from = snapshot(
            "snapshot_id: 2026-10-01-a\ncaptured_at: 2026-10-01\nscope_products:\n  - ortyo\nmatrix_products:\n  - ortyo\ncapabilities:\n  - id: provider-templates\n    disposition: should\n    ortyo_status: absent\nmatrix:\n  provider-templates:\n    ortyo: absent\nsignals:\nobservations:\n",
        );
        let to = snapshot(
            "snapshot_id: 2026-10-02-b\ncaptured_at: 2026-10-02\nscope_products:\n  - ortyo\nmatrix_products:\n  - ortyo\ncapabilities:\n  - id: provider-templates\n    disposition: should\n    ortyo_status: implemented\nmatrix:\n  provider-templates:\n    ortyo: present\nsignals:\nobservations:\n",
        );

        let report = diff_snapshots(&from, &to);
        assert_eq!(
            report.ortyo_status_changes,
            vec![ValueChange {
                id: "provider-templates".to_owned(),
                from: "absent".to_owned(),
                to: "implemented".to_owned(),
            }]
        );
        assert_eq!(report.matrix_changes[0].kind, "ortyo_motion");
    }

    #[test]
    fn new_market_motion_signal_stays_explicit() {
        let from = snapshot(
            "snapshot_id: 2026-10-01-a\ncaptured_at: 2026-10-01\nscope_products:\n  - ortyo\nmatrix_products:\n  - ortyo\ncapabilities:\n  - id: mcp-agent\n    disposition: covered\n    ortyo_status: implemented\nmatrix:\nsignals:\nobservations:\n",
        );
        let to = snapshot(
            "snapshot_id: 2026-10-02-b\ncaptured_at: 2026-10-02\nscope_products:\n  - ortyo\nmatrix_products:\n  - ortyo\ncapabilities:\n  - id: mcp-agent\n    disposition: covered\n    ortyo_status: implemented\nmatrix:\nsignals:\n  - id: competitor-agent-launch\n    capability: mcp-agent\n    class: market_motion\nobservations:\n",
        );

        let report = diff_snapshots(&from, &to);
        assert_eq!(report.signals_added.len(), 1);
        assert_eq!(report.signals_added[0].class, "market_motion");
    }
}
