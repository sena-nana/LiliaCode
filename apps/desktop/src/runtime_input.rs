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
