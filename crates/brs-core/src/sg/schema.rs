//! Built-in node types: parent type and the fields each level declares.
//! Field order matches what the device prints (fidelity/scenegraph/node-api):
//! the most derived level first, `Node` last. Defaults not covered by a
//! golden are provisional.

use super::{float_array, rect_value, FieldKind};
use crate::value::{AssocArray, Object, Value};

pub struct FieldSpec {
    pub name: &'static str,
    pub kind: FieldKind,
    pub default: fn() -> Value,
    pub always_notify: bool,
}

const fn spec(name: &'static str, kind: FieldKind, default: fn() -> Value) -> FieldSpec {
    FieldSpec {
        name,
        kind,
        default,
        always_notify: false,
    }
}

fn empty() -> Value {
    Value::string("")
}
fn invalid() -> Value {
    Value::Invalid
}
fn yes() -> Value {
    Value::Boolean(true)
}
fn no() -> Value {
    Value::Boolean(false)
}
fn zero_f() -> Value {
    Value::Float(0.0)
}
/// Label `lineSpacing` default (scenegraph/label-wrap, FHD channel).
fn line_spacing() -> Value {
    Value::Float(12.0)
}
fn one_f() -> Value {
    Value::Float(1.0)
}
fn zero_i() -> Value {
    Value::Integer(0)
}
fn white() -> Value {
    Value::Integer(-1)
}
fn origin() -> Value {
    float_array(&[0.0, 0.0])
}
fn unit_scale() -> Value {
    float_array(&[1.0, 1.0])
}
fn zero_rect() -> Value {
    rect_value(0.0, 0.0, 0.0, 0.0)
}
fn change() -> Value {
    let mut aa = AssocArray::new();
    aa.set("Index1", Value::Integer(0));
    aa.set("Index2", Value::Integer(0));
    aa.set("Operation", Value::string("none"));
    Value::object(Object::AssocArray(aa))
}
fn empty_array() -> Value {
    Value::object(Object::Array(Vec::new()))
}

/// Parent of a built-in type; `None` for `Node` and unknown names.
pub fn parent(name: &str) -> Option<&'static str> {
    Some(match name.to_ascii_lowercase().as_str() {
        "node" => return None,
        "group" | "timer" | "task" | "contentnode" | "font" | "animationbase" | "audio"
        | "channelstore" | "componentlibrary" | "soundeffect" => "Node",
        "labellist" | "markuplist" | "markupgrid" | "postergrid" | "rowlist" | "zoomrowlist"
        | "checklist" | "radiobuttonlist" => "ArrayGrid",
        "rectangle"
        | "label"
        | "poster"
        | "scene"
        | "layoutgroup"
        | "maskgroup"
        | "busyspinner"
        | "video"
        | "simplelabel"
        | "multistylelabel"
        | "keyboard"
        | "minikeyboard"
        | "textedit box"
        | "texteditbox"
        | "pinpad"
        | "dialog"
        | "standarddialog"
        | "overhang"
        | "infopane"
        | "button"
        | "targetgroup"
        | "arraygrid"
        | "scrollinglabel"
        | "standardkeyboarddialog"
        | "standardmessagedialog"
        | "standardpinpaddialog"
        | "keyboarddialog"
        | "progressdialog"
        | "pinpaddialog" => "Group",
        "buttongroup" => "LayoutGroup",
        "animation" | "sequentialanimation" | "parallelanimation" => "AnimationBase",
        "floatfieldinterpolator" | "vector2dfieldinterpolator" | "colorfieldinterpolator" => "Node",
        _ => return None,
    })
}

/// Canonical spelling of a built-in type, or `None` if unknown.
pub fn canonical(name: &str) -> Option<&'static str> {
    const NAMES: &[&str] = &[
        "Node",
        "Group",
        "Rectangle",
        "Label",
        "Poster",
        "Scene",
        "LayoutGroup",
        "ButtonGroup",
        "MaskGroup",
        "BusySpinner",
        "Video",
        "Audio",
        "SimpleLabel",
        "MultiStyleLabel",
        "ScrollingLabel",
        "RowList",
        "ZoomRowList",
        "LabelList",
        "MarkupList",
        "MarkupGrid",
        "PosterGrid",
        "ArrayGrid",
        "TargetGroup",
        "Keyboard",
        "MiniKeyboard",
        "TextEditBox",
        "PinPad",
        "Dialog",
        "StandardDialog",
        "StandardKeyboardDialog",
        "StandardMessageDialog",
        "StandardPinPadDialog",
        "KeyboardDialog",
        "ProgressDialog",
        "PinPadDialog",
        "Overhang",
        "InfoPane",
        "Button",
        "CheckList",
        "RadioButtonList",
        "Timer",
        "Task",
        "ContentNode",
        "Font",
        "AnimationBase",
        "Animation",
        "SequentialAnimation",
        "ParallelAnimation",
        "FloatFieldInterpolator",
        "Vector2DFieldInterpolator",
        "ColorFieldInterpolator",
        "ChannelStore",
        "ComponentLibrary",
        "SoundEffect",
    ];
    NAMES.iter().copied().find(|n| n.eq_ignore_ascii_case(name))
}

