use std::borrow::Cow;

use nana_ui::runtime::{
    AppContext, DocumentId, FrameworkError, HeadlessInput, InputRouteError, InputRouteOutcome,
};
use nana_ui_platform::{
    InputModifiers, InputPayload, KeyInput, KeyState, LogicalKey, PhysicalKey, PointerId,
    PointerInput, PointerPhase, WheelInput, WheelUnit,
};

pub(crate) struct ScriptedInput {
    inner: HeadlessInput,
}

impl ScriptedInput {
    pub(crate) fn bind(context: &mut AppContext, document: DocumentId) -> Self {
        // Rebinding without a cancel left the previous pointer hovering, and
        // chart tooltips stay open for any pointer still on the chart.
        let now = context.world().animation_now();
        let _ = context.unbind_input_source(HeadlessInput::SOURCE, now);
        Self {
            inner: HeadlessInput::bind(context, document),
        }
    }

    pub(crate) fn services_mut(&mut self) -> &mut nana_ui_platform::HeadlessHostServices {
        self.inner.services_mut()
    }

    pub(crate) fn press(
        &mut self,
        context: &mut AppContext,
        name: &str,
        modifiers: InputModifiers,
        repeat: bool,
    ) -> Result<InputRouteOutcome, InputRouteError> {
        self.press_text(context, name, modifiers, repeat, None)
    }

    pub(crate) fn press_key(
        &mut self,
        context: &mut AppContext,
        key: KeyInput,
    ) -> Result<InputRouteOutcome, InputRouteError> {
        self.inner.press(context, key, None, None)
    }

    pub(crate) fn press_text(
        &mut self,
        context: &mut AppContext,
        name: &str,
        modifiers: InputModifiers,
        repeat: bool,
        text: Option<&str>,
    ) -> Result<InputRouteOutcome, InputRouteError> {
        self.inner
            .press(context, platform_key(name, modifiers, repeat), text, None)
    }

    pub(crate) fn pointer(
        &mut self,
        context: &mut AppContext,
        phase: PointerPhase,
        x: f32,
        y: f32,
        buttons: u16,
    ) -> Result<InputRouteOutcome, InputRouteError> {
        self.inner
            .route(context, pointer_payload(phase, x, y, buttons))
    }

    pub(crate) fn wheel(
        &mut self,
        context: &mut AppContext,
        x: f32,
        y: f32,
        delta_y: f32,
    ) -> Result<InputRouteOutcome, InputRouteError> {
        self.inner.route(
            context,
            InputPayload::Wheel(WheelInput {
                pointer_id: PointerId(1),
                x,
                y,
                delta_x: 0.0,
                delta_y,
                unit: WheelUnit::Pixels,
                modifiers: InputModifiers::default(),
            }),
        )
    }
}

pub(crate) fn platform_key(name: &str, modifiers: InputModifiers, repeat: bool) -> KeyInput {
    let name: Cow<'static, str> = Cow::Owned(name.to_owned());
    KeyInput {
        physical: PhysicalKey(name.clone()),
        logical: LogicalKey(name),
        state: KeyState::Pressed,
        repeat,
        modifiers,
    }
}

pub(crate) fn pointer_payload(phase: PointerPhase, x: f32, y: f32, buttons: u16) -> InputPayload {
    let mut pointer = PointerInput::mouse(phase, x, y);
    pointer.buttons = buttons;
    pointer.pressure = if buttons == 0 { 0.0 } else { 1.0 };
    InputPayload::Pointer(pointer)
}

