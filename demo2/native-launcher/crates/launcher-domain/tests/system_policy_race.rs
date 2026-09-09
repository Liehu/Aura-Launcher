//! P2.9 Batch 6 closure tests: race (concurrent resolution), security
//! (origin spoofing / policy bypass attempts) and fault injection over the
//! SystemResolver + taxonomy — all pure-type, deterministic.

use launcher_domain::system::{
    SystemCapability, SystemCommand, SystemPolicy, SystemResolver, SystemRisk, SystemTarget,
};

fn cmd(origin: &str, op: &str, risk: SystemRisk) -> SystemCommand {
    SystemCommand {
        capability: SystemCapability::Process,
        operation: op.into(),
        target: SystemTarget::Process {
            pid: 1,
            name: "x".into(),
        },
        risk,
        origin: origin.into(),
    }
}

fn resolver() -> SystemResolver {
    let mut r = SystemResolver::default();
    r.policy.allowed_origins = vec!["search".into(), "workflow".into()];
    r.policy
        .origin_risk_ceiling
        .push(("search".into(), SystemRisk::Reversible));
    r
}

/// Security: origins outside the allow-list are denied and the denial
/// records the claimed origin (no silent pass-through).
#[test]
fn security_disallowed_origin_denied_with_propagation() {
    let r = resolver();
    let mut c = cmd("evil-plugin", "kill", SystemRisk::Destructive);
    c.origin = "evil-plugin".into();
    let res = r.resolve(&c).unwrap();
    assert!(!res.approved_by_policy);
    assert_eq!(res.origin, "evil-plugin");
}

/// Security: ceiling bypass via origin renaming is impossible — the ceiling
/// follows the origin string, so a plugin claiming to be "search" IS "search"
/// and stays under its ceiling.
#[test]
fn security_ceiling_follows_origin() {
    let r = resolver();
    let mut c = cmd("search", "kill", SystemRisk::Destructive);
    c.origin = "search".into(); // allowed origin, but destructive
    let res = r.resolve(&c).unwrap();
    assert!(
        !res.approved_by_policy,
        "destructive op exceeds search's Reversible ceiling"
    );
}

/// Race: concurrent resolution from many threads — decisions stay consistent
/// (the resolver is read-only over its policy).
#[test]
fn race_concurrent_resolution_consistent() {
    let r = Arc::new(resolver());
    let failures = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let r = r.clone();
        let failures = failures.clone();
        handles.push(std::thread::spawn(move || {
            for _ in 0..200 {
                match r.resolve(&cmd("search", "focus", SystemRisk::Reversible)) {
                    Ok(x)
                        if x.approved_by_policy
                            && x.reason == "allowed"
                            && x.origin == "search" => {}
                    _ => {
                        failures.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        }));
    }
    for h in handles {
        h.join().expect("thread");
    }
    assert_eq!(failures.load(Ordering::SeqCst), 0);
}

/// Fault injection: invalid commands (bad operation shape, empty origin,
/// empty file target) all fail validation outright.
#[test]
fn fault_invalid_commands_rejected() {
    let r = SystemResolver::default();
    let mut c = cmd("search", "FOO", SystemRisk::Info);
    assert!(r.resolve(&c).is_err(), "non-identifier operation rejected");
    c.operation = "focus".into();
    c.origin = String::new();
    assert!(r.resolve(&c).is_err(), "empty origin rejected");
    c.origin = "search".into();
    c.target = SystemTarget::File {
        path: "  ".into(),
    };
    c.capability = SystemCapability::File;
    assert!(r.resolve(&c).is_err(), "empty file target rejected");
}

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
