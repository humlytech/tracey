//! Shared query execution over a `DashboardData` snapshot.
//!
//! Maps query requests to proto responses without any transport. Used both by
//! the daemon service (over roam RPC) and by the in-process `--no-daemon` query
//! backend, so the two paths produce byte-identical results.

use std::path::Path;

use tracey_core::RuleId;
use tracey_proto::*;

use crate::data::DashboardData;
use crate::server::QueryEngine;

/// Resolve spec/impl from optional parameters, defaulting to the first spec and
/// its first implementation.
pub fn resolve_spec_impl(
    spec: Option<&str>,
    impl_name: Option<&str>,
    config: &ApiConfig,
) -> (String, String) {
    let spec_name = spec.map(String::from).unwrap_or_else(|| {
        config
            .specs
            .first()
            .map(|s| s.name.clone())
            .unwrap_or_default()
    });

    let impl_name = impl_name.map(String::from).unwrap_or_else(|| {
        config
            .specs
            .iter()
            .find(|s| s.name == spec_name)
            .and_then(|s| s.implementations.first().cloned())
            .unwrap_or_default()
    });

    (spec_name, impl_name)
}

/// Coverage status for all specs/impls.
pub fn status(data: &DashboardData) -> StatusResponse {
    let query = QueryEngine::new(data);
    let stats = query.status();

    StatusResponse {
        impls: stats
            .into_iter()
            .map(|(spec, impl_name, s)| ImplStatus {
                spec,
                impl_name,
                total_rules: s.total_rules,
                covered_rules: s.impl_covered,
                stale_rules: s.stale_covered,
                verified_rules: s.verify_covered,
            })
            .collect(),
    }
}

/// Uncovered rules for a spec/impl.
pub fn uncovered(data: &DashboardData, req: UncoveredRequest) -> UncoveredResponse {
    let query = QueryEngine::new(data);
    let (spec, impl_name) =
        resolve_spec_impl(req.spec.as_deref(), req.impl_name.as_deref(), &data.config);

    if let Some(result) = query.uncovered(&spec, &impl_name, req.prefix.as_deref()) {
        UncoveredResponse {
            spec: result.spec,
            impl_name: result.impl_name,
            total_rules: result.stats.total_rules,
            uncovered_count: result.total_uncovered,
            by_section: result
                .by_section
                .into_iter()
                .map(|(section, rules)| SectionRules {
                    section,
                    rules: rules
                        .into_iter()
                        .map(|r| tracey_proto::RuleRef {
                            id: r.id,
                            text: None,
                        })
                        .collect(),
                })
                .collect(),
        }
    } else {
        UncoveredResponse {
            spec,
            impl_name,
            total_rules: 0,
            uncovered_count: 0,
            by_section: vec![],
        }
    }
}

/// Untested rules for a spec/impl.
pub fn untested(data: &DashboardData, req: UntestedRequest) -> UntestedResponse {
    let query = QueryEngine::new(data);
    let (spec, impl_name) =
        resolve_spec_impl(req.spec.as_deref(), req.impl_name.as_deref(), &data.config);

    if let Some(result) = query.untested(&spec, &impl_name, req.prefix.as_deref()) {
        UntestedResponse {
            spec: result.spec,
            impl_name: result.impl_name,
            total_rules: result.stats.total_rules,
            untested_count: result.total_untested,
            by_section: result
                .by_section
                .into_iter()
                .map(|(section, rules)| SectionRules {
                    section,
                    rules: rules
                        .into_iter()
                        .map(|r| tracey_proto::RuleRef {
                            id: r.id,
                            text: None,
                        })
                        .collect(),
                })
                .collect(),
        }
    } else {
        UntestedResponse {
            spec,
            impl_name,
            total_rules: 0,
            untested_count: 0,
            by_section: vec![],
        }
    }
}

/// Stale references for a spec/impl.
pub fn stale(data: &DashboardData, req: StaleRequest) -> StaleResponse {
    let query = QueryEngine::new(data);
    let (spec, impl_name) =
        resolve_spec_impl(req.spec.as_deref(), req.impl_name.as_deref(), &data.config);

    if let Some(result) = query.stale(&spec, &impl_name, req.prefix.as_deref()) {
        StaleResponse {
            spec: result.spec,
            impl_name: result.impl_name,
            total_rules: result.stats.total_rules,
            stale_count: result.entries.len(),
            refs: result
                .entries
                .into_iter()
                .map(|e| StaleEntry {
                    current_id: e.current_id,
                    file: e.file,
                    line: e.line,
                    reference_id: e.reference_id,
                })
                .collect(),
        }
    } else {
        StaleResponse {
            spec,
            impl_name,
            total_rules: 0,
            stale_count: 0,
            refs: vec![],
        }
    }
}

