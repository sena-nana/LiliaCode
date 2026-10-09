//! The work panel's coding tab: workspace search, the git changes with their
//! diffs, and the project's terminals and tasks.

use std::sync::{Arc, Mutex};

use nana_ui::icons_tabler as tabler;
use nana_ui::runtime::view::{dynamic, each, signal, when, widget, EachExt, IntoView, Signal};
use nana_ui::runtime::{
    AlignSpec, AppContext, Button, DocumentId, FrameworkError, LengthSpec, MountedView,
    SemanticColorRole, StableNodeId, Stack, Text, TextChanged, TextInput,
};
use nana_ui::{ButtonKind, ControlSize, Icon};
use nana_ui_core::type_scale;

use super::theme::{caption, list_row_button, meta};
use super::timeline_row::row_action;
use crate::runtime_shell::{
    coding_entry_intent, emit, runtime_diff, IntentSink, ShellCodingChange, ShellCodingHit,
    ShellCodingSnapshot, ShellIntent,
};

/// One entry of the run section: a terminal session or a project task.
#[derive(Clone, Debug, PartialEq)]
struct RunRow {
    id: String,
    label: String,
    task: bool,
}

#[derive(Clone, Copy)]
struct CodingSignals {
    query: Signal<String>,
    mode: Signal<String>,
    scope: Signal<String>,
    busy: Signal<bool>,
    git: Signal<String>,
    diff_scope: Signal<String>,
    changes: Signal<Vec<ShellCodingChange>>,
    selected: Signal<Option<String>>,
    /// The open diff, and a counter that moves whenever it is replaced so
    /// the diff view is rebuilt for new content.
    diff: Signal<Option<crate::application::DocumentDiff>>,
    diff_generation: Signal<u64>,
    truncated: Signal<bool>,
    hits: Signal<Vec<ShellCodingHit>>,
    runs: Signal<Vec<RunRow>>,
}

impl CodingSignals {
    fn new() -> Self {
        Self {
            query: signal(String::new()),
            mode: signal(String::new()),
            scope: signal(String::new()),
            busy: signal(false),
            git: signal(String::new()),
            diff_scope: signal(String::new()),
            changes: signal(Vec::new()),
            selected: signal(None),
            diff: signal(None),
            diff_generation: signal(0),
            truncated: signal(false),
            hits: signal(Vec::new()),
            runs: signal(Vec::new()),
        }
    }

    fn apply(&self, coding: &ShellCodingSnapshot) {
        self.query.set_if_changed(coding.query.clone());
        self.mode.set_if_changed(coding.mode_label.clone());
        self.scope.set_if_changed(coding.scope_label.clone());
        self.busy.set_if_changed(coding.busy);
        self.git.set_if_changed(coding.git.clone());
        self.diff_scope
            .set_if_changed(coding.diff_scope_label.clone());
        self.changes.set_if_changed(coding.changes.clone());
        self.selected.set_if_changed(coding.selected_change.clone());
        if self.diff.set_if_changed(coding.selected_diff.clone()) {
            self.diff_generation.update(|generation| *generation += 1);
        }
        self.truncated.set_if_changed(coding.diff_truncated);
        self.hits.set_if_changed(coding.hits.clone());
        self.runs.set_if_changed(
            coding
                .terminals
                .iter()
                .map(|row| RunRow {
                    id: row.id.clone(),
                    label: row.label.clone(),
                    task: false,
                })
                .chain(coding.tasks.iter().map(|row| RunRow {
                    id: row.id.clone(),
                    label: row.label.clone(),
                    task: true,
                }))
                .collect(),
        );
    }
}

pub(crate) struct CodingPanelView {
    pub root: StableNodeId,
    signals: Arc<Mutex<Option<CodingSignals>>>,
    _mounted: MountedView,
}

