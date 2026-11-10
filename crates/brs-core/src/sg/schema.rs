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
fn empty_aa() -> Value {
    Value::object(Object::AssocArray(AssocArray::new()))
}
fn empty_array() -> Value {
    Value::object(Object::Array(Vec::new()))
}

/// Parent of a built-in type; `None` for `Node` and unknown names.
pub fn parent(name: &str) -> Option<&'static str> {
    Some(match name.to_ascii_lowercase().as_str() {
        "node" => return None,
        "roku_analytics:analyticsnode" => "Node",
        "group" | "timer" | "task" | "contentnode" | "font" | "animationbase" | "audio"
        | "channelstore" | "componentlibrary" | "soundeffect" => "Node",
        "labellist" | "markuplist" | "markupgrid" | "postergrid" | "rowlist" | "zoomrowlist" => {
            "ArrayGrid"
        }
        "checklist" | "radiobuttonlist" => "LabelList",
        "standardkeyboarddialog" | "standardmessagedialog" | "standardpinpaddialog" => {
            "StandardDialog"
        }
        "keyboarddialog" | "progressdialog" | "pinpaddialog" => "Dialog",
        "rectangle" | "label" | "poster" | "scene" | "layoutgroup" | "maskgroup"
        | "busyspinner" | "video" | "simplelabel" | "multistylelabel" | "keyboard"
        | "minikeyboard" | "textedit box" | "texteditbox" | "pinpad" | "dialog"
        | "standarddialog" | "overhang" | "infopane" | "button" | "targetgroup" | "arraygrid"
        | "scrollinglabel" | "dynamickeyboard" => "Group",
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
        "DynamicKeyboard",
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
        "Roku_Analytics:AnalyticsNode",
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
            // Not a Group field: Video (video/lifecycle) has no `effect`.
            spec("effect", Node, invalid),
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
            spec("drmParams", AssocArray, empty_aa),
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
        // Defaults recorded on the device (video/lifecycle "-- defaults").
        // Node-valued UI parts (bufferingBar, trickPlayBar...) and the native
        // agents are created by `video::initialize`.
        "Video" => vec![
            spec("actionButtonRow", Node, invalid),
            spec("allowOptionsKeyOverride", Boolean, no),
            spec("alwaysShowVideoPlanes", Boolean, no),
            spec("asyncStopSemantics", Boolean, no),
            spec("audioChannelsCount", Integer, zero_i),
            spec("audioFormat", String, empty),
            spec("audioSelectionPreferences", AssocArray, empty_aa),
            spec("audioTrack", String, empty),
            spec("autoPlayAfterSeek", Boolean, yes),
            spec("availableAudioTracks", Array, empty_array),
            spec("availableSubtitleTracks", Array, empty_array),
            spec("bifDisplay", Node, invalid),
            spec("bookmarkerExitURL", String, empty),
            spec("bottomRowButtonFocused", Integer, zero_i),
            spec("bottomRowButtonSelected", Integer, zero_i),
            spec("bottomRowButtons", AssocArray, invalid),
            spec("bufferingBar", Node, invalid),
            spec("bufferingBarVisibilityAuto", Boolean, yes),
            spec("bufferingBarVisibilityBounds", AssocArray, invalid),
            spec("bufferingBarVisibilityHint", Boolean, no),
            spec("bufferingStatus", AssocArray, invalid),
            spec("bufferingTextColor", Integer, zero_i),
            spec("capFontPath", String, empty),
            spec("capFontSize", Integer, zero_i),
            spec("captionMode", String, || Value::string("Inherit")),
            spec("captionRenderArea", AssocArray, invalid),
            spec("captionStyle", AssocArray, invalid),
            spec("captionsAvailable", Boolean, no),
            spec("cdnSwitch", AssocArray, invalid),
            spec("cgms", String, || Value::string("norestriction")),
            spec("clipId", Integer, zero_i),
            spec("completedStreamInfo", AssocArray, invalid),
            spec("content", Node, invalid),
            spec("contentBlocked", Boolean, no),
            spec("contentClassifier", String, empty),
            spec("contentIndex", Integer, || Value::Integer(-1)),
            spec("contentIsPlaylist", Boolean, no),
            spec("control", String, || Value::string("none")),
            spec("controlExt", AssocArray, invalid),
            spec("currentAudioTrack", String, empty),
            spec("currentSubtitleTrack", String, empty),
            spec("customTitle", Node, invalid),
            spec("decoderStats", AssocArray, invalid),
            spec("detachableMediaPlayer", Boolean, no),
            spec("disableScreenSaver", Boolean, no),
            spec("dismissSeekWithBack", Boolean, no),
            spec("dolbyAtmosDetected", Boolean, no),
            spec("downloadedSegment", AssocArray, invalid),
            spec("drmHttpAgent", Object, invalid),
            spec("dtsVirtualXDetected", Boolean, no),
            spec("duration", Double, || Value::Double(0.0)),
            spec("effectiveAudioSelectionPreferences", Array, empty_array),
            spec("effectiveSubtitleSelectionPreferences", Array, empty_array),
            spec("enableDecoderStats", Boolean, no),
            spec("enableLiveAvailabilityWindow", Boolean, yes),
            spec("enablePivotNode", Boolean, yes),
            spec("enableScreenSaverWhilePlaying", Boolean, no),
            spec("enableThumbnailTilesDuringLive", Boolean, no),
            spec("enableTrickPlay", Boolean, yes),
            spec("enableTunerAbsoluteSeek", Boolean, no),
            spec("enableUI", Boolean, yes),
            spec("errorCode", Integer, zero_i),
            spec("errorInfo", AssocArray, invalid),
            spec("errorMsg", String, empty),
            spec("errorStr", String, empty),
            spec("focusButton", Integer, zero_i),
            spec("focusable", Boolean, yes),
            spec("focusedItem", String, empty),
            spec("gain", Float, || Value::Float(0.0)),
            spec("globalCaptionMode", String, || Value::string("On")),
            spec("height", Float, || Value::Float(0.0)),
            spec("interstitialPods", AssocArray, invalid),
            spec("interstitialPodsProcessingDone", AssocArray, invalid),
            spec("licenseStatus", AssocArray, invalid),
            spec("loop", Boolean, no),
            spec("manifestData", AssocArray, invalid),
            spec("maxVideoDecodeResolution", Array, empty_array),
            spec("mediaPlayerAgent", Object, invalid),
            spec("mute", Boolean, no),
            spec("nextContentIndex", Integer, || Value::Integer(-1)),
            spec("notificationInterval", Float, || Value::Float(0.5)),
            spec("pauseBufferChanged", Boolean, no),
            spec("pauseBufferClipId", Integer, zero_i),
            spec("pauseBufferEnd", Double, || Value::Double(0.0)),
            spec("pauseBufferEndMargin", Float, || Value::Float(0.0)),
            spec("pauseBufferEpochOffset", Double, || Value::Double(0.0)),
            spec("pauseBufferOverflow", Boolean, no),
            spec("pauseBufferPosition", Double, || Value::Double(0.0)),
            spec("pauseBufferStart", Double, || Value::Double(0.0)),
            spec("pauseBufferStartMargin", Float, || Value::Float(0.0)),
            spec("pivotNode", Node, invalid),
            spec("playPauseWithSelect", Boolean, no),
            spec("playStartInfo", AssocArray, invalid),
            spec(
                "playbackActionButtonFocusIndicatorBlendColor",
                Integer,
                || Value::Integer(-1),
            ),
            spec("playbackActionButtonFocused", Integer, zero_i),
            spec("playbackActionButtonFocusedTextColor", Integer, || {
                Value::Integer(-1)
            }),
            spec("playbackActionButtonFocusedTextFont", Node, invalid),
            spec("playbackActionButtonHeight", Float, || Value::Float(0.0)),
            spec("playbackActionButtonSelected", Integer, zero_i),
            spec("playbackActionButtonUnfocusedTextColor", Integer, || {
                Value::Integer(-1)
            }),
            spec("playbackActionButtonUnfocusedTextFont", Node, invalid),
            spec("playbackActionButtons", AssocArray, invalid),
            spec("playbackSpeed", Float, || Value::Float(1.0)),
            spec("playbackUI", Node, invalid),
            spec("position", Double, || Value::Double(0.0)),
            spec("positionInfo", AssocArray, invalid),
            spec("resolution", String, empty),
            spec("retrievingBar", Node, invalid),
            spec("retrievingBarVisibilityAuto", Boolean, yes),
            spec("retrievingBarVisibilityBounds", AssocArray, invalid),
            spec("retrievingBarVisibilityHint", Boolean, no),
            spec("retrievingTextColor", Integer, zero_i),
            spec("seamlessAudioTrackSelection", Boolean, no),
            spec("seek", Double, || Value::Double(0.0)),
            spec("seekClip", AssocArray, invalid),
            spec("seekMode", String, || Value::string("default")),
            spec("showUI", AssocArray, invalid),
            spec("singleForegroundVideo", Boolean, no),
            spec("state", String, || Value::string("none")),
            spec("stateInfo", AssocArray, invalid),
            spec("streamInfo", AssocArray, invalid),
            spec("streamingSegment", AssocArray, invalid),
            spec("subtitleSelectionPreferences", AssocArray, empty_aa),
            spec("subtitleTrack", String, empty),
            spec("supplementaryAudioVolume", Integer, || Value::Integer(50)),
            spec("suppressCaptions", Boolean, no),
            spec("syncTrickplayWithControl", Boolean, no),
            spec("teletextTrack", String, empty),
            spec("thumbnailTiles", AssocArray, invalid),
            spec("timeToStartStreaming", Double, || Value::Double(0.0)),
            spec("timedMetaData", AssocArray, invalid),
            spec("timedMetaData2", AssocArray, invalid),
            spec("timedMetaDataSelectionKeys", Array, empty_array),
            spec("tracks", Array, empty_array),
            spec("trickPlayBackground", Node, invalid),
            spec("trickPlayBackgroundOverlay", String, empty),
            spec("trickPlayBar", Node, invalid),
            spec("trickPlayBarVisibilityAuto", Boolean, yes),
            spec("trickPlayBarVisibilityHint", Boolean, no),
            spec("trickPlayMode", String, empty),
            spec("trickPlayPosition", Double, || Value::Double(0.0)),
            spec("uiMetadata", AssocArray, invalid),
            spec("videoFormat", String, empty),
            spec("videoTrack", String, empty),
            spec("warningInfo", AssocArray, invalid),
            spec("width", Float, || Value::Float(0.0)),
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
        "Roku_Analytics:AnalyticsNode" => vec![
            spec("debug", Boolean, no),
            spec("init", AssocArray, invalid),
            spec("trackEvent", AssocArray, invalid),
            spec("initVideoPlayer", AssocArray, invalid),
            spec("setContentMetadata", AssocArray, invalid),
            spec("finishedVideoPlayback", AssocArray, invalid),
        ],
        "ChannelStore" => {
            let mut fields = vec![
                spec("command", String, empty),
                spec("requestedUserData", String, || Value::string("all")),
                spec("channelCredData", String, empty),
                spec("nonce", String, empty),
                spec("deltaOrder", AssocArray, invalid),
            ];
            for name in [
                "channelCred",
                "storeChannelCredDataStatus",
                "userData",
                "userRegionData",
                "catalog",
                "storeCatalog",
                "purchases",
                "order",
                "orderStatus",
                "requestedUserDataInfo",
                "requestPartnerOrder",
                "confirmPartnerOrder",
                "requestPartnerOrderStatus",
                "confirmPartnerOrderStatus",
                "deviceAttestationToken",
            ] {
                fields.push(spec(name, Node, invalid));
            }
            for field in &mut fields {
                // Repeating a command is a new request; cancellation must also notify.
                if field.name == "command" || matches!(field.name, "userData" | "userRegionData") {
                    field.always_notify = true;
                }
            }
            fields
        }
        "BusySpinner" => vec![
            spec("poster", Node, invalid),
            spec("uri", Uri, empty),
            spec("clockwise", Boolean, yes),
            spec("spinInterval", Time, || Value::Double(2.0)),
            spec("control", String, || Value::string("none")),
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
            spec("wrapDividerBitmapUri", Uri, empty),
            spec("wrapDividerWidth", Float, zero_f),
            spec("wrapDividerHeight", Float, zero_f),
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
        "MarkupGrid" => vec![
            spec("wrapDividerHeight", Float, || Value::Float(54.0)),
            spec("drawFocusFeedbackOnTop", Boolean, yes),
        ],
        "LabelList" | "MarkupList" => vec![
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
        "RadioButtonList" => vec![
            spec("focusedColor", Color, || Value::Integer(0x0000_00FF)),
            spec("checkedItem", Integer, || Value::Integer(-1)),
            spec("checkOnSelect", Boolean, yes),
            spec("checkedIconUri", Uri, empty),
            spec("focusedCheckedIconUri", Uri, empty),
        ],
        "CheckList" => vec![
            spec("checkedState", Array, empty_array),
            spec("checkOnSelect", Boolean, yes),
            spec("checkedIconUri", Uri, empty),
            spec("focusedCheckedIconUri", Uri, empty),
            spec("uncheckedIconUri", Uri, empty),
            spec("focusedUncheckedIconUri", Uri, empty),
        ],
        "PosterGrid" => vec![],
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
        "MiniKeyboard" => vec![
            spec("focusable", Boolean, yes),
            spec("text", String, empty),
            spec("textEditBox", Node, invalid),
            spec("showTextEditBox", Boolean, yes),
            spec("lowerCase", Boolean, yes),
            spec("keyColor", Color, white),
            spec("focusedKeyColor", Color, white),
            spec("keyboardBitmapUri", Uri, empty),
            spec("focusBitmapUri", Uri, empty),
        ],
        "TextEditBox" => vec![
            spec("focusable", Boolean, yes),
            spec("text", String, empty),
            spec("hintText", String, empty),
            spec("maxTextLength", Integer, || Value::Integer(15)),
            spec("cursorPosition", Integer, zero_i),
            spec("clearOnDownKey", Boolean, yes),
            spec("active", Boolean, no),
            spec("secureMode", Boolean, no),
            spec("textColor", Color, white),
            spec("hintTextColor", Color, white),
            spec("backgroundUri", Uri, empty),
            spec("leadingEllipsis", Boolean, no),
            spec("width", Float, || Value::Float(-1.0)),
        ],
        "Keyboard" | "DynamicKeyboard" => vec![
            spec("keyboardDomain", String, || Value::string("generic")),
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
        "StandardKeyboardDialog" => vec![
            spec("text", String, empty),
            spec("textEditBox", Node, invalid),
            spec("keyboardDomain", String, || Value::string("generic")),
        ],
        "Dialog"
        | "StandardDialog"
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
        "InfoPane" => vec![
            spec("width", Float, || Value::Float(780.0)),
            spec("height", Float, || Value::Float(300.0)),
            spec("infoText", String, empty),
            spec("infoText2", String, empty),
            spec("bulletText", Array, empty_array),
            spec("textColor", Color, white),
            spec("infoText2Color", Color, white),
            spec("infoText2BottomAlign", Boolean, no),
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
