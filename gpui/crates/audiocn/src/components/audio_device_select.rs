//! Port of `components/ui/audio-device-select.tsx`: a microphone, speaker or
//! source picker built on [`Select`], with permission-needed, loading, empty
//! and disconnected states. It takes the devices as data ([`AudioDevice`],
//! the fields of `useAudioDevices`' `AudioDeviceInfo` plus the component's
//! `status` / `description`); it never enumerates devices itself.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, Refineable, RenderOnce, SharedString,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::theme::ActiveTheme;
use crate::ui::styles::{TextSize, provide, radius_lg, use_context};
use crate::ui::{
    Button, ButtonSize, ButtonVariant, Select, SelectContent, SelectGroup, SelectItem, SelectNode,
    SelectSize, SelectTrigger, SelectValue,
};

/// The value of the "None" item.
const NONE_VALUE: &str = "__none__";
/// The value of the disabled "Finding devices…" item.
const LOADING_VALUE: &str = "__loading__";

/// `AudioDeviceKind` of `useAudioDevices`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AudioDeviceKind {
    /// `audioinput`: microphones and other sources.
    #[default]
    AudioInput,
    /// `audiooutput`: speakers and headphones.
    AudioOutput,
}

/// `AudioDeviceStatus`: whether a device can be picked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AudioDeviceStatus {
    #[default]
    Available,
    /// Listed but disabled (e.g. in use by another app).
    Unavailable,
    /// Listed but disabled until permission is granted.
    PermissionRequired,
}

/// Microphone permission (`AudioPermission` of `useAudioDevices`).
/// [`AudioDeviceSelect`] treats `Unsupported` like `Denied`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AudioPermission {
    #[default]
    Granted,
    Prompt,
    Denied,
    Unsupported,
}

/// An audio device to list (`AudioDevice` / `AudioDeviceInfo`).
///
/// ```ignore
/// AudioDevice::new("usb", "Shure MV7+").with_description("USB")
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct AudioDevice {
    pub id: SharedString,
    pub label: SharedString,
    pub kind: AudioDeviceKind,
    pub group_id: SharedString,
    /// The system default device (shows a "Default" hint unless the label
    /// already starts with "Default").
    pub is_default: bool,
    pub status: AudioDeviceStatus,
    /// A second line under the label, e.g. "USB".
    pub description: Option<SharedString>,
}

