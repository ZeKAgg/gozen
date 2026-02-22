use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use gozen_lsp::project_queries::{
    completion_context_at_position, extract_node_path_at_position, gather_signal_usage_locations,
    node_path_completions_for_prefix, resolve_node_definition_locations, uri_to_res_path,
    CompletionContextKind,
};
use gozen_project::ProjectGraph;
use tower_lsp::lsp_types::Position;
use url::Url;

#[test]
fn extracts_node_path_for_get_node_string() {
    let src = "func _ready():\n\tvar n = get_node(\"UI/Hud\")\n";
    let line = src.lines().nth(1).unwrap();
    let col = line.find("UI/Hud").unwrap() + 2;
    let path = extract_node_path_at_position(
        src,
        Position {
            line: 1,
            character: col as u32,
        },
    );
    assert_eq!(path.as_deref(), Some("UI/Hud"));
}

#[test]
fn completion_context_for_group_call() {
    let src = "func _ready():\n\tis_in_group(\"enemy\")\n";
    let line = src.lines().nth(1).unwrap();
    let col = line.find("enemy").unwrap() + 3;
    let ctx = completion_context_at_position(
        src,
        Position {
            line: 1,
            character: col as u32,
        },
    );
    assert!(matches!(ctx, Some(CompletionContextKind::Group { .. })));
}

#[test]
fn uri_maps_to_res_path_under_root() {
    let root = PathBuf::from("/tmp/gozen_root");
    let file = root.join("scripts/player.gd");
    let uri = Url::from_file_path(file).unwrap();
    let res = uri_to_res_path(&uri, &root);
    assert_eq!(res.as_deref(), Some("res://scripts/player.gd"));
}

fn tmp_project_dir(prefix: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{}_{}", prefix, ts));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn resolves_node_definition_and_completions_from_project_graph() {
    let root = tmp_project_dir("gozen_lsp_node");
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::create_dir_all(root.join("scenes")).unwrap();
    fs::write(
        root.join("project.godot"),
        "[application]\nconfig/name=\"x\"\n",
    )
    .unwrap();
    fs::write(
        root.join("scripts/main.gd"),
        "extends Node\nfunc _ready():\n\tvar n = get_node(\"UI\")\n",
    )
    .unwrap();
    fs::write(
        root.join("scenes/main.tscn"),
        "[gd_scene load_steps=2 format=3]\n\
[ext_resource type=\"Script\" path=\"res://scripts/main.gd\" id=\"1\"]\n\
[node name=\"Root\" type=\"Node2D\"]\n\
script = ExtResource(\"1\")\n\
[node name=\"UI\" type=\"Node2D\" parent=\".\"]\n\
[node name=\"Hud\" type=\"Node2D\" parent=\"UI\"]\n",
    )
    .unwrap();

    let graph = ProjectGraph::build(&root).unwrap();
    let locs = resolve_node_definition_locations(&graph, &root, "res://scripts/main.gd", "UI");
    assert_eq!(locs.len(), 1);
    assert!(locs[0].uri.path().ends_with("/scenes/main.tscn"));

    let comps = node_path_completions_for_prefix(&graph, "res://scripts/main.gd", "U");
    assert!(comps.iter().any(|c| c == "UI"));
}

#[test]
fn gathers_signal_usages_from_emit_connect_and_scene_connection() {
    let root = tmp_project_dir("gozen_lsp_signal");
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::create_dir_all(root.join("scenes")).unwrap();
    fs::write(
        root.join("project.godot"),
        "[application]\nconfig/name=\"x\"\n",
    )
    .unwrap();
    fs::write(
        root.join("scripts/a.gd"),
        "extends Node\nsignal hit\nfunc _ready():\n\thit.emit()\n\temit_signal(\"hit\")\n",
    )
    .unwrap();
    fs::write(
        root.join("scripts/b.gd"),
        "extends Node\nfunc _ready():\n\tconnect(\"hit\", Callable(self, \"_on_hit\"))\n",
    )
    .unwrap();
    fs::write(
        root.join("scenes/main.tscn"),
        "[gd_scene load_steps=1 format=3]\n\
[node name=\"Root\" type=\"Node\"]\n\
[connection signal=\"hit\" from=\".\" to=\".\" method=\"_on_hit\"]\n",
    )
    .unwrap();

    let graph = ProjectGraph::build(&root).unwrap();
    let usages = gather_signal_usage_locations(&graph, &root, &HashMap::new(), "hit");
    assert!(
        usages.len() >= 3,
        "expected emit/connect/scene usages, got {}",
        usages.len()
    );
    assert!(
        usages
            .iter()
            .any(|l| l.uri.path().ends_with("/scenes/main.tscn")),
        "expected at least one scene connection usage"
    );
}
