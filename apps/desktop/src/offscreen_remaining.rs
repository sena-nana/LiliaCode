//! Offscreen GPU snapshots of the remaining workbench surfaces.
//!
//! Not a product present path: `nana-ui-devtools` CPU-readback is test-only.

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::Arc;

    use nana_ui::runtime::{
        DocumentId, LayoutViewport, RuntimeDocument, SemanticColorRole, Stack, ThemeMode,
    };
    use nana_ui::{
        GraphModel, GraphNode, GraphPoint, GraphSelection, GraphSize, GraphViewport, NanaTextShaper,
    };
    use nana_ui_devtools::offscreen::{self, Size};
    use serde_json::json;

    use crate::module::automation::editor::{NodeEditorField, NodeEditorSnapshot};
    use crate::module::automation::view::{
        AutomationRow, AutomationTarget, AutomationView, AutomationViewSnapshot,
    };
    use crate::module::memory::view::{MemoryCard, MemoryView, MemoryViewSnapshot};
    use crate::module::roadmap::view::{
        RoadmapCard, RoadmapTask, RoadmapView, RoadmapViewSnapshot,
    };
    use crate::runtime_compat::HostedWindowId;

    const WIDTH: u32 = 1180;
    const HEIGHT: u32 = 760;

    fn out_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/offscreen-remaining")
    }

    fn unique_colors(pixels: &[u8]) -> usize {
        pixels
            .chunks_exact(4)
            .map(|pixel| u32::from_be_bytes([0, pixel[0], pixel[1], pixel[2]]))
            .collect::<HashSet<_>>()
            .len()
    }

    fn capture(
        document: &mut RuntimeDocument,
        name: &str,
        theme: ThemeMode,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let mut shaper = NanaTextShaper::default();
        document
            .context_mut()
            .set_theme(theme)
            .expect("theme");
        document.flush(
            LayoutViewport::new(WIDTH as f32, HEIGHT as f32),
            &mut shaper,
        )?;
        let Some(mut gpu) = offscreen::optional() else {
            return Err("no offscreen GPU".into());
        };
        let renderers = gpu.default_gpu_renderers();
        let color = document
            .context()
            .world()
            .style_model()
            .color(SemanticColorRole::Background);
        let clear = [color.r, color.g, color.b, color.a];
        let size = Size::new(WIDTH, HEIGHT);
        let pixels = gpu.paint(
            document.scene(),
            size,
            clear,
            None,
            Some(&renderers),
        )?;
        let colors = unique_colors(&pixels);
        assert!(
            colors > 8,
            "{name} painted {colors} unique colours (flat clear)"
        );
        let path = out_dir().join(format!("{name}.png"));
        offscreen::write_png(&path, size, &pixels)?;
        Ok(path)
    }

    fn sample_graph() -> GraphModel {
        GraphModel::new(
            vec![
                GraphNode::new(
                    "trigger",
                    "手动触发",
                    GraphPoint::new(80.0, 80.0),
                    GraphSize::new(184.0, 112.0),
                ),
                GraphNode::new(
                    "human-2",
                    "人工确认",
                    GraphPoint::new(320.0, 80.0),
                    GraphSize::new(184.0, 112.0),
                ),
            ],
            Vec::new(),
        )
        .expect("sample graph")
    }

    fn automation_snapshot(inspector: &str) -> AutomationViewSnapshot {
        AutomationViewSnapshot {
            rows: vec![AutomationRow {
                id: "wf-1".into(),
                label: "自动事件与重启验收".into(),
                selected: true,
            }],
            target: Some(AutomationTarget {
                window_id: HostedWindowId::PRIMARY,
                workflow_id: "wf-1".into(),
                modified_at: 1,
            }),
            name: "自动事件与重启验收".into(),
            enabled: true,
            published: true,
            graph: sample_graph(),
            viewport: GraphViewport::new(GraphPoint::new(24.0, 24.0), 1.0),
            selection: Some(GraphSelection::Node("human-2".into())),
            editor: Some(NodeEditorSnapshot {
                node_id: "human-2".into(),
                title: "人工确认".into(),
                fields: vec![NodeEditorField {
                    key: "prompt".into(),
                    value: json!("重启后继续同一运行"),
                }],
            }),
            inspector_panel: inspector.into(),
            include_inbox: false,
            event_kinds: vec!["task_created".into()],
            projects: vec![(
                "native-agent-debug-project".into(),
                "验收项目".into(),
                true,
            )],
            ..Default::default()
        }
    }

    #[test]
    fn remaining_surfaces_paint_offscreen() {
        if !offscreen::pixels_available() {
            return;
        }
        let document_id = DocumentId::new(901).unwrap();
        let mut document = RuntimeDocument::new(document_id);
        let host = document
            .context_mut()
            .create_component(document_id, Stack::fill_row(0.0))
            .unwrap();
        let snapshot = automation_snapshot("node");
        let mut view = AutomationView::mount(
            document.context_mut(),
            document_id,
            &snapshot,
            true,
            Arc::new(|_| {}),
        )
        .unwrap();
        document
            .context_mut()
            .append_child(host, view.sidebar)
            .unwrap();
        document
            .context_mut()
            .append_child(host, view.page)
            .unwrap();
        view.sync(document.context_mut(), document_id, &snapshot, true)
            .unwrap();
        capture(&mut document, "automation-node-light", ThemeMode::Light).unwrap();
        capture(&mut document, "automation-node-dark", ThemeMode::Dark).unwrap();

        let scoped = automation_snapshot("scope");
        view.sync(document.context_mut(), document_id, &scoped, true)
            .unwrap();
        capture(&mut document, "automation-scope-light", ThemeMode::Light).unwrap();

        let document_id = DocumentId::new(902).unwrap();
        let mut document = RuntimeDocument::new(document_id);
        let host = document
            .context_mut()
            .create_component(document_id, Stack::fill_column(16.0).padding(16.0))
            .unwrap();
        let mut memory = MemoryView::mount(document.context_mut(), document_id, Arc::new(|_| {}))
            .unwrap();
        document
            .context_mut()
            .append_child(host, memory.root)
            .unwrap();
        memory
            .sync(
                document.context_mut(),
                document_id,
                &MemoryViewSnapshot {
                    project_id: Some("native-agent-debug-project".into()),
                    selected: Some("mem-1".into()),
                    title: "正常入口多行记忆".into(),
                    body: "项目约定：使用正常用户入口。\n验证要求：保存、禁用、重新启用。".into(),
                    tags: "验收,持久化".into(),
                    scope_label: "项目".into(),
                    enabled: true,
                    global_enabled: true,
                    baseline_enabled: true,
                    task_injection: Some(true),
                    cooldown: "3".into(),
                    cards: vec![MemoryCard {
                        id: "mem-1".into(),
                        title: "正常入口多行记忆".into(),
                        subtitle: "项目 · 启用".into(),
                    }],
                    tasks: vec![("native-agent-debug-task".into(), "验收任务".into())],
                    ..Default::default()
                },
            )
            .unwrap();
        capture(&mut document, "memory-light", ThemeMode::Light).unwrap();
        capture(&mut document, "memory-dark", ThemeMode::Dark).unwrap();

        let document_id = DocumentId::new(903).unwrap();
        let mut document = RuntimeDocument::new(document_id);
        let host = document
            .context_mut()
            .create_component(document_id, Stack::fill_column(16.0).padding(16.0))
            .unwrap();
        let mut roadmap =
            RoadmapView::mount(document.context_mut(), document_id, Arc::new(|_| {})).unwrap();
        document
            .context_mut()
            .append_child(host, roadmap.root)
            .unwrap();
        roadmap
            .sync(
                document.context_mut(),
                document_id,
                &RoadmapViewSnapshot {
                    project_id: Some("native-agent-debug-project".into()),
                    selected: Some("ms-1".into()),
                    title: "正常入口路线图验收".into(),
                    description: "第一阶段：建立关联\n第二阶段：验证重启保留".into(),
                    due_date: String::new(),
                    status_label: "进行中".into(),
                    cards: vec![RoadmapCard {
                        id: "ms-1".into(),
                        title: "正常入口路线图验收".into(),
                        subtitle: "进行中 · 无截止日期".into(),
                    }],
                    tasks: vec![RoadmapTask {
                        id: "native-agent-debug-task".into(),
                        title: "验收任务".into(),
                        linked: true,
                    }],
                    error: None,
                },
            )
            .unwrap();
        capture(&mut document, "roadmap-light", ThemeMode::Light).unwrap();
        capture(&mut document, "roadmap-dark", ThemeMode::Dark).unwrap();
    }
}