impl CodingPanelView {
    pub(crate) fn mount(
        context: &mut AppContext,
        document: DocumentId,
        sink: IntentSink,
    ) -> Result<Self, FrameworkError> {
        let signals = Arc::new(Mutex::new(None));
        let install = Arc::clone(&signals);
        let mounted = context.mount_view_detached(document, move || {
            let state = CodingSignals::new();
            *install.lock().expect("coding signals") = Some(state);
            panel(state, sink)
        })?;
        let root = mounted
            .roots()
            .first()
            .copied()
            .ok_or(FrameworkError::InvalidInput)?;
        Ok(Self {
            root,
            signals,
            _mounted: mounted,
        })
    }

    pub(crate) fn sync(&self, coding: &ShellCodingSnapshot) {
        if let Some(state) = *self.signals.lock().expect("coding signals") {
            state.apply(coding);
        }
    }
}

fn panel(state: CodingSignals, sink: IntentSink) -> impl IntoView {
    widget(
        Stack::column(16.0)
            .padding_xy(12.0, 12.0)
            .min_width(LengthSpec::Px(0.0)),
    )
    .children((
        search_section(state, Arc::clone(&sink)),
        changes_section(state, Arc::clone(&sink)),
        runs_section(state, sink),
    ))
}

fn emit_on(sink: &IntentSink, intent: ShellIntent) -> impl Fn() + Send + Sync + 'static {
    let sink = Arc::clone(sink);
    move || emit(&sink, intent.clone())
}

fn section_header(title: &'static str, tools: impl IntoView) -> impl IntoView {
    widget(
        Stack::bar(4.0)
            .align(AlignSpec::Center)
            .min_height(LengthSpec::Px(24.0)),
    )
    .children((
        widget(Stack::row(0.0).grow(1.0)).children(widget(caption(title))),
        tools,
    ))
}

/// A small text toggle that names its current state.
fn chip(label: Signal<String>, on_activate: impl Fn() + Send + Sync + 'static) -> impl IntoView {
    widget(
        Button::new("")
            .kind(ButtonKind::Subtle)
            .size(ControlSize::Small),
    )
    .bind(move |button| button.label = label.get())
    .on_activate(on_activate)
}

fn search_section(state: CodingSignals, sink: IntentSink) -> impl IntoView {
    let query_sink = Arc::clone(&sink);
    let field = widget(
        TextInput::new("")
            .placeholder("搜索工作区")
            .size(ControlSize::Small),
    )
    .value(state.query)
    .on_input(move |event: &TextChanged| {
        emit(
            &query_sink,
            ShellIntent::CodingQueryChanged(event.value.to_string()),
        )
    });
    let tools = widget(Stack::row(2.0).align(AlignSpec::Center)).children((
        widget(row_action(tabler::FOLDER, "在文件管理器中打开"))
            .on_activate(emit_on(&sink, ShellIntent::OpenCodingWorkspace)),
        widget(row_action(tabler::TERMINAL_2, "打开工作区终端"))
            .on_activate(emit_on(&sink, ShellIntent::OpenCodingTerminal)),
        widget(row_action(tabler::REFRESH, "刷新"))
            .disabled(state.busy)
            .on_activate(emit_on(&sink, ShellIntent::RefreshCoding)),
    ));
    let hit_sink = Arc::clone(&sink);
    widget(Stack::column(8.0)).children((
        section_header("搜索", tools),
        widget(Stack::bar(6.0).align(AlignSpec::Center)).children((
            widget(
                Stack::row(0.0)
                    .grow(1.0)
                    .shrink(1.0)
                    .min_width(LengthSpec::Px(0.0)),
            )
            .children(field),
            widget(row_action(tabler::SEARCH, "搜索"))
                .disabled(state.busy)
                .on_activate(emit_on(&sink, ShellIntent::SearchCoding)),
        )),
        widget(Stack::row(6.0).wrap(true)).children((
            chip(state.mode, emit_on(&sink, ShellIntent::CycleCodingMode)),
            chip(state.scope, emit_on(&sink, ShellIntent::ToggleCodingScope)),
        )),
        state
            .hits
            .each(
                |hit| hit.id.clone(),
                move |hit| hit_row(hit, Arc::clone(&hit_sink)),
            )
            .gap(2.0),
    ))
}

