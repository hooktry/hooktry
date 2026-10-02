use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
    env, fs,
    path::Path,
    process::ExitCode,
};

#[derive(Debug, Clone, Default)]
struct Capability {
    id: String,
    disposition: String,
    hooktry_status: String,
}

#[derive(Debug, Clone, Default)]
struct Product {
    id: String,
    cohorts: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct Priority {
    horizon: String,
    next_evidence: String,
    capabilities: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct SignalProfile {
    direct_demand: usize,
    demand_proxy: usize,
    market_motion: usize,
}

#[derive(Debug, Default)]
struct MarketModel {
    capabilities: HashMap<String, Capability>,
    products: HashMap<String, Product>,
    priorities: Vec<Priority>,
    signals: HashMap<String, SignalProfile>,
    matrix_products: Vec<String>,
    matrix: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    product: String,
    capability: String,
    tier: String,
    horizon: String,
    disposition: String,
    hooktry_status: String,
    direct_demand: usize,
    supporting_signals: usize,
    known_external_peers: usize,
    product_cohorts: usize,
    research_channel: String,
    reasons: Vec<String>,
}

#[derive(Debug, Clone)]
struct CapabilityDebt {
    capability: String,
    tier: String,
    horizon: String,
    disposition: String,
    hooktry_status: String,
    unknown_cells: usize,
    known_external_peers: usize,
    direct_demand: usize,
    supporting_signals: usize,
    research_channel: String,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let config = match parse_args(&args) {
        Some(config) => config,
        None => {
            eprintln!("usage: market-research-debt [--limit N] [--max-per-capability N] [--json]");
            return ExitCode::from(2);
        }
    };

    let model = match load_model(Path::new("docs/market")) {
        Ok(model) => model,
        Err(error) => {
            eprintln!("market research debt failed: {error}");
            return ExitCode::from(2);
        }
    };

    let mut candidates = build_candidates(&model);
    candidates.sort_by(compare_candidates);

    let debt = capability_debt(&model, &candidates);
    let top_checks = select_top_checks(&candidates, config.limit, config.max_per_capability);
    let output = if config.json {
        render_json(&model, &candidates, &debt, &top_checks)
    } else {
        render_text(&model, &candidates, &debt, &top_checks)
    };
    print!("{output}");

    ExitCode::SUCCESS
}

struct Config {
    limit: usize,
    max_per_capability: usize,
    json: bool,
}

fn parse_args(args: &[String]) -> Option<Config> {
    let mut limit = 20usize;
    let mut max_per_capability = 3usize;
    let mut json = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--limit" => {
                index += 1;
                limit = args.get(index)?.parse().ok()?;
            }
            "--max-per-capability" => {
                index += 1;
                max_per_capability = args.get(index)?.parse().ok()?;
            }
            "--json" => json = true,
            _ => return None,
        }
        index += 1;
    }

    Some(Config {
        limit,
        max_per_capability,
        json,
    })
}

fn load_model(dir: &Path) -> Result<MarketModel, String> {
    let capabilities = parse_capabilities(&read(&dir.join("capabilities.yaml"))?);
    let products = parse_products(&read(&dir.join("scope.yaml"))?);
    let priorities = parse_priorities(&read(&dir.join("priorities.yaml"))?);
    let signals = parse_signals(&read(&dir.join("signals.yaml"))?);
    let (matrix_products, matrix) = parse_matrix(&read(&dir.join("matrix.yaml"))?);

    Ok(MarketModel {
        capabilities: capabilities
            .into_iter()
            .map(|capability| (capability.id.clone(), capability))
            .collect(),
        products: products
            .into_iter()
            .map(|product| (product.id.clone(), product))
            .collect(),
        priorities,
        signals,
        matrix_products,
        matrix,
    })
}