pub(crate) fn route_error(error: InputRouteError) -> FrameworkError {
    match error {
        InputRouteError::Dispatch(error) => error,
        InputRouteError::UnknownSource
        | InputRouteError::StaleGeneration
        | InputRouteError::Disconnected
        | InputRouteError::OutOfOrder
        | InputRouteError::TimestampRegression => FrameworkError::InvalidInput,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nana_ui::runtime::{
        Button, Entity, LayoutViewport, LengthSpec, NodeStyle, SemanticColorRole, Stack,
        TimeSeriesChart, TimeSeriesLayer, Tooltip,
    };
    use std::sync::Arc;

    fn chart_tooltip_visible(context: &AppContext, chart: nana_ui::runtime::StableNodeId) -> bool {
        context
            .world()
            .node(chart)
            .into_iter()
            .flat_map(|node| node.children)
            .any(|child| {
                context
                    .read(Entity::<Tooltip>::from_stable_id(child), |tip| {
                        !tip.style.layout.hidden
                    })
                    .unwrap_or(false)
            })
    }

    #[test]
    fn rebinding_scripted_pointer_clears_chart_tooltip_after_leaving() {
        let mut context = AppContext::new();
        let document = DocumentId::new(1).expect("document");
        let root = context
            .create_component(document, Stack::column(8.0))
            .expect("root");
        let mut style = NodeStyle::default();
        Arc::make_mut(&mut style.layout).height = Some(LengthSpec::Px(180.0));
        let chart = context
            .create_detached_component(
                document,
                TimeSeriesChart::new([4.0, 8.0])
                    .label("总量")
                    .axis_labels(["09-01", "09-02"])
                    .stacked([TimeSeriesLayer::new(
                        "输入",
                        [1.0, 2.0],
                        SemanticColorRole::Accent,
                    )])
                    .style(style),
            )
            .expect("chart");
        let tab = context
            .create_detached_component(document, Button::new("用量与额度"))
            .expect("tab");
        context.append_child(root, chart).expect("chart child");
        context.append_child(root, tab).expect("tab child");
        context
            .layout_document(document, LayoutViewport::new(480.0, 420.0))
            .expect("layout");
        context.rebuild_hit_test(document);

        let bounds = context
            .world()
            .layout_box(chart.stable_id())
            .expect("chart bounds");
        let (chart_x, chart_y) = context
            .world()
            .layout_pointer_position(
                chart.stable_id(),
                bounds.x + bounds.width * 0.75,
                bounds.y + bounds.height * 0.5,
            )
            .expect("chart pointer");
        let mut input = ScriptedInput::bind(&mut context, document);
        input
            .pointer(&mut context, PointerPhase::Move, chart_x, chart_y, 0)
            .expect("hover chart");
        assert!(context.read(chart, |chart| chart.active).unwrap().is_some());
        assert!(chart_tooltip_visible(&context, chart.stable_id()));

        let bounds = context
            .world()
            .layout_box(chart.stable_id())
            .expect("chart bounds");
        let (other_x, other_y) = context
            .world()
            .layout_pointer_position(
                chart.stable_id(),
                bounds.x + bounds.width * 0.25,
                bounds.y + bounds.height * 0.5,
            )
            .expect("second chart pointer");
        let mut input = ScriptedInput::bind(&mut context, document);
        input
            .pointer(&mut context, PointerPhase::Move, other_x, other_y, 0)
            .expect("hover another datum");
        assert!(context.read(chart, |chart| chart.active).unwrap().is_some());
        assert!(chart_tooltip_visible(&context, chart.stable_id()));

        let bounds = context.world().layout_box(tab.stable_id()).expect("tab bounds");
        let (tab_x, tab_y) = context
            .world()
            .layout_pointer_position(
                tab.stable_id(),
                bounds.x + bounds.width * 0.5,
                bounds.y + bounds.height * 0.5,
            )
            .expect("tab pointer");
        let mut input = ScriptedInput::bind(&mut context, document);
        input
            .pointer(&mut context, PointerPhase::Move, tab_x, tab_y, 0)
            .expect("hover tab");
        assert_eq!(context.read(chart, |chart| chart.active).unwrap(), None);
        assert!(!chart_tooltip_visible(&context, chart.stable_id()));
    }
}