impl AudioDevice {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            ..Default::default()
        }
    }

    pub fn with_kind(mut self, kind: AudioDeviceKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_group_id(mut self, group_id: impl Into<SharedString>) -> Self {
        self.group_id = group_id.into();
        self
    }

    pub fn with_default(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }

    pub fn with_status(mut self, status: AudioDeviceStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// One row of the device list (`DeviceItem`).
#[derive(Clone, Debug, PartialEq)]
struct DeviceItem {
    value: SharedString,
    label: SharedString,
    device: Option<AudioDevice>,
    missing: bool,
    none: bool,
}

/// The items and the `missing` flag of `AudioDeviceSelect`.
fn device_items(
    devices: &[AudioDevice],
    value: Option<&SharedString>,
    allow_none: bool,
    none_label: &SharedString,
    loading: bool,
    known_labels: &HashMap<SharedString, SharedString>,
) -> (Vec<DeviceItem>, bool) {
    let missing = value.is_some_and(|v| !loading && !devices.iter().any(|d| &d.id == v));
    let mut list = Vec::new();
    if allow_none {
        list.push(DeviceItem {
            value: NONE_VALUE.into(),
            label: none_label.clone(),
            device: None,
            missing: false,
            none: true,
        });
    }
    for device in devices {
        list.push(DeviceItem {
            value: device.id.clone(),
            label: device.label.clone(),
            device: Some(device.clone()),
            missing: false,
            none: false,
        });
    }
    if let (true, Some(value)) = (missing, value) {
        let remembered = known_labels
            .get(value)
            .cloned()
            .unwrap_or_else(|| "Unknown device".into());
        list.push(DeviceItem {
            value: value.clone(),
            label: format!("{remembered} (disconnected)").into(),
            device: None,
            missing: true,
            none: false,
        });
    }
    (list, missing)
}

type IdHandler = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App) + 'static>;
type ActionHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// What `AudioDeviceSelect` hands its parts (React context).
#[derive(Clone)]
struct DeviceSelectContext {
    items: Rc<Vec<DeviceItem>>,
    loading: bool,
    permission: AudioPermission,
    /// `data-missing` on the trigger in React (no visual effect).
    #[allow(dead_code)]
    missing: bool,
    on_request_permission: Option<ActionHandler>,
}

fn effective_permission(p: AudioPermission) -> AudioPermission {
    match p {
        AudioPermission::Unsupported => AudioPermission::Denied,
        p => p,
    }
}

struct DeviceSelectState {
    uncontrolled: Option<SharedString>,
    known_labels: HashMap<SharedString, SharedString>,
}

/// A microphone, speaker or source picker (`<AudioDeviceSelect>`). Lists
/// `devices` with "Default" hints and descriptions, keeps a selected device
/// that went away visible as "… (disconnected)", and shows permission,
/// loading ("Finding devices…") and empty ("No devices found.") states.
/// Controlled with [`AudioDeviceSelect::value`] (`None` is no selection, or
/// "None" with `allow_none`), uncontrolled with `default_value`.
///
/// ```ignore
/// AudioDeviceSelect::new("mic", devices)
///     .value(selected.clone())
///     .permission(AudioPermission::Prompt)
///     .on_request_permission(|_, cx| request_access(cx))
///     .on_value_change(|id, _, cx| select_device(id, cx))
/// ```
#[derive(IntoElement)]
pub struct AudioDeviceSelect {
    id: ElementId,
    devices: Vec<AudioDevice>,
    value: Option<Option<SharedString>>,
    default_value: Option<SharedString>,
    on_value_change: Option<IdHandler>,
    allow_none: bool,
    none_label: SharedString,
    loading: bool,
    permission: AudioPermission,
    on_request_permission: Option<ActionHandler>,
    disabled: bool,
    trigger: Option<AudioDeviceSelectTrigger>,
    content: Option<AudioDeviceSelectContent>,
}

impl AudioDeviceSelect {
    pub fn new(id: impl Into<ElementId>, devices: impl IntoIterator<Item = AudioDevice>) -> Self {
        Self {
            id: id.into(),
            devices: devices.into_iter().collect(),
            value: None,
            default_value: None,
            on_value_change: None,
            allow_none: false,
            none_label: "None".into(),
            loading: false,
            permission: AudioPermission::Granted,
            on_request_permission: None,
            disabled: false,
            trigger: None,
            content: None,
        }
    }

    /// Controlled selected device id; `None` is no selection (or "None").
    pub fn value(mut self, id: Option<SharedString>) -> Self {
        self.value = Some(id);
        self
    }

    /// Initial device id when uncontrolled. Default none.
    pub fn default_value(mut self, id: impl Into<SharedString>) -> Self {
        self.default_value = Some(id.into());
        self
    }

    /// Called with the picked device id (`None` for the "None" item).
    pub fn on_value_change(
        mut self,
        f: impl Fn(Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_value_change = Some(Rc::new(f));
        self
    }

    /// Adds a "None" item. Default false.
    pub fn allow_none(mut self, allow: bool) -> Self {
        self.allow_none = allow;
        self
    }

    /// Default "None".
    pub fn none_label(mut self, label: impl Into<SharedString>) -> Self {
        self.none_label = label.into();
        self
    }

    /// Shows "Finding devices…". Default false.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Microphone permission. Default [`AudioPermission::Granted`].
    pub fn permission(mut self, permission: AudioPermission) -> Self {
        self.permission = permission;
        self
    }

    /// Shows an "Allow access" button while permission is `Prompt`.
    pub fn on_request_permission(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_request_permission = Some(Rc::new(f));
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The trigger (default: an [`AudioDeviceSelectTrigger`] with an
    /// [`AudioDeviceSelectValue`]).
    pub fn trigger(mut self, trigger: AudioDeviceSelectTrigger) -> Self {
        self.trigger = Some(trigger);
        self
    }

    /// The popup (default: [`AudioDeviceSelectContent::new`]).
    pub fn content(mut self, content: AudioDeviceSelectContent) -> Self {
        self.content = Some(content);
        self
    }
}

impl RenderOnce for AudioDeviceSelect {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let default_value = self.default_value.clone();
        let state = window.use_keyed_state(
            ElementId::Name(format!("{}-device", self.id).into()),
            cx,
            |_, _| DeviceSelectState {
                uncontrolled: default_value,
                known_labels: HashMap::new(),
            },
        );
        let devices = self.devices;
        state.update(cx, |s, _| {
            for device in &devices {
                s.known_labels
                    .insert(device.id.clone(), device.label.clone());
            }
        });
        let controlled = self.value.is_some();
        let value = match self.value {
            Some(v) => v,
            None => state.read(cx).uncontrolled.clone(),
        };
        let (items, missing) = device_items(
            &devices,
            value.as_ref(),
            self.allow_none,
            &self.none_label,
            self.loading,
            &state.read(cx).known_labels,
        );
        let selected = match &value {
            None if self.allow_none => Some(SharedString::from(NONE_VALUE)),
            v => v.clone(),
        };
        let context = DeviceSelectContext {
            items: Rc::new(items),
            loading: self.loading,
            permission: effective_permission(self.permission),
            missing,
            on_request_permission: self.on_request_permission,
        };

        let trigger = self
            .trigger
            .unwrap_or_else(|| AudioDeviceSelectTrigger::new().child(AudioDeviceSelectValue::new()))
            .into_select_trigger();
        let content = self
            .content
            .unwrap_or_default()
            .into_select_content(&context, cx);

        let handler = self.on_value_change;
        let select = Select::new(self.id)
            .selected(selected)
            .disabled(self.disabled)
            .trigger(trigger)
            .content(content)
            .on_value_change(move |next, window, cx| {
                let id = (next.as_ref() != NONE_VALUE).then_some(next);
                if !controlled {
                    state.update(cx, |s, cx| {
                        s.uncontrolled = id.clone();
                        cx.notify();
                    });
                }
                if let Some(h) = handler.as_ref() {
                    h(id, window, cx);
                }
            });
        provide(context, div().w_full().min_w_0().child(select))
    }
}

/// The trigger (`<AudioDeviceSelectTrigger>`): a full-width [`SelectTrigger`].
/// Put an [`AudioDeviceSelectValue`] (and any icon) in it.
///
/// ```ignore
/// AudioDeviceSelectTrigger::new().child(AudioDeviceSelectValue::new().placeholder("Select a microphone"))
/// ```
#[derive(Default)]
pub struct AudioDeviceSelectTrigger {
    size: SelectSize,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl AudioDeviceSelectTrigger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn size(mut self, size: SelectSize) -> Self {
        self.size = size;
        self
    }

    fn into_select_trigger(self) -> SelectTrigger {
        // `w-full min-w-0`.
        let mut trigger = SelectTrigger::new().size(self.size).w_full().min_w_0();
        trigger.style().refine(&self.style);
        trigger.children(self.children)
    }
}

impl ParentElement for AudioDeviceSelectTrigger {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for AudioDeviceSelectTrigger {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The selected device's label in the trigger (`<AudioDeviceSelectValue>`),
/// or the placeholder ("Finding devices…" while loading). Must be inside an
/// [`AudioDeviceSelect`]'s trigger.
#[derive(IntoElement)]
pub struct AudioDeviceSelectValue {
    placeholder: SharedString,
    style: StyleRefinement,
}

impl Default for AudioDeviceSelectValue {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDeviceSelectValue {
    pub fn new() -> Self {
        Self {
            placeholder: "Select a device".into(),
            style: StyleRefinement::default(),
        }
    }

    /// Shown with no selection. Default "Select a device".
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }
}

impl Styled for AudioDeviceSelectValue {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for AudioDeviceSelectValue {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = use_context::<DeviceSelectContext>()
            .expect("AudioDeviceSelectValue must be used inside AudioDeviceSelect.");
        let muted = cx.theme().muted_foreground;
        let placeholder = self.placeholder;
        let mut value = SelectValue::new().render(move |selected, _| match selected {
            None => div()
                .text_color(muted)
                .child(if ctx.loading {
                    "Finding devices…".into()
                } else {
                    placeholder.clone()
                })
                .into_any_element(),
            Some(selected) => ctx
                .items
                .iter()
                .find(|item| &item.value == selected)
                .map(|item| item.label.clone())
                .unwrap_or_else(|| placeholder.clone())
                .into_any_element(),
        });
        // `truncate`.
        value = value.overflow_hidden().text_ellipsis().whitespace_nowrap();
        value.style().refine(&self.style);
        value
    }
}

/// A device row (`<AudioDeviceSelectItem>`): the label with a "Default"
/// hint and an optional description line. Children replace that layout.
///
/// ```ignore
/// AudioDeviceSelectContent::new().child(AudioDeviceSelectItem::new("usb").device(usb_mic))
/// ```
pub struct AudioDeviceSelectItem {
    value: SharedString,
    device: Option<AudioDevice>,
    disabled: bool,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl AudioDeviceSelectItem {
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            device: None,
            disabled: false,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    pub fn device(mut self, device: AudioDevice) -> Self {
        self.device = Some(device);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    fn into_select_item(self, cx: &App) -> SelectItem {
        let label = self
            .device
            .as_ref()
            .map(|d| d.label.clone())
            .unwrap_or_else(|| self.value.clone());
        // `items-start`.
        let mut item = SelectItem::new(self.value, label)
            .disabled(self.disabled)
            .items_start();
        item.style().refine(&self.style);
        if !self.children.is_empty() {
            return item.children(self.children);
        }
        let muted = cx.theme().muted_foreground;
        let device = self.device.unwrap_or_default();
        let show_default = device.is_default && !device.label.starts_with("Default");
        item.child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(device.label.clone()),
                        )
                        .when(show_default, |el| {
                            el.child(div().text_xs_().text_color(muted).child("Default"))
                        }),
                )
                .when_some(device.description, |el, d| {
                    el.child(div().text_xs_().text_color(muted).child(d))
                }),
        )
    }
}

impl ParentElement for AudioDeviceSelectItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for AudioDeviceSelectItem {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// The permission notice (`<AudioDeviceSelectPermission>`): a message and,
/// while permission is `Prompt` and a handler exists, an "Allow access"
/// button. Popup content is laid out outside the select's context, so the
/// default content passes `permission` / `on_request_permission`
/// explicitly; set them yourself in custom content.
#[derive(IntoElement)]
pub struct AudioDeviceSelectPermission {
    message: Option<SharedString>,
    action_label: SharedString,
    permission: Option<AudioPermission>,
    on_request_permission: Option<ActionHandler>,
    style: StyleRefinement,
}

impl Default for AudioDeviceSelectPermission {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDeviceSelectPermission {
    pub fn new() -> Self {
        Self {
            message: None,
            action_label: "Allow access".into(),
            permission: None,
            on_request_permission: None,
            style: StyleRefinement::default(),
        }
    }

    /// Default: "Allow microphone access to see your devices." (or the
    /// blocked message when denied).
    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Default "Allow access".
    pub fn action_label(mut self, label: impl Into<SharedString>) -> Self {
        self.action_label = label.into();
        self
    }

    pub fn permission(mut self, permission: AudioPermission) -> Self {
        self.permission = Some(permission);
        self
    }

    pub fn on_request_permission(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_request_permission = Some(Rc::new(f));
        self
    }
}

impl Styled for AudioDeviceSelectPermission {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for AudioDeviceSelectPermission {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let ctx = use_context::<DeviceSelectContext>();
        let permission = effective_permission(
            self.permission
                .or(ctx.as_ref().map(|c| c.permission))
                .unwrap_or_default(),
        );
        let on_request = self
            .on_request_permission
            .or_else(|| ctx.and_then(|c| c.on_request_permission));
        let denied = permission == AudioPermission::Denied;
        let text = self.message.unwrap_or_else(|| {
            if denied {
                "Microphone access is blocked. Allow it in your browser's site settings.".into()
            } else {
                "Allow microphone access to see your devices.".into()
            }
        });
        let mut el = div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(8.0))
            .p(px(8.0))
            .text_xs_()
            .text_color(cx.theme().muted_foreground)
            .child(text)
            .when_some(on_request.filter(|_| !denied), |el, on_request| {
                el.child(
                    Button::new("audio-device-select-permission")
                        .size(ButtonSize::Xs)
                        .variant(ButtonVariant::Outline)
                        .on_click(move |_, window, cx| on_request(window, cx))
                        .child(self.action_label),
                )
            });
        el.style().refine(&self.style);
        el
    }
}

/// The popup (`<AudioDeviceSelectContent>`): without children it lists the
/// permission notice, the loading row, every device (unavailable ones
/// disabled), the "None" and disconnected rows, and "No devices found.".
/// Opens below the trigger (`alignItemWithTrigger={false}`).
#[derive(Default)]
pub struct AudioDeviceSelectContent {
    children: Vec<SelectNode>,
    style: StyleRefinement,
}

impl AudioDeviceSelectContent {
    pub fn new() -> Self {
        Self::default()
    }

    /// Custom content: items, groups, [`AudioDeviceSelectItem`]s or elements.
    pub fn child(mut self, node: impl Into<SelectNode>) -> Self {
        self.children.push(node.into());
        self
    }

    pub fn children(mut self, nodes: impl IntoIterator<Item = impl Into<SelectNode>>) -> Self {
        self.children.extend(nodes.into_iter().map(Into::into));
        self
    }

    fn into_select_content(self, ctx: &DeviceSelectContext, cx: &App) -> SelectContent {
        let mut content = SelectContent::new().align_item_with_trigger(false);
        content.style().refine(&self.style);
        if !self.children.is_empty() {
            return content.children(self.children);
        }
        let needs_permission = ctx.permission != AudioPermission::Granted;
        let mut group = SelectGroup::new();
        if needs_permission {
            let mut notice = AudioDeviceSelectPermission::new().permission(ctx.permission);
            notice.on_request_permission = ctx.on_request_permission.clone();
            group = group.child(notice.into_any_element());
        }
        if ctx.loading {
            group = group.child(SelectItem::new(LOADING_VALUE, "Finding devices…").disabled(true));
        }
        for item in ctx.items.iter() {
            if item.none || item.missing {
                group = group.child(
                    SelectItem::new(item.value.clone(), item.label.clone()).disabled(item.missing),
                );
                continue;
            }
            let status = item.device.as_ref().map(|d| d.status).unwrap_or_default();
            let mut row = AudioDeviceSelectItem::new(item.value.clone())
                .disabled(status != AudioDeviceStatus::Available);
            row.device = item.device.clone();
            group = group.child(row.into_select_item(cx));
        }
        if !ctx.loading && ctx.items.is_empty() && !needs_permission {
            group = group.child(
                div()
                    .p(px(8.0))
                    .text_xs_()
                    .text_color(cx.theme().muted_foreground)
                    .child("No devices found.")
                    .into_any_element(),
            );
        }
        content.child(group)
    }
}

impl Styled for AudioDeviceSelectContent {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// A framed area under the select for a level or waveform preview
/// (`<AudioDeviceSelectPreview>`): `bg-muted/20 rounded-lg border px-2`.
///
/// ```ignore
/// AudioDeviceSelectPreview::new().child(LiveWaveform::new("preview").source(visual).h(px(32.0)))
/// ```
#[derive(IntoElement, Default)]
pub struct AudioDeviceSelectPreview {
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl AudioDeviceSelectPreview {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ParentElement for AudioDeviceSelectPreview {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for AudioDeviceSelectPreview {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for AudioDeviceSelectPreview {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let mut el = div()
            .overflow_hidden()
            .rounded(radius_lg(t))
            .border_1()
            .border_color(t.border)
            .bg(t.muted.opacity(0.2))
            .px(px(8.0))
            .children(self.children);
        el.style().refine(&self.style);
        el
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(items: &[DeviceItem]) -> Vec<&str> {
        items.iter().map(|i| i.label.as_ref()).collect()
    }

    #[test]
    fn audio_device_items_show_the_selected_device() {
        let devices = [
            AudioDevice::new("default", "Built-in"),
            AudioDevice::new("usb", "USB mic"),
        ];
        let usb: SharedString = "usb".into();
        let (items, missing) = device_items(
            &devices,
            Some(&usb),
            false,
            &"None".into(),
            false,
            &HashMap::new(),
        );
        assert!(!missing);
        assert_eq!(labels(&items), ["Built-in", "USB mic"]);
        let selected = items.iter().find(|i| i.value == usb).unwrap();
        assert_eq!(selected.label.as_ref(), "USB mic");
    }

    #[test]
    fn audio_device_keeps_a_disconnected_device_visible() {
        let devices = [AudioDevice::new("a", "A")];
        let gone: SharedString = "gone".into();
        let (items, missing) = device_items(
            &devices,
            Some(&gone),
            false,
            &"None".into(),
            false,
            &HashMap::new(),
        );
        assert!(missing);
        let last = items.last().unwrap();
        assert!(last.missing);
        assert_eq!(last.label.as_ref(), "Unknown device (disconnected)");

        // A device seen before keeps its label.
        let mut known = HashMap::new();
        known.insert(gone.clone(), SharedString::from("Rode NT-USB"));
        let (items, _) = device_items(&devices, Some(&gone), false, &"None".into(), false, &known);
        assert_eq!(
            items.last().unwrap().label.as_ref(),
            "Rode NT-USB (disconnected)"
        );

        // Not while loading.
        let (items, missing) = device_items(
            &[],
            Some(&gone),
            false,
            &"None".into(),
            true,
            &HashMap::new(),
        );
        assert!(!missing);
        assert!(items.is_empty());
    }

    #[test]
    fn audio_device_shows_none_when_allowed() {
        let (items, missing) = device_items(
            &[],
            None,
            true,
            &"No microphone".into(),
            false,
            &HashMap::new(),
        );
        assert!(!missing);
        assert_eq!(items.len(), 1);
        assert!(items[0].none);
        assert_eq!(items[0].value.as_ref(), NONE_VALUE);
        assert_eq!(items[0].label.as_ref(), "No microphone");
    }

    #[test]
    fn audio_device_defaults_and_permission() {
        let s = AudioDeviceSelect::new("x", []);
        assert_eq!(s.none_label.as_ref(), "None");
        assert_eq!(s.permission, AudioPermission::Granted);
        assert!(!s.allow_none && !s.loading && !s.disabled);
        assert_eq!(
            effective_permission(AudioPermission::Unsupported),
            AudioPermission::Denied
        );
        assert_eq!(
            AudioDeviceSelectValue::new().placeholder.as_ref(),
            "Select a device"
        );
        assert_eq!(
            AudioDeviceSelectPermission::new().action_label.as_ref(),
            "Allow access"
        );
        let d = AudioDevice::new("busy", "Elgato Wave:3")
            .with_status(AudioDeviceStatus::Unavailable)
            .with_description("In use by another app");
        assert_eq!(d.kind, AudioDeviceKind::AudioInput);
        assert_eq!(d.description.as_deref(), Some("In use by another app"));
    }
}