/// Fields declared by one level of a built-in type.
pub fn fields(name: &str) -> Vec<FieldSpec> {
    use FieldKind::*;
    match name {
        "Node" => vec![
            spec("id", String, empty),
            spec("focusable", Boolean, no),
            spec("focusedChild", Node, invalid),
            spec("change", AssocArray, change),
        ],
        "Group" => vec![
            spec("effect", Node, invalid),
            spec("visible", Boolean, yes),
            spec("opacity", Float, one_f),
            spec("clippingRect", Rect2D, zero_rect),
            spec("translation", Vector2D, origin),
            spec("scale", Vector2D, unit_scale),
            spec("rotation", Float, zero_f),
            spec("scaleRotateCenter", Vector2D, origin),
            spec("childRenderOrder", String, || Value::string("last")),
            spec("renderPass", Integer, zero_i),
            spec("inheritParentTransform", Boolean, yes),
            spec("inheritParentOpacity", Boolean, yes),
            spec("muteAudioGuide", Boolean, no),
            spec("enableRenderTracking", Boolean, no),
            spec("renderTracking", String, || Value::string("disabled")),
        ],
        "Rectangle" => vec![
            spec("width", Float, zero_f),
            spec("height", Float, zero_f),
            spec("color", Color, white),
            spec("blendingEnabled", Boolean, yes),
        ],
        "Label" | "SimpleLabel" | "ScrollingLabel" | "MultiStyleLabel" => vec![
            spec("text", String, empty),
            spec("color", Color, || Value::Integer(0xDDDD_DDFFu32 as i32)),
            spec("font", Font, || Value::string("font:MediumSystemFont")),
            spec("horizAlign", String, || Value::string("left")),
            spec("vertAlign", String, || Value::string("top")),
            spec("width", Float, zero_f),
            spec("height", Float, zero_f),
            spec("numLines", Integer, zero_i),
            spec("maxLines", Integer, zero_i),
            spec("wrap", Boolean, no),
            spec("lineSpacing", Float, line_spacing),
            spec("displayPartialLines", Boolean, no),
            spec("ellipsizeOnBoundary", Boolean, no),
            spec("truncateOnDelimiter", String, empty),
            spec("wordBreakChars", String, empty),
            spec("ellipsisText", String, empty),
            spec("isTextEllipsized", Boolean, no),
        ],
        "Poster" => vec![
            spec("uri", Uri, empty),
            spec("width", Float, zero_f),
            spec("height", Float, zero_f),
            spec("loadSync", Boolean, no),
            spec("loadWidth", Float, zero_f),
            spec("loadHeight", Float, zero_f),
            spec("loadDisplayMode", String, || Value::string("noScale")),
            spec("loadStatus", String, || Value::string("none")),
            spec("bitmapWidth", Float, zero_f),
            spec("bitmapHeight", Float, zero_f),
            spec("bitmapMargins", AssocArray, invalid),
            spec("blendColor", Color, white),
            spec("loadingBitmapUri", Uri, empty),
            spec("loadingBitmapOpacity", Float, one_f),
            spec("failedBitmapUri", Uri, empty),
            spec("failedBitmapOpacity", Float, one_f),
            spec("audioGuideText", String, empty),
        ],
        "Scene" => vec![
            spec("backgroundUri", Uri, empty),
            spec("backgroundColor", Color, || Value::Integer(0x0000_00FF)),
            spec("backExitsScene", Boolean, yes),
            spec("dialog", Node, invalid),
            spec("currentDesignResolution", AssocArray, invalid),
            spec("palette", Node, invalid),
        ],
        "ButtonGroup" => vec![
            spec("buttons", Array, empty_array),
            spec("buttonFocused", Integer, zero_i),
            FieldSpec {
                name: "buttonSelected",
                kind: Integer,
                default: zero_i,
                always_notify: true,
            },
            FieldSpec {
                name: "focusButton",
                kind: Integer,
                default: zero_i,
                always_notify: true,
            },
            spec("focusable", Boolean, yes),
            spec("minWidth", Float, zero_f),
            spec("maxWidth", Float, || Value::Float(32767.0)),
            spec("buttonHeight", Float, zero_f),
            spec("textColor", Color, white),
            spec("focusedTextColor", Color, white),
            spec("textFont", Font, || Value::string("font:MediumSystemFont")),
            spec("focusedTextFont", Font, || {
                Value::string("font:MediumBoldSystemFont")
            }),
        ],
        "LayoutGroup" => vec![
            spec("layoutDirection", String, || Value::string("vert")),
            spec("horizAlignment", String, || Value::string("left")),
            spec("vertAlignment", String, || Value::string("top")),
            spec("itemSpacings", Array, empty_array),
            spec("addItemSpacingAfterChild", Boolean, yes),
        ],
        "Timer" => vec![
            spec("control", String, || Value::string("none")),
            spec("repeat", Boolean, no),
            spec("duration", Float, zero_f),
            FieldSpec {
                name: "fire",
                kind: Trigger,
                default: invalid,
                always_notify: true,
            },
        ],
        "Task" => vec![
            spec("functionName", String, empty),
            spec("control", String, || Value::string("init")),
            spec("state", String, || Value::string("init")),
        ],
        "ContentNode" => vec![
            spec("title", String, empty),
            spec("description", String, empty),
            spec("url", Uri, empty),
            spec("streamFormat", String, empty),
            spec("HDPosterUrl", Uri, empty),
            spec("SDPosterUrl", Uri, empty),
            spec("FHDPosterUrl", Uri, empty),
            spec("ShortDescriptionLine1", String, empty),
            spec("ShortDescriptionLine2", String, empty),
        ],
        "Font" => vec![
            spec("uri", Uri, empty),
            spec("size", Integer, || Value::Integer(24)),
            spec("fallbackGlyph", String, empty),
        ],
        "Video" => vec![
            spec("content", Node, invalid),
            spec("control", String, || Value::string("none")),
            spec("state", String, || Value::string("none")),
            spec("position", Double, || Value::Double(0.0)),
            spec("duration", Double, || Value::Double(0.0)),
            spec("width", Float, zero_f),
            spec("height", Float, zero_f),
            spec("errorCode", Integer, zero_i),
            spec("errorMsg", String, empty),
        ],
        "AnimationBase" => vec![
            spec("control", String, || Value::string("none")),
            spec("state", String, || Value::string("stopped")),
            spec("repeat", Boolean, no),
            spec("delay", Float, zero_f),
        ],
        "Animation" => vec![
            spec("duration", Float, zero_f),
            spec("easeFunction", String, || Value::string("outCubic")),
        ],
        "FloatFieldInterpolator" | "Vector2DFieldInterpolator" | "ColorFieldInterpolator" => {
            vec![
                spec("key", Array, empty_array),
                spec("keyValue", Array, empty_array),
                spec("fieldToInterp", String, empty),
                spec("fraction", Float, zero_f),
                spec("reverse", Boolean, no),
            ]
        }
        "BusySpinner" => vec![
            spec("uri", Uri, empty),
            spec("spinInterval", Float, || Value::Float(2.0)),
            spec("control", String, || Value::string("start")),
        ],
        "MaskGroup" => vec![
            spec("maskUri", Uri, empty),
            spec("maskSize", Vector2D, origin),
            spec("maskOffset", Vector2D, origin),
        ],
        "ArrayGrid" => vec![
            spec("content", Node, invalid),
            spec("itemComponentName", String, empty),
            spec("itemSize", Vector2D, origin),
            spec("itemSpacing", Vector2D, origin),
            spec("numRows", Integer, || Value::Integer(1)),
            spec("numColumns", Integer, || Value::Integer(1)),
            spec("itemFocused", Integer, || Value::Integer(-1)),
            spec("itemUnfocused", Integer, || Value::Integer(-1)),
            FieldSpec {
                name: "itemSelected",
                kind: Integer,
                default: || Value::Integer(-1),
                always_notify: true,
            },
            FieldSpec {
                name: "jumpToItem",
                kind: Integer,
                default: zero_i,
                always_notify: true,
            },
            FieldSpec {
                name: "animateToItem",
                kind: Integer,
                default: zero_i,
                always_notify: true,
            },
            spec("focusable", Boolean, yes),
            spec("focusRow", Integer, zero_i),
            spec("focusColumn", Integer, zero_i),
            spec("vertFocusAnimationStyle", String, || {
                Value::string("fixedfocuswrap")
            }),
            spec("horizFocusAnimationStyle", String, || {
                Value::string("floatingfocus")
            }),
            spec("drawFocusFeedback", Boolean, yes),
            spec("drawFocusFeedbackOnTop", Boolean, no),
            spec("focusBitmapUri", Uri, empty),
            spec("focusBitmapBlendColor", Color, white),
            spec("focusFootprintBitmapUri", Uri, empty),
            spec("focusFootprintBlendColor", Color, white),
            spec("rowHeights", Array, empty_array),
            spec("columnWidths", Array, empty_array),
            spec("rowSpacings", Array, empty_array),
            spec("columnSpacings", Array, empty_array),
        ],
        "LabelList" | "MarkupList" | "CheckList" | "RadioButtonList" => vec![
            spec("itemSize", Vector2D, || float_array(&[510.0, 72.0])),
            spec("numRows", Integer, || Value::Integer(12)),
            spec("color", Color, || Value::Integer(0xFEFE_FEFFu32 as i32)),
            spec("focusedColor", Color, || Value::Integer(0x2626_26FF)),
            spec("font", Font, || Value::string("font:MediumSystemFont")),
            spec("focusedFont", Font, || {
                Value::string("font:MediumBoldSystemFont")
            }),
            spec("textHorizAlign", String, || Value::string("left")),
        ],
        "MarkupGrid" | "PosterGrid" => vec![],
        "RowList" | "ZoomRowList" => vec![
            spec("rowItemFocused", Array, empty_array),
            FieldSpec {
                name: "rowItemSelected",
                kind: Array,
                default: empty_array,
                always_notify: true,
            },
            FieldSpec {
                name: "jumpToRowItem",
                kind: Array,
                default: empty_array,
                always_notify: true,
            },
            spec("vertFocusAnimationStyle", String, || {
                Value::string("fixedfocus")
            }),
            spec("rowItemSize", Array, empty_array),
            spec("rowItemSpacing", Array, empty_array),
            spec("rowFocusAnimationStyle", Array, empty_array),
            spec("showRowLabel", Array, empty_array),
            spec("rowLabelOffset", Array, empty_array),
            spec("rowLabelFont", Font, || {
                Value::string("font:MediumSystemFont")
            }),
            spec("rowLabelColor", Color, white),
            spec("rowLabelHeight", Float, || Value::Float(40.0)),
            spec("rowHeights", Array, empty_array),
            spec("rowSpacings", Array, empty_array),
        ],
        "Button" => vec![
            spec("focusable", Boolean, yes),
            spec("text", String, empty),
            spec("iconUri", Uri, empty),
            spec("focusedIconUri", Uri, empty),
            spec("minWidth", Float, zero_f),
            spec("maxWidth", Float, || Value::Float(32767.0)),
            spec("height", Float, || Value::Float(72.0)),
            spec("textColor", Color, || Value::Integer(0xDDDD_DDFFu32 as i32)),
            spec("focusedTextColor", Color, || Value::Integer(0x2626_26FF)),
            spec("textFont", Font, || Value::string("font:MediumSystemFont")),
            spec("focusedTextFont", Font, || {
                Value::string("font:MediumBoldSystemFont")
            }),
            spec("showFocusFootprint", Boolean, no),
            spec("focusBitmapUri", Uri, empty),
            spec("focusFootprintBitmapUri", Uri, empty),
            FieldSpec {
                name: "buttonSelected",
                kind: Trigger,
                default: invalid,
                always_notify: true,
            },
        ],
        "Keyboard" | "MiniKeyboard" | "TextEditBox" => vec![
            spec("focusable", Boolean, yes),
            spec("text", String, empty),
            spec("textEditBox", Node, invalid),
            spec("showTextEditBox", Boolean, yes),
            spec("keyColor", Color, white),
            spec("focusedKeyColor", Color, white),
            spec("keyboardBitmapUri", Uri, empty),
            spec("focusBitmapUri", Uri, empty),
            spec("hintText", String, empty),
            spec("maxTextLength", Integer, || Value::Integer(75)),
            spec("cursorPosition", Integer, zero_i),
        ],
        "PinPad" => vec![
            spec("pin", String, empty),
            spec("pinLength", Integer, || Value::Integer(4)),
        ],
        "Dialog"
        | "StandardDialog"
        | "StandardKeyboardDialog"
        | "StandardMessageDialog"
        | "StandardPinPadDialog"
        | "KeyboardDialog"
        | "ProgressDialog"
        | "PinPadDialog" => vec![
            spec("title", String, empty),
            spec("message", String, empty),
            spec("buttons", Array, empty_array),
            FieldSpec {
                name: "close",
                kind: Boolean,
                default: no,
                always_notify: true,
            },
            FieldSpec {
                name: "wasClosed",
                kind: Trigger,
                default: invalid,
                always_notify: true,
            },
            spec("optionsDialog", Boolean, no),
            spec("buttonGroup", Node, invalid),
            spec("width", Float, || Value::Float(-1.0)),
            spec("maxHeight", Float, || Value::Float(-1.0)),
            spec("buttonSelected", Integer, zero_i),
            spec("buttonFocused", Integer, zero_i),
            spec("close", Boolean, no),
            spec("wasClosed", Boolean, no),
        ],
        "Overhang" => vec![
            spec("title", String, empty),
            spec("logoUri", Uri, empty),
            spec("showOptions", Boolean, no),
        ],
        "ComponentLibrary" => vec![
            spec("uri", Uri, empty),
            spec("loadStatus", String, || Value::string("none")),
        ],
        _ => Vec::new(),
    }
}