fn build_candidates(model: &MarketModel) -> Vec<Candidate> {
    let mut candidates = Vec::new();

    for (capability_id, row) in &model.matrix {
        let Some(capability) = model.capabilities.get(capability_id) else {
            continue;
        };
        let priority = effective_priority(capability, &model.priorities);
        let signal = model
            .signals
            .get(capability_id)
            .cloned()
            .unwrap_or_default();
        let known_external_peers = row
            .iter()
            .filter(|(product, state)| product.as_str() != "hooktry" && state.as_str() != "unknown")
            .count();

        for product_id in &model.matrix_products {
            if product_id == "hooktry" || row.get(product_id).map(String::as_str) != Some("unknown")
            {
                continue;
            }

            let product = model.products.get(product_id);
            let product_cohorts = product.map_or(0, |value| value.cohorts.len());
            let research_channel = if priority.next_evidence.is_empty() {
                "market_research".to_owned()
            } else {
                priority.next_evidence.clone()
            };
            let mut reasons = vec![
                format!("horizon={}", priority.horizon),
                format!("disposition={}", capability.disposition),
                format!("hooktry={}", capability.hooktry_status),
                format!("next_evidence={research_channel}"),
            ];
            if signal.direct_demand > 0 {
                reasons.push(format!("direct_demand={}", signal.direct_demand));
            }
            let supporting_signals = signal.demand_proxy + signal.market_motion;
            if supporting_signals > 0 {
                reasons.push(format!("supporting_signals={supporting_signals}"));
            }
            reasons.push(format!("known_peers={known_external_peers}"));
            if product_cohorts > 1 {
                reasons.push(format!("product_cohorts={product_cohorts}"));
            }

            candidates.push(Candidate {
                product: product_id.clone(),
                capability: capability_id.clone(),
                tier: decision_tier(
                    &priority.horizon,
                    &capability.hooktry_status,
                    &capability.disposition,
                    signal.direct_demand,
                    supporting_signals,
                )
                .to_owned(),
                horizon: priority.horizon.clone(),
                disposition: capability.disposition.clone(),
                hooktry_status: capability.hooktry_status.clone(),
                direct_demand: signal.direct_demand,
                supporting_signals,
                known_external_peers,
                product_cohorts,
                research_channel,
                reasons,
            });
        }
    }

    candidates
}

fn effective_priority(capability: &Capability, priorities: &[Priority]) -> Priority {
    let mut matches: Vec<Priority> = priorities
        .iter()
        .filter(|priority| priority.capabilities.contains(&capability.id))
        .cloned()
        .collect();
    matches.sort_by_key(|priority| horizon_rank(&priority.horizon));

    if let Some(priority) = matches.into_iter().next() {
        return priority;
    }

    let horizon = match capability.disposition.as_str() {
        "must" => "now",
        "should" => "next",
        "differentiation" => "validate",
        "watch" | "covered" => "watch",
        "out_of_scope" => "substrate",
        _ => "watch",
    };

    Priority {
        horizon: horizon.to_owned(),
        next_evidence: "market_research".to_owned(),
        capabilities: vec![capability.id.clone()],
    }
}

fn decision_tier(
    horizon: &str,
    hooktry_status: &str,
    disposition: &str,
    direct_demand: usize,
    supporting_signals: usize,
) -> &'static str {
    let open_gap = hooktry_status != "implemented";

    if horizon == "now" && open_gap {
        "P0"
    } else if horizon == "next" && open_gap && direct_demand > 0 {
        "P1"
    } else if (horizon == "next" && open_gap)
        || (horizon == "validate" && open_gap && direct_demand > 0)
        || (disposition == "differentiation" && (direct_demand > 0 || supporting_signals > 0))
    {
        "P2"
    } else if matches!(horizon, "validate" | "watch" | "option") && open_gap {
        "P3"
    } else {
        "P4"
    }
}

fn tier_rank(tier: &str) -> usize {
    match tier {
        "P0" => 0,
        "P1" => 1,
        "P2" => 2,
        "P3" => 3,
        _ => 4,
    }
}

fn horizon_rank(horizon: &str) -> usize {
    match horizon {
        "now" => 0,
        "next" => 1,
        "validate" => 2,
        "watch" => 3,
        "option" => 4,
        "substrate" => 5,
        _ => 6,
    }
}

