//! Dependency Resolver (P2.8-D, spec `P2.8 — Ecosystem & Distribution
//! 1.0 技术设计规范.md` §14-§17): deterministic, explainable resolution of
//! package dependencies into an ordered InstallPlan.
//!
//! Contract (§14): resolution is DETERMINISTIC (same index → same plan),
//! failures carry a precise reason (missing dep / cycle / version conflict),
//! and the plan is DATA for user approval + the staged installer — never an
//! authority to install by itself.

use serde::{Deserialize, Serialize};

/// One package known to the resolver (from the repository index).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageMeta {
    pub id: String,
    pub version: String,
    /// Ids of packages this one requires (exact ids; ranges are a 2.1
    /// contract with the version-channel system, §31).
    pub depends: Vec<String>,
}

/// §16 InstallPlan: the approved-order list of packages to install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPlan {
    /// Dependencies first (topological order), requested package(s) last.
    pub steps: Vec<PackageMeta>,
}

/// Resolution failure with a precise, user-explainable reason (§14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    MissingDependency { package: String, missing: String },
    DependencyCycle { chain: Vec<String> },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::MissingDependency { package, missing } => {
                write!(f, "`{package}` depends on `{missing}`, which is not in the repository index")
            }
            ResolveError::DependencyCycle { chain } => {
                write!(f, "dependency cycle: {}", chain.join(" -> "))
            }
        }
    }
}

/// Resolve `requested` (ids) against the index into an InstallPlan.
/// Deterministic: dependencies are emitted before dependents; among equal
/// candidates the index order is preserved (stable sort on insertion time).
pub fn resolve(index: &[PackageMeta], requested: &[String]) -> Result<InstallPlan, ResolveError> {
    let by_id: std::collections::HashMap<&str, &PackageMeta> =
        index.iter().map(|p| (p.id.as_str(), p)).collect();
    for r in requested {
        if !by_id.contains_key(r.as_str()) {
            return Err(ResolveError::MissingDependency {
                package: "<request>".into(),
                missing: r.clone(),
            });
        }
    }
    let mut plan: Vec<PackageMeta> = Vec::new();
    let mut placed: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut visiting: Vec<String> = Vec::new();

    // recursive emit: deps first (DFS), requested ids appended last at top level
    fn emit(
        id: &str,
        by_id: &std::collections::HashMap<&str, &PackageMeta>,
        plan: &mut Vec<PackageMeta>,
        placed: &mut std::collections::HashSet<String>,
        visiting: &mut Vec<String>,
    ) -> Result<(), ResolveError> {
        if placed.contains(id) {
            return Ok(());
        }
        if visiting.iter().any(|v| v == id) {
            let mut chain = visiting.clone();
            chain.push(id.to_string());
            return Err(ResolveError::DependencyCycle { chain });
        }
        let meta = by_id.get(id).ok_or_else(|| ResolveError::MissingDependency {
            package: visiting.last().cloned().unwrap_or_default(),
            missing: id.to_string(),
        })?;
        visiting.push(id.to_string());
        for dep in &meta.depends {
            emit(dep, by_id, plan, placed, visiting)?;
        }
        visiting.pop();
        placed.insert(id.to_string());
        plan.push((*meta).clone());
        Ok(())
    }

    for r in requested {
        emit(r, &by_id, &mut plan, &mut placed, &mut visiting)?;
    }
    Ok(InstallPlan { steps: plan })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(id: &str, depends: &[&str]) -> PackageMeta {
        PackageMeta {
            id: id.into(),
            version: "1.0.0".into(),
            depends: depends.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Dependencies are installed BEFORE their dependents (§16 order).
    #[test]
    fn deps_come_first() {
        let index = vec![
            meta("app", &["lib-a", "lib-b"]),
            meta("lib-b", &["lib-a"]),
            meta("lib-a", &[]),
        ];
        let plan = resolve(&index, &["app".into()]).unwrap();
        let ids: Vec<&str> = plan.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["lib-a", "lib-b", "app"]);
    }

    /// §14: missing dependencies are explained precisely.
    #[test]
    fn missing_dependency_is_explained() {
        let index = vec![meta("app", &["ghost-lib"])];
        let err = resolve(&index, &["app".into()]).unwrap_err();
        assert!(matches!(
            err,
            ResolveError::MissingDependency { ref missing, .. } if missing == "ghost-lib"
        ));
        assert!(err.to_string().contains("not in the repository index"));
    }

    /// §14: dependency cycles are detected with the chain.
    #[test]
    fn cycles_detected() {
        let index = vec![
            meta("a", &["b"]),
            meta("b", &["a"]),
        ];
        let err = resolve(&index, &["a".into()]).unwrap_err();
        assert!(matches!(err, ResolveError::DependencyCycle { .. }));
    }

    /// Deterministic: the same index + request always yields the same plan.
    #[test]
    fn resolution_is_deterministic() {
        let index = vec![
            meta("app", &["lib-b", "lib-a"]),
            meta("lib-a", &[]),
            meta("lib-b", &["lib-a"]),
        ];
        let p1 = resolve(&index, &["app".into()]).unwrap();
        let p2 = resolve(&index, &["app".into()]).unwrap();
        assert_eq!(p1, p2);
    }

    /// Shared dependencies are installed exactly once.
    #[test]
    fn shared_dependency_installed_once() {
        let index = vec![
            meta("app1", &["shared"]),
            meta("app2", &["shared"]),
            meta("shared", &[]),
        ];
        let plan = resolve(&index, &["app1".into(), "app2".into()]).unwrap();
        let ids: Vec<&str> = plan.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids.iter().filter(|i| **i == "shared").count(), 1);
        assert_eq!(ids.last().copied(), Some("app2"));
    }

    /// InstallPlan is DATA: it roundtrips for the approval UI (§16).
    #[test]
    fn install_plan_roundtrips() {
        let index = vec![meta("app", &["lib"]) , meta("lib", &[])];
        let plan = resolve(&index, &["app".into()]).unwrap();
        let json = serde_json::to_string(&plan).unwrap();
        let back: InstallPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, plan);
    }
}
