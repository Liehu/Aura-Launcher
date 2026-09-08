import io

p = "apps/launcher-app/src/visual_scenarios.rs"
s = io.open(p, encoding="utf-8").read()

# trim unused imports
s = s.replace(
"""use launcher_domain::{
    Action, ActionDescriptor, ActionKind, ActionPayload, Category, Command, StepRun,
    StepRunStatus, WorkflowAction, WorkflowRun, WorkflowRunStatus, WorkflowStep,
};""",
"use launcher_domain::{Category, Command, StepRun, StepRunStatus};")

# drop unused confirm param
s = s.replace(
"    let action = |id: &str, title: &str, enabled: bool, confirm: bool| {",
"    let action = |id: &str, title: &str, enabled: bool| {")

io.open(p, "w", encoding="utf-8", newline="\n").write(s)
print("ok")