fn signal_rank(candidate: &Candidate) -> usize {
    if candidate.direct_demand > 0 {
        0
    } else if candidate.supporting_signals > 0 {
        1
    } else {
        2
    }
}

fn hooktry_gap_rank(status: &str) -> usize {
    match status {
        "absent" | "unknown" | "planned" => 0,
        "partial" => 1,
        "implemented" => 2,
        _ => 3,
    }
}

fn disposition_rank(disposition: &str) -> usize {
    match disposition {
        "must" => 0,
        "should" => 1,
        "differentiation" => 2,
        "watch" => 3,
        "covered" => 4,
        "out_of_scope" => 5,
        _ => 6,
    }
}

fn compare_candidates(left: &Candidate, right: &Candidate) -> Ordering {
    (
        tier_rank(&left.tier),
        signal_rank(left),
        hooktry_gap_rank(&left.hooktry_status),
        horizon_rank(&left.horizon),
        disposition_rank(&left.disposition),
        left.known_external_peers,
        usize::MAX - left.product_cohorts,
        &left.capability,
        &left.product,
    )
        .cmp(&(
            tier_rank(&right.tier),
            signal_rank(right),
            hooktry_gap_rank(&right.hooktry_status),
            horizon_rank(&right.horizon),
            disposition_rank(&right.disposition),
            right.known_external_peers,
            usize::MAX - right.product_cohorts,
            &right.capability,
            &right.product,
        ))
}

fn capability_debt(model: &MarketModel, candidates: &[Candidate]) -> Vec<CapabilityDebt> {
    let mut grouped: HashMap<String, Vec<&Candidate>> = HashMap::new();
    for candidate in candidates {
        grouped
            .entry(candidate.capability.clone())
            .or_default()
            .push(candidate);
    }

    let mut debt = Vec::new();
    for (capability, items) in grouped {
        let Some(first) = items.first() else {
            continue;
        };
        debt.push(CapabilityDebt {
            capability,
            tier: first.tier.clone(),
            horizon: first.horizon.clone(),
            disposition: first.disposition.clone(),
            hooktry_status: first.hooktry_status.clone(),
            unknown_cells: items.len(),
            known_external_peers: first.known_external_peers,
            direct_demand: first.direct_demand,
            supporting_signals: first.supporting_signals,
            research_channel: first.research_channel.clone(),
        });
    }

    debt.sort_by(|left, right| {
        let left_candidate = Candidate {
            product: String::new(),
            capability: left.capability.clone(),
            tier: left.tier.clone(),
            horizon: left.horizon.clone(),
            disposition: left.disposition.clone(),
            hooktry_status: left.hooktry_status.clone(),
            direct_demand: left.direct_demand,
            supporting_signals: left.supporting_signals,
            known_external_peers: left.known_external_peers,
            product_cohorts: 0,
            research_channel: left.research_channel.clone(),
            reasons: Vec::new(),
        };
        let right_candidate = Candidate {
            product: String::new(),
            capability: right.capability.clone(),
            tier: right.tier.clone(),
            horizon: right.horizon.clone(),
            disposition: right.disposition.clone(),
            hooktry_status: right.hooktry_status.clone(),
            direct_demand: right.direct_demand,
            supporting_signals: right.supporting_signals,
            known_external_peers: right.known_external_peers,
            product_cohorts: 0,
            research_channel: right.research_channel.clone(),
            reasons: Vec::new(),
        };
        compare_candidates(&left_candidate, &right_candidate)
            .then_with(|| right.unknown_cells.cmp(&left.unknown_cells))
    });

    let matrix_capabilities: HashSet<_> = model.matrix.keys().collect();
    debug_assert!(
        debt.iter()
            .all(|item| matrix_capabilities.contains(&item.capability))
    );

    debt
}

fn select_top_checks(
    candidates: &[Candidate],
    limit: usize,
    max_per_capability: usize,
) -> Vec<&Candidate> {
    let mut selected = Vec::new();
    let mut per_capability: HashMap<&str, usize> = HashMap::new();

    for candidate in candidates {
        if selected.len() >= limit {
            break;
        }
        if candidate.research_channel != "market_research" {
            continue;
        }
        let count = per_capability
            .entry(candidate.capability.as_str())
            .or_default();
        if max_per_capability > 0 && *count >= max_per_capability {
            continue;
        }
        *count += 1;
        selected.push(candidate);
    }

    selected
}

