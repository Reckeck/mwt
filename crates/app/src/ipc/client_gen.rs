//! The typed TypeScript client, generated from the `#[ipc]` command metadata.
//! Only the pilot module has metadata so far; the other commands keep their
//! hand-written wrappers. The test compares the committed file with the
//! generated text; `UPDATE_CLIENT=1 cargo test -p trade-app client_gen`
//! rewrites it.

use std::fmt::Write;
use std::path::PathBuf;

use crate::commands::workflows;
use crate::ipc::run::CommandMeta;

/// Every `#[ipc]` command that has a generated client: the pilot module.
const CLIENT_COMMANDS: &[&CommandMeta] = &[
    &workflows::__IPC_META_list_workflows,
    &workflows::__IPC_META_get_workflow,
    &workflows::__IPC_META_save_workflow,
    &workflows::__IPC_META_save_workflow_draft,
    &workflows::__IPC_META_discard_workflow_draft,
    &workflows::__IPC_META_set_workflow_active,
    &workflows::__IPC_META_delete_workflow,
    &workflows::__IPC_META_restore_workflow_version,
    &workflows::__IPC_META_dry_run_workflow,
    &workflows::__IPC_META_list_workflow_runs,
    &workflows::__IPC_META_list_workflow_queue,
    &workflows::__IPC_META_discard_workflow_queued,
    &workflows::__IPC_META_list_workflow_waits,
    &workflows::__IPC_META_list_workflow_vars,
    &workflows::__IPC_META_set_workflow_var,
    &workflows::__IPC_META_delete_workflow_var,
    &workflows::__IPC_META_export_workflow,
    &workflows::__IPC_META_import_workflow,
    &workflows::__IPC_META_preview_workflow_import,
    &workflows::__IPC_META_workflow_templates,
    &workflows::__IPC_META_workflow_from_template,
    &workflows::__IPC_META_save_workflow_test,
    &workflows::__IPC_META_delete_workflow_test,
    &workflows::__IPC_META_run_workflow_tests,
];

const HEADER: &str = "// AUTO-GENERATED from the #[ipc] command metadata. Do not edit manually.\n\
// Regenerate: UPDATE_CLIENT=1 cargo test -p trade-app client_gen\n\
import { call } from './ipc'\n";

/// `snake_case` to `camelCase`, the spelling Tauri expects for argument keys.
fn camel_case(snake: &str) -> String {
    let mut out = String::new();
    let mut upper_next = false;
    for character in snake.chars() {
        if character == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(character.to_uppercase());
            upper_next = false;
        } else {
            out.push(character);
        }
    }
    out
}

/// One exported function for one command.
fn render_command(meta: &CommandMeta) -> String {
    let ret = (meta.ret)();
    let params: Vec<String> = meta
        .args
        .iter()
        .map(|arg| format!("{}: {}", camel_case(arg.name), (arg.ts)()))
        .collect();
    let keys: Vec<String> = meta.args.iter().map(|arg| camel_case(arg.name)).collect();
    let payload = if keys.is_empty() {
        String::new()
    } else {
        format!(", {{ {} }}", keys.join(", "))
    };
    format!(
        "export function {}({}): Promise<{ret}> {{\n  return call<{ret}>('{}'{payload})\n}}\n",
        camel_case(meta.name),
        params.join(", "),
        meta.name,
    )
}

/// The whole text of `client.gen.ts`.
fn render_client(commands: &[&CommandMeta]) -> String {
    let mut text = String::from(HEADER);
    for meta in commands {
        write!(text, "\n{}", render_command(meta)).expect("write to a String");
    }
    text
}

fn client_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src/shared/api/client.gen.ts")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_case_joins_the_words() {
        assert_eq!(camel_case("list_workflow_runs"), "listWorkflowRuns");
        assert_eq!(camel_case("id"), "id");
    }

    #[test]
    fn a_command_with_arguments_passes_them_by_camel_case_key() {
        let text = render_command(&workflows::__IPC_META_get_workflow);
        assert!(text.contains("export function getWorkflow(id: string): Promise<Workflow>"));
        assert!(text.contains("call<Workflow>('get_workflow', { id })"));
    }

    #[test]
    fn a_command_without_arguments_sends_no_payload() {
        let text = render_command(&workflows::__IPC_META_list_workflows);
        assert!(text.contains("listWorkflows(): Promise<"));
        assert!(text.contains("('list_workflows')"));
    }

    #[test]
    fn client_gen_is_current() {
        let generated = render_client(CLIENT_COMMANDS);
        let path = client_path();
        if std::env::var_os("UPDATE_CLIENT").is_some() {
            std::fs::write(&path, &generated).expect("write client.gen.ts");
            return;
        }
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed == generated,
            "src/shared/api/client.gen.ts is stale: run `UPDATE_CLIENT=1 cargo test -p trade-app client_gen`"
        );
    }
}
