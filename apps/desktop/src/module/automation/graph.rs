use lilia_feature_automation::AutomationWorkflow;
use nana_ui::{
    GraphEdge as CanvasGraphEdge, GraphEndpoint, GraphModel, GraphNode as CanvasGraphNode,
    GraphPoint, GraphPort, GraphPortKind, GraphPortSide, GraphRect, GraphSize, GraphViewport,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn initial_viewport(bounds: GraphRect) -> GraphViewport {
    GraphViewport::new(
        GraphPoint::new(24.0 - bounds.origin.x, 24.0 - bounds.origin.y),
        1.0,
    )
}

pub(crate) fn automation_graph_model(workflow: &AutomationWorkflow) -> Result<GraphModel, String> {
    let mut input_ports = BTreeMap::<String, BTreeSet<String>>::new();
    let mut output_ports = BTreeMap::<String, BTreeSet<String>>::new();
    for node in &workflow.draft.nodes {
        if node.kind != "trigger" {
            input_ports
                .entry(node.id.clone())
                .or_default()
                .insert("input".to_owned());
        }
        if node.kind != "logic" {
            output_ports
                .entry(node.id.clone())
                .or_default()
                .insert("output".to_owned());
        }
        if node.kind == "logic" {
            let ports = output_ports.entry(node.id.clone()).or_default();
            match node
                .config
                .get("logic")
                .and_then(Value::as_str)
                .unwrap_or("condition")
            {
                "condition" => {
                    ports.extend(["true".to_owned(), "false".to_owned()]);
                }
                "switch" => {
                    ports.insert("default".to_owned());
                    if let Some(cases) = node.config.get("cases") {
                        let cases: lilia_contracts::AutomationSwitchCases =
                            serde_json::from_value(cases.clone())
                                .map_err(|_| "分支需要填写匹配值列表。".to_owned())?;
                        ports.extend(
                            cases
                                .values()
                                .iter()
                                .map(|value| {
                                    lilia_feature_automation::automation_json_value_to_port(
                                        &Value::String(value.clone()),
                                    )
                                })
                                .filter(|value| !value.is_empty()),
                        );
                    }
                }
                _ => {}
            }
        }
    }
    for edge in &workflow.draft.edges {
        output_ports
            .entry(edge.source.clone())
            .or_default()
            .insert(edge.source_handle.as_deref().unwrap_or("output").to_owned());
        input_ports
            .entry(edge.target.clone())
            .or_default()
            .insert(edge.target_handle.as_deref().unwrap_or("input").to_owned());
    }

    let nodes = workflow
        .draft
        .nodes
        .iter()
        .map(|node| {
            let mut graph_node = CanvasGraphNode::new(
                node.id.clone(),
                node.title.clone(),
                GraphPoint::new(node.position.x as f32, node.position.y as f32),
                GraphSize::new(184.0, 112.0),
            );
            for port in input_ports.get(&node.id).into_iter().flatten() {
                graph_node = graph_node.with_port(GraphPort::new(
                    format!("in:{port}"),
                    port_label(port),
                    GraphPortKind::Input,
                    GraphPortSide::Left,
                ));
            }
            for port in output_ports.get(&node.id).into_iter().flatten() {
                graph_node = graph_node.with_port(GraphPort::new(
                    format!("out:{port}"),
                    port_label(port),
                    GraphPortKind::Output,
                    GraphPortSide::Right,
                ));
            }
            graph_node
        })
        .collect::<Vec<_>>();
    let edges = workflow
        .draft
        .edges
        .iter()
        .map(|edge| {
            let source_handle = edge.source_handle.as_deref().unwrap_or("output");
            let target_handle = edge.target_handle.as_deref().unwrap_or("input");
            CanvasGraphEdge::new(
                edge.id.clone(),
                GraphEndpoint::new(edge.source.clone(), format!("out:{source_handle}")),
                GraphEndpoint::new(edge.target.clone(), format!("in:{target_handle}")),
            )
            .with_label(port_label(source_handle))
        })
        .collect::<Vec<_>>();
    GraphModel::new(nodes, edges).map_err(|error| error.to_string())
}

fn port_label(port: &str) -> &str {
    match port {
        "input" => "输入",
        "output" => "继续",
        "true" => "成立",
        "false" => "不成立",
        "default" => "其他",
        _ => port,
    }
}

pub(crate) fn new_connection(
    workflow: &AutomationWorkflow,
    source: GraphEndpoint,
    target: GraphEndpoint,
) -> Result<Option<lilia_feature_automation::AutomationEdge>, String> {
    let graph = automation_graph_model(workflow)?;
    for (endpoint, kind) in [
        (&source, GraphPortKind::Output),
        (&target, GraphPortKind::Input),
    ] {
        if !graph.nodes().iter().any(|node| {
            node.id == endpoint.node
                && node
                    .ports
                    .iter()
                    .any(|port| port.id == endpoint.port && port.kind == kind)
        }) {
            return Err("连接端点已不存在，请重新选择。".into());
        }
    }
    let source_handle = source
        .port
        .as_str()
        .strip_prefix("out:")
        .unwrap_or(source.port.as_str());
    let source_handle = (source_handle != "output").then(|| source_handle.to_owned());
    let target_handle = target
        .port
        .as_str()
        .strip_prefix("in:")
        .unwrap_or(target.port.as_str());
    let target_handle = (target_handle != "input").then(|| target_handle.to_owned());
    if workflow.draft.edges.iter().any(|edge| {
        edge.source == source.node.as_str()
            && edge.target == target.node.as_str()
            && edge
                .source_handle
                .as_deref()
                .filter(|handle| *handle != "output")
                == source_handle.as_deref()
            && edge
                .target_handle
                .as_deref()
                .filter(|handle| *handle != "input")
                == target_handle.as_deref()
    }) {
        return Ok(None);
    }
    let mut sequence = workflow.draft.edges.len() + 1;
    let id = loop {
        let id = format!("edge:{}:{}:{sequence}", source.node, target.node);
        if workflow.draft.edges.iter().all(|edge| edge.id != id) {
            break id;
        }
        sequence += 1;
    };
    let edge = lilia_feature_automation::AutomationEdge {
        id,
        source: source.node.as_str().to_owned(),
        target: target.node.as_str().to_owned(),
        source_handle,
        target_handle,
    };
    let mut edges = workflow.draft.edges.clone();
    edges.push(edge.clone());
    lilia_feature_automation::validate_automation_graph(&workflow.draft.nodes, &edges)
        .map_err(|error| error.to_string())?;
    Ok(Some(edge))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lilia_feature_automation::{
        AutomationDraft, AutomationEdge, AutomationNode, AutomationNodePosition,
        AutomationScopeFilter, automation_active_outgoing_edges, automation_json_value_to_port,
    };
    use serde_json::json;

    #[test]
    fn initial_viewport_keeps_small_graph_readable_beside_editor() {
        let bounds = GraphRect::new(GraphPoint::new(320.0, 180.0), GraphSize::new(424.0, 112.0));
        let viewport = initial_viewport(bounds);
        assert_eq!(viewport.zoom, 1.0);
        assert_eq!(
            viewport.world_to_view(bounds.origin),
            GraphPoint::new(24.0, 24.0)
        );
        let end = viewport.world_to_view(GraphPoint::new(744.0, 292.0));
        assert!(end.x < 500.0 && end.y < 300.0);
    }

    #[test]
    fn ordinary_canvas_connections_execute_and_reject_duplicates_and_cycles() {
        let mut workflow = AutomationWorkflow {
            id: "workflow".into(),
            name: "Workflow".into(),
            enabled: false,
            scope: AutomationScopeFilter::default(),
            published_version_id: None,
            created_at: 1,
            updated_at: 1,
            draft: AutomationDraft {
                nodes: ["trigger", "first", "second"]
                    .into_iter()
                    .map(|id| AutomationNode {
                        id: id.into(),
                        title: id.into(),
                        kind: if id == "trigger" { "trigger" } else { "human" }.into(),
                        position: AutomationNodePosition { x: 0.0, y: 0.0 },
                        config: json!({}),
                    })
                    .collect(),
                ..Default::default()
            },
        };
        let source = GraphEndpoint::new("first", "out:output");
        let target = GraphEndpoint::new("second", "in:input");
        let edge = new_connection(&workflow, source.clone(), target.clone())
            .unwrap()
            .unwrap();
        assert!(edge.source_handle.is_none());
        workflow.draft.edges.push(edge);
        assert_eq!(
            automation_active_outgoing_edges(
                &workflow.draft.edges,
                "first",
                &json!({"confirmed":true})
            )
            .len(),
            1
        );
        assert!(new_connection(&workflow, source, target).unwrap().is_none());
        assert!(
            new_connection(
                &workflow,
                GraphEndpoint::new("second", "out:output"),
                GraphEndpoint::new("first", "in:input")
            )
            .is_err()
        );
        workflow.draft.edges[0].source_handle = Some("output".into());
        workflow.draft.edges[0].target_handle = Some("input".into());
        assert!(
            new_connection(
                &workflow,
                GraphEndpoint::new("first", "out:output"),
                GraphEndpoint::new("second", "in:input")
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(
            automation_active_outgoing_edges(
                &workflow.draft.edges,
                "first",
                &json!({"confirmed":true})
            )
            .len(),
            1
        );
    }

    #[test]
    fn declared_switch_ports_route_matching_values_and_default() {
        let workflow = AutomationWorkflow {
            id: "workflow".into(),
            name: "Workflow".into(),
            enabled: false,
            scope: AutomationScopeFilter::default(),
            published_version_id: None,
            created_at: 1,
            updated_at: 1,
            draft: AutomationDraft {
                nodes: vec![AutomationNode {
                    id: "switch".into(),
                    title: "Switch".into(),
                    kind: "logic".into(),
                    position: AutomationNodePosition { x: 0.0, y: 0.0 },
                    config: json!({"logic":"switch", "cases":["review ready"]}),
                }],
                ..Default::default()
            },
        };
        let model = automation_graph_model(&workflow).unwrap();
        let node = &model.nodes()[0];
        let edges: Vec<_> = node
            .ports
            .iter()
            .filter(|port| port.id.as_str() != "in:input" && port.id.as_str() != "out:output")
            .map(|port| AutomationEdge {
                id: port.id.as_str().to_owned(),
                source: "switch".into(),
                target: port.id.as_str().to_owned(),
                source_handle: port.id.as_str().strip_prefix("out:").map(str::to_owned),
                target_handle: None,
            })
            .collect();
        for (value, expected) in [
            ("review ready", "out:review_ready"),
            ("missing", "out:default"),
        ] {
            let output = json!({"routeKind":"switch", "selectedHandle":automation_json_value_to_port(&json!(value))});
            let active = automation_active_outgoing_edges(&edges, "switch", &output);
            assert_eq!(active.len(), 1);
            assert_eq!(active[0].id, expected);
        }
    }
}