fn render_text(
    model: &MarketModel,
    candidates: &[Candidate],
    debt: &[CapabilityDebt],
    top_checks: &[&Candidate],
) -> String {
    let external_cells = model.matrix_products.len().saturating_sub(1) * model.matrix.len();
    let unknown_cells = candidates.len();
    let known_cells = external_cells.saturating_sub(unknown_cells);

    let mut output = String::new();
    output.push_str("MCIF research debt\n\n");
    output.push_str(&format!(
        "External matrix coverage: {known_cells}/{external_cells} evidenced, {unknown_cells} unknown\n"
    ));
    let deferred_non_market_cells = candidates
        .iter()
        .filter(|candidate| candidate.research_channel != "market_research")
        .count();
    let deferred_non_market_capabilities = debt
        .iter()
        .filter(|item| item.research_channel != "market_research")
        .count();
    let first_party_cells = candidates
        .iter()
        .filter(|candidate| candidate.research_channel == "first_party_usage")
        .count();
    let first_party_capabilities = debt
        .iter()
        .filter(|item| item.research_channel == "first_party_usage")
        .count();
    let external_usage_cells = candidates
        .iter()
        .filter(|candidate| candidate.research_channel == "external_usage")
        .count();
    let external_usage_capabilities = debt
        .iter()
        .filter(|item| item.research_channel == "external_usage")
        .count();

    output.push_str(&format!(
        "Capabilities with unresolved external cells: {}\n",
        debt.len()
    ));
    output.push_str(&format!(
        "Deferred from market research: {deferred_non_market_capabilities} capabilities / {deferred_non_market_cells} cells\n"
    ));
    output.push_str(&format!(
        "- first_party_usage: {first_party_capabilities} capabilities / {first_party_cells} cells\n"
    ));
    output.push_str(&format!(
        "- external_usage: {external_usage_capabilities} capabilities / {external_usage_cells} cells\n\n"
    ));

    output.push_str("Capability queue\n");
    for item in debt.iter().take(12) {
        output.push_str(&format!(
            "- {} {}: unknown={} known={} horizon={} disposition={} hooktry={} channel={}{}{}\n",
            item.tier,
            item.capability,
            item.unknown_cells,
            item.known_external_peers,
            item.horizon,
            item.disposition,
            item.hooktry_status,
            item.research_channel,
            if item.direct_demand > 0 {
                format!(" direct_demand={}", item.direct_demand)
            } else {
                String::new()
            },
            if item.supporting_signals > 0 {
                format!(" supporting_signals={}", item.supporting_signals)
            } else {
                String::new()
            }
        ));
    }

    output.push_str(&format!("\nTop {} research checks\n", top_checks.len()));
    for (index, candidate) in top_checks.iter().enumerate() {
        output.push_str(&format!(
            "{}. {} {}/{} - {}\n",
            index + 1,
            candidate.tier,
            candidate.product,
            candidate.capability,
            candidate.reasons.join(", ")
        ));
    }

    output.push_str(
        "\nPriority semantics: decision-changing tier -> direct demand -> Hooktry gap -> horizon -> disposition -> supporting signals -> evidence scarcity -> product cohort breadth. No aggregate score is used.\n",
    );
    output
}