fn hit_row(hit: ShellCodingHit, sink: IntentSink) -> impl IntoView {
    let summary = (!hit.summary.is_empty()).then(|| {
        widget(Stack::column(0.0).padding_xy(37.0, 0.0)).children(widget(meta(hit.summary.clone())))
    });
    widget(Stack::column(0.0)).children((
        widget(list_row_button(hit.label.clone(), tabler::FILE_CODE))
            .on_activate(emit_on(&sink, ShellIntent::OpenCodingHit(hit.id.clone()))),
        summary,
    ))
}

fn changes_section(state: CodingSignals, sink: IntentSink) -> impl IntoView {
    let title = widget(Stack::row(6.0).align(AlignSpec::Center)).children((
        widget(caption("改动")),
        widget(meta(String::new())).bind(move |text| {
            let git = state.git.get();
            if text.value != git {
                text.value = git;
            }
        }),
    ));
    let row_sink = Arc::clone(&sink);
    widget(Stack::column(6.0)).children((
        widget(
            Stack::bar(4.0)
                .align(AlignSpec::Center)
                .min_height(LengthSpec::Px(24.0)),
        )
        .children((
            widget(Stack::row(0.0).grow(1.0)).children(title),
            chip(
                state.diff_scope,
                emit_on(&sink, ShellIntent::CycleCodingDiffScope),
            ),
        )),
        when(
            move || state.changes.with(Vec::is_empty),
            || widget(meta("没有未提交的改动")),
        ),
        state
            .changes
            .each(
                |change| change.path.clone(),
                move |change| change_row(change, state, Arc::clone(&row_sink)),
            )
            .gap(2.0),
        when(
            move || state.truncated.get(),
            || widget(meta("改动较多，差异已截断")),
        ),
    ))
}

fn status_role(status: &str) -> SemanticColorRole {
    match status {
        "A" | "?" => SemanticColorRole::Success,
        "D" => SemanticColorRole::Danger,
        "U" => SemanticColorRole::Warning,
        _ => SemanticColorRole::Accent,
    }
}

fn change_row(change: ShellCodingChange, state: CodingSignals, sink: IntentSink) -> impl IntoView {
    let path = change.path.clone();
    let open_path = path.clone();
    let diff_path = path.clone();
    let is_open = move || {
        state
            .selected
            .with(|selected| selected.as_deref() == Some(&path))
    };
    let file_name = change
        .path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&change.path)
        .to_owned();
    let mut toggle = list_row_button(file_name, change_icon(change.status));
    toggle.accessible_name = change.path.clone();
    let counts = widget(Stack::row(6.0).align(AlignSpec::Center).shrink(0.0)).children((
        (change.additions > 0).then(|| {
            widget(
                Text::new(format!("+{}", change.additions))
                    .font_size(type_scale::META)
                    .color(SemanticColorRole::Success),
            )
        }),
        (change.deletions > 0).then(|| {
            widget(
                Text::new(format!("−{}", change.deletions))
                    .font_size(type_scale::META)
                    .color(SemanticColorRole::Danger),
            )
        }),
        widget(
            Text::new(change.status)
                .font_size(type_scale::META)
                .font_weight(600)
                .color(status_role(change.status)),
        ),
    ));
    let opened = is_open.clone();
    widget(Stack::column(4.0)).children((
        widget(
            Stack::bar(6.0)
                .align(AlignSpec::Center)
                .padding_xy(0.0, 0.0),
        )
        .children((
            widget(
                Stack::row(0.0)
                    .grow(1.0)
                    .shrink(1.0)
                    .min_width(LengthSpec::Px(0.0)),
            )
            .children(
                widget(toggle)
                    .bind(move |button| {
                        button.style.background = is_open().then_some(SemanticColorRole::Selected);
                    })
                    .on_activate(emit_on(&sink, ShellIntent::SelectCodingChange(open_path))),
            ),
            counts,
        )),
        when(opened, move || diff_view(state, diff_path.clone())),
    ))
}