/// Unmapped code units for a spec/impl.
pub fn unmapped(data: &DashboardData, req: UnmappedRequest) -> UnmappedResponse {
    let query = QueryEngine::new(data);
    let (spec, impl_name) =
        resolve_spec_impl(req.spec.as_deref(), req.impl_name.as_deref(), &data.config);

    if let Some(result) = query.unmapped(&spec, &impl_name, req.path.as_deref()) {
        // Convert tree nodes to flat entries
        let mut entries = Vec::new();
        fn flatten_tree(node: &crate::server::FileTreeNode, entries: &mut Vec<UnmappedEntry>) {
            entries.push(UnmappedEntry {
                path: node.path.clone(),
                is_dir: node.is_dir,
                total_units: node.total_units,
                unmapped_units: node.total_units.saturating_sub(node.covered_units),
                units: vec![], // Tree nodes don't have unit details
            });
            for child in &node.children {
                flatten_tree(child, entries);
            }
        }
        for node in &result.tree {
            flatten_tree(node, &mut entries);
        }

        // If we have file details, add those units
        if let Some(details) = &result.file_details {
            // Find the entry for this file and update its units
            if let Some(entry) = entries.iter_mut().find(|e| e.path == details.path) {
                entry.units = details
                    .units
                    .iter()
                    .filter(|u| !u.is_covered)
                    .map(|u| UnmappedUnit {
                        kind: u.kind.clone(),
                        name: u.name.clone(),
                        start_line: u.start_line,
                        end_line: u.end_line,
                    })
                    .collect();
            }
        }

        UnmappedResponse {
            spec: result.spec,
            impl_name: result.impl_name,
            total_units: result.total_units,
            unmapped_count: result.total_units.saturating_sub(result.covered_units),
            entries,
        }
    } else {
        UnmappedResponse {
            spec,
            impl_name,
            total_units: 0,
            unmapped_count: 0,
            entries: vec![],
        }
    }
}

/// Current configuration.
pub fn config(data: &DashboardData) -> ApiConfig {
    data.config.clone()
}

/// Validation results for a spec/impl.
pub fn validate(data: &DashboardData, req: ValidateRequest) -> ValidationResult {
    let (spec, impl_name) =
        resolve_spec_impl(req.spec.as_deref(), req.impl_name.as_deref(), &data.config);

    data.validation_by_impl
        .get(&(spec.clone(), impl_name.clone()))
        .cloned()
        .unwrap_or_else(|| ValidationResult {
            spec,
            impl_name,
            errors: Vec::new(),
            warning_count: 0,
            error_count: 0,
        })
}

/// Details for a specific rule, including a version diff when the references are
/// stale (computed from git history).
pub async fn rule(
    data: &DashboardData,
    project_root: &Path,
    rule_id: RuleId,
) -> Option<RuleInfo> {
    let query = QueryEngine::new(data);
    let info = query.rule(&rule_id)?;

    // Compute version diff only when references are stale
    let version_diff = if info.is_stale && info.id.version > 1 {
        let prev_id = RuleId::new(info.id.base.clone(), info.id.version - 1)
            .expect("version - 1 >= 1 since version > 1");
        if let Some(source_file) = info.source_file.as_deref() {
            crate::daemon::service::load_previous_rule_text_from_git(
                project_root,
                source_file,
                &prev_id,
            )
            .await
            .map(|historical| marq::diff_markdown_inline(&historical.text, &info.raw))
        } else {
            None
        }
    } else {
        None
    };

    Some(RuleInfo {
        id: info.id,
        raw: info.raw,
        html: info.html,
        source_file: info.source_file,
        source_line: info.source_line,
        coverage: info
            .coverage
            .into_iter()
            .map(|c| RuleCoverage {
                spec: c.spec,
                impl_name: c.impl_name,
                impl_refs: c.impl_refs,
                verify_refs: c.verify_refs,
            })
            .collect(),
        version_diff,
    })
}