fn render_json(
    model: &MarketModel,
    candidates: &[Candidate],
    debt: &[CapabilityDebt],
    top_checks: &[&Candidate],
) -> String {
    let external_cells = model.matrix_products.len().saturating_sub(1) * model.matrix.len();
    let unknown_cells = candidates.len();
    let known_cells = external_cells.saturating_sub(unknown_cells);

    let deferred_non_market_cells = candidates
        .iter()
        .filter(|candidate| candidate.research_channel != "market_research")
        .count();
    let deferred_non_market_capabilities = debt
        .iter()
        .filter(|item| item.research_channel != "market_research")
        .count();
    let first_party_cells = candidates
        .iter()
        .filter(|candidate| candidate.research_channel == "first_party_usage")
        .count();
    let first_party_capabilities = debt
        .iter()
        .filter(|item| item.research_channel == "first_party_usage")
        .count();
    let external_usage_cells = candidates
        .iter()
        .filter(|candidate| candidate.research_channel == "external_usage")
        .count();
    let external_usage_capabilities = debt
        .iter()
        .filter(|item| item.research_channel == "external_usage")
        .count();

    let capability_json = debt
        .iter()
        .map(|item| {
            format!(
                "{{\"tier\":\"{}\",\"capability\":\"{}\",\"unknown_cells\":{},\"known_external_peers\":{},\"horizon\":\"{}\",\"disposition\":\"{}\",\"hooktry_status\":\"{}\",\"research_channel\":\"{}\",\"direct_demand\":{},\"supporting_signals\":{}}}",
                json_escape(&item.tier),
                json_escape(&item.capability),
                item.unknown_cells,
                item.known_external_peers,
                json_escape(&item.horizon),
                json_escape(&item.disposition),
                json_escape(&item.hooktry_status),
                json_escape(&item.research_channel),
                item.direct_demand,
                item.supporting_signals
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    let checks_json = top_checks
        .iter()
        .map(|candidate| {
            format!(
                "{{\"tier\":\"{}\",\"product\":\"{}\",\"capability\":\"{}\",\"horizon\":\"{}\",\"disposition\":\"{}\",\"hooktry_status\":\"{}\",\"research_channel\":\"{}\",\"direct_demand\":{},\"supporting_signals\":{},\"known_external_peers\":{},\"product_cohorts\":{},\"reasons\":{}}}",
                json_escape(&candidate.tier),
                json_escape(&candidate.product),
                json_escape(&candidate.capability),
                json_escape(&candidate.horizon),
                json_escape(&candidate.disposition),
                json_escape(&candidate.hooktry_status),
                json_escape(&candidate.research_channel),
                candidate.direct_demand,
                candidate.supporting_signals,
                candidate.known_external_peers,
                candidate.product_cohorts,
                json_array(&candidate.reasons)
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\"external_cells\":{external_cells},\"known_cells\":{known_cells},\"unknown_cells\":{unknown_cells},\"capabilities_with_debt\":{},\"deferred_non_market_capabilities\":{deferred_non_market_capabilities},\"deferred_non_market_cells\":{deferred_non_market_cells},\"deferred_to_first_party_capabilities\":{first_party_capabilities},\"deferred_to_first_party_cells\":{first_party_cells},\"deferred_to_external_usage_capabilities\":{external_usage_capabilities},\"deferred_to_external_usage_cells\":{external_usage_cells},\"capability_queue\":[{capability_json}],\"top_checks\":[{checks_json}]}}",
        debt.len()
    )
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
            } else if let Some(value) = line.strip_prefix("    hooktry_status: ") {
                item.hooktry_status = scalar(value);
            }
        }
    }

    if let Some(item) = current {
        items.push(item);
    }
    items
}

fn parse_products(text: &str) -> Vec<Product> {
    let mut items = Vec::new();
    let mut section = "";
    let mut current: Option<Product> = None;

    for line in text.lines() {
        if !line.starts_with(' ') && line.ends_with(':') {
            if let Some(item) = current.take() {
                items.push(item);
            }
            section = line.trim_end_matches(':');
            continue;
        }
        if section != "products" {
            continue;
        }

        if let Some(value) = line.strip_prefix("  - id: ") {
            if let Some(item) = current.take() {
                items.push(item);
            }
            current = Some(Product {
                id: scalar(value),
                ..Product::default()
            });
        } else if let Some(item) = current.as_mut()
            && let Some(value) = line.strip_prefix("    cohorts: ")
        {
            item.cohorts = inline_list(value);
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
        if line.starts_with("  - id: ") {
            if let Some(item) = current.take() {
                items.push(item);
            }
            current = Some(Priority::default());
        } else if let Some(item) = current.as_mut() {
            let trimmed = line.trim();
            if let Some(value) = trimmed.strip_prefix("horizon: ") {
                item.horizon = scalar(value);
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

fn parse_signals(text: &str) -> HashMap<String, SignalProfile> {
    let mut profiles: HashMap<String, SignalProfile> = HashMap::new();
    let mut capability = String::new();
    let mut class = String::new();

    let flush = |capability: &mut String,
                 class: &mut String,
                 profiles: &mut HashMap<String, SignalProfile>| {
        if capability.is_empty() || class.is_empty() {
            capability.clear();
            class.clear();
            return;
        }
        let profile = profiles.entry(capability.clone()).or_default();
        match class.as_str() {
            "direct_demand" => profile.direct_demand += 1,
            "demand_proxy" => profile.demand_proxy += 1,
            "market_motion" => profile.market_motion += 1,
            _ => {}
        }
        capability.clear();
        class.clear();
    };

    for line in text.lines() {
        if line.starts_with("  - id: ") {
            flush(&mut capability, &mut class, &mut profiles);
        } else {
            let trimmed = line.trim();
            if let Some(value) = trimmed.strip_prefix("capability: ") {
                capability = scalar(value);
            } else if let Some(value) = trimmed.strip_prefix("class: ") {
                class = scalar(value);
            }
        }
    }
    flush(&mut capability, &mut class, &mut profiles);

    profiles
}

fn parse_matrix(text: &str) -> (Vec<String>, BTreeMap<String, BTreeMap<String, String>>) {
    let mut products = Vec::new();
    let mut matrix: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut section = "";
    let mut capability = String::new();

    for line in text.lines() {
        if !line.starts_with(' ') && line.ends_with(':') {
            section = line.trim_end_matches(':');
            capability.clear();
            continue;
        }
        if section == "products" {
            if let Some(value) = line.strip_prefix("  - ") {
                products.push(scalar(value));
            }
        } else if section == "matrix" {
            if line.starts_with("  ") && !line.starts_with("    ") && line.ends_with(':') {
                capability = scalar(line.trim().trim_end_matches(':'));
                matrix.entry(capability.clone()).or_default();
            } else if line.starts_with("    ")
                && !capability.is_empty()
                && let Some((product, state)) = key_value(line.trim())
            {
                matrix
                    .entry(capability.clone())
                    .or_default()
                    .insert(product, state);
            }
        }
    }

    (products, matrix)
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

fn json_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", json_escape(value)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_now_gap_beats_completed_table_stake() {
        let now = Candidate {
            product: "a".to_owned(),
            capability: "x".to_owned(),
            tier: "P0".to_owned(),
            horizon: "now".to_owned(),
            disposition: "must".to_owned(),
            hooktry_status: "partial".to_owned(),
            direct_demand: 0,
            supporting_signals: 0,
            known_external_peers: 5,
            product_cohorts: 1,
            research_channel: "market_research".to_owned(),
            reasons: vec![],
        };
        let completed = Candidate {
            tier: "P4".to_owned(),
            direct_demand: 10,
            hooktry_status: "implemented".to_owned(),
            ..now.clone()
        };
        assert_eq!(compare_candidates(&now, &completed), Ordering::Less);
    }

    #[test]
    fn tiering_prioritizes_decision_change_over_existing_coverage() {
        assert_eq!(decision_tier("now", "partial", "must", 0, 0), "P0");
        assert_eq!(decision_tier("next", "absent", "should", 1, 0), "P1");
        assert_eq!(decision_tier("next", "unknown", "should", 0, 0), "P2");
        assert_eq!(decision_tier("next", "implemented", "must", 4, 0), "P4");
        assert_eq!(
            decision_tier("validate", "implemented", "differentiation", 0, 1),
            "P2"
        );
    }

    #[test]
    fn top_checks_are_diversified_by_capability() {
        let candidates = vec![
            Candidate {
                product: "a".to_owned(),
                capability: "search".to_owned(),
                tier: "P0".to_owned(),
                horizon: "now".to_owned(),
                disposition: "must".to_owned(),
                hooktry_status: "partial".to_owned(),
                direct_demand: 0,
                supporting_signals: 0,
                known_external_peers: 1,
                product_cohorts: 1,
                research_channel: "market_research".to_owned(),
                reasons: vec![],
            },
            Candidate {
                product: "b".to_owned(),
                capability: "search".to_owned(),
                tier: "P0".to_owned(),
                horizon: "now".to_owned(),
                disposition: "must".to_owned(),
                hooktry_status: "partial".to_owned(),
                direct_demand: 0,
                supporting_signals: 0,
                known_external_peers: 1,
                product_cohorts: 1,
                research_channel: "market_research".to_owned(),
                reasons: vec![],
            },
            Candidate {
                product: "c".to_owned(),
                capability: "response".to_owned(),
                tier: "P0".to_owned(),
                horizon: "now".to_owned(),
                disposition: "must".to_owned(),
                hooktry_status: "partial".to_owned(),
                direct_demand: 0,
                supporting_signals: 0,
                known_external_peers: 1,
                product_cohorts: 1,
                research_channel: "market_research".to_owned(),
                reasons: vec![],
            },
        ];

        let selected = select_top_checks(&candidates, 3, 1);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].capability, "search");
        assert_eq!(selected[1].capability, "response");
    }

    #[test]
    fn first_party_evidence_candidates_are_not_external_research_checks() {
        let candidate = Candidate {
            product: "hookdeck".to_owned(),
            capability: "ordering".to_owned(),
            tier: "P2".to_owned(),
            horizon: "validate".to_owned(),
            disposition: "differentiation".to_owned(),
            hooktry_status: "implemented".to_owned(),
            direct_demand: 2,
            supporting_signals: 1,
            known_external_peers: 2,
            product_cohorts: 1,
            research_channel: "first_party_usage".to_owned(),
            reasons: vec![],
        };

        assert!(select_top_checks(&[candidate], 20, 3).is_empty());
    }

    #[test]
    fn external_usage_candidates_are_not_external_research_checks() {
        let candidate = Candidate {
            product: "hookdeck".to_owned(),
            capability: "cardinality".to_owned(),
            tier: "P2".to_owned(),
            horizon: "validate".to_owned(),
            disposition: "differentiation".to_owned(),
            hooktry_status: "implemented".to_owned(),
            direct_demand: 2,
            supporting_signals: 1,
            known_external_peers: 2,
            product_cohorts: 1,
            research_channel: "external_usage".to_owned(),
            reasons: vec![],
        };

        assert!(select_top_checks(&[candidate], 20, 3).is_empty());
    }

    #[test]
    fn direct_demand_breaks_same_horizon_tie() {
        let no_signal = Candidate {
            product: "a".to_owned(),
            capability: "x".to_owned(),
            tier: "P1".to_owned(),
            horizon: "next".to_owned(),
            disposition: "should".to_owned(),
            hooktry_status: "unknown".to_owned(),
            direct_demand: 0,
            supporting_signals: 0,
            known_external_peers: 1,
            product_cohorts: 1,
            research_channel: "market_research".to_owned(),
            reasons: vec![],
        };
        let demanded = Candidate {
            direct_demand: 1,
            ..no_signal.clone()
        };
        assert_eq!(compare_candidates(&demanded, &no_signal), Ordering::Less);
    }

    #[test]
    fn unresolved_hooktry_gap_breaks_signal_tie() {
        let implemented = Candidate {
            product: "a".to_owned(),
            capability: "x".to_owned(),
            tier: "P0".to_owned(),
            horizon: "now".to_owned(),
            disposition: "must".to_owned(),
            hooktry_status: "implemented".to_owned(),
            direct_demand: 0,
            supporting_signals: 0,
            known_external_peers: 1,
            product_cohorts: 1,
            research_channel: "market_research".to_owned(),
            reasons: vec![],
        };
        let absent = Candidate {
            hooktry_status: "absent".to_owned(),
            ..implemented.clone()
        };
        assert_eq!(compare_candidates(&absent, &implemented), Ordering::Less);
    }
}