fn change_icon(status: &str) -> Icon {
    match status {
        "A" | "?" => tabler::FILE_PLUS,
        "D" => tabler::FILE_MINUS,
        _ => tabler::FILE_DIFF,
    }
}

/// The open file's diff, rebuilt whenever the diff content is replaced.
fn diff_view(state: CodingSignals, _path: String) -> impl IntoView {
    dynamic(
        move || state.diff_generation.get(),
        move |_| {
            let diff = state.diff.get_untracked();
            diff.map(|diff| {
                let lines = diff
                    .hunks
                    .iter()
                    .map(|hunk| hunk.lines.len() + 1)
                    .sum::<usize>();
                let height = (lines as f32 * 22.0 + 56.0).min(420.0);
                let mut view = runtime_diff(&diff).review_actions(false);
                view.layout = nana_ui::runtime::DiffLayout::Unified;
                std::sync::Arc::make_mut(&mut view.style.layout).height =
                    Some(LengthSpec::Px(height));
                widget(view)
            })
        },
    )
}

fn runs_section(state: CodingSignals, sink: IntentSink) -> impl IntoView {
    let run_sink = Arc::clone(&sink);
    when(
        move || !state.runs.with(Vec::is_empty),
        move || {
            let sink = Arc::clone(&run_sink);
            widget(Stack::column(6.0)).children((
                section_header("运行", ()),
                each(
                    state.runs,
                    |row| (row.task, row.id.clone()),
                    move |row| {
                        widget(list_row_button(
                            row.label.clone(),
                            if row.task {
                                tabler::PLAYER_PLAY
                            } else {
                                tabler::TERMINAL_2
                            },
                        ))
                        .on_activate(emit_on(&sink, coding_entry_intent(row.task, &row.id)))
                    },
                )
                .gap(2.0),
            ))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_labelled(
        context: &AppContext,
        root: StableNodeId,
        label: &str,
    ) -> Option<StableNodeId> {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let node = context.world().node(id)?;
            stack.extend(node.children.iter().copied());
            if context
                .world()
                .accessibility(id)
                .is_some_and(|state| state.label.as_deref() == Some(label))
            {
                return Some(id);
            }
        }
        None
    }

    #[test]
    fn changed_files_open_their_diff_without_review_actions() {
        let mut context = AppContext::new();
        let document = DocumentId::new(1).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&events);
        let view = CodingPanelView::mount(
            &mut context,
            document,
            Arc::new(move |intent| log.lock().unwrap().push(intent)),
        )
        .unwrap();
        let coding = crate::offscreen_tests::coding_fixture().coding.unwrap();
        view.sync(&coding);
        context.flush_reactive().unwrap();
        let row = find_labelled(&context, view.root, "apps/desktop/src/ui/coding_panel.rs")
            .expect("change row");
        assert!(context.activate_node(row).unwrap());
        assert!(events.lock().unwrap().iter().any(|intent| matches!(
            intent,
            ShellIntent::SelectCodingChange(path) if path == "apps/desktop/src/ui/coding_panel.rs"
        )));
        let mut stack = vec![view.root];
        let mut header = false;
        while let Some(id) = stack.pop() {
            let Some(node) = context.world().node(id) else {
                continue;
            };
            stack.extend(node.children.iter().copied());
            header |= context.world().text(id) == Some("@@ -10,4 +10,6 @@");
        }
        assert!(header, "the selected file's hunk is shown");
        for label in ["接受块0", "拒绝块0"] {
            assert!(find_labelled(&context, view.root, label).is_none());
        }
    }
}
