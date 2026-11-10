use brs_core::{Channel, Host, Status};
use std::{cell::RefCell, rc::Rc};
struct Output(Rc<RefCell<String>>);
impl Host for Output {
    fn write(&mut self, text: &str) {
        self.0.borrow_mut().push_str(text);
    }
    fn now_ms(&self) -> f64 {
        0.0
    }
}
#[test]
fn optional_fields_preserve_single_evaluation_and_propagate_invalid() {
    let source = "sub Main()\n m.calls = 0\n print Fetch()?.value\n print m.calls\n missing = invalid\n print missing?.nested?.value\n print { rows: [\n 1\n 2\n] }?.rows.Count()\nend sub\nfunction Fetch()\n m.calls++\n return { value: 7 }\nend function";
    let channel = Channel::from_files(std::collections::BTreeMap::from([
        ("manifest".into(), b"title=Syntax test\n".to_vec()),
        ("source/main.brs".into(), source.as_bytes().to_vec()),
    ]))
    .unwrap();
    let output = Rc::new(RefCell::new(String::new()));
    let mut vm = brs_core::launch(&channel, Box::new(Output(output.clone()))).unwrap();
    assert_eq!(vm.run(10000), Status::Finished);
    assert!(output.borrow().contains("\n 7\n 1\ninvalid\n 2\n"));
}

#[test]
fn busy_spinner_exposes_a_usable_poster() {
    let source = r#"sub Main()
        spinner = CreateObject("roSGNode", "BusySpinner")
        print spinner.poster.subtype()
        spinner.poster.uri = "pkg:/images/missing.png"
        print spinner.poster.uri
        print spinner.poster.loadStatus
    end sub"#;
    let channel = Channel::from_files(std::collections::BTreeMap::from([
        ("manifest".into(), b"title=Spinner test\n".to_vec()),
        ("source/main.brs".into(), source.as_bytes().to_vec()),
    ]))
    .unwrap();
    let output = Rc::new(RefCell::new(String::new()));
    let mut vm = brs_core::launch(&channel, Box::new(Output(output.clone()))).unwrap();
    assert_eq!(vm.run(10000), Status::Finished);
    assert!(output
        .borrow()
        .contains("\nPoster\npkg:/images/missing.png\nloading\n"));
}

#[test]
fn channel_store_completes_repeated_offline_requests_without_credentials() {
    let source = r#"sub Main()
        store = CreateObject("roSGNode", "ChannelStore")
        port = CreateObject("roMessagePort")
        store.observeField("channelCred", port)
        for i = 1 to 2
            store.command = "getChannelCred"
            event = wait(0, port)
            result = event.getData()
            print result.status < 0
            print result.json = ""
            print result.errorCode <> ""
        end for
    end sub"#;
    let channel = Channel::from_files(std::collections::BTreeMap::from([
        ("manifest".into(), b"title=Offline store test\n".to_vec()),
        ("source/main.brs".into(), source.as_bytes().to_vec()),
    ]))
    .unwrap();
    let output = Rc::new(RefCell::new(String::new()));
    let mut vm = brs_core::launch(&channel, Box::new(Output(output.clone()))).unwrap();
    let mut status = Status::Waiting;
    for _ in 0..10 {
        status = vm.run(10000);
        if status == Status::Finished {
            break;
        }
    }
    assert_eq!(status, Status::Finished);
    assert!(output
        .borrow()
        .contains("\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\n"));
}

struct Clock(Rc<RefCell<String>>, Rc<std::cell::Cell<f64>>);
impl Host for Clock {
    fn write(&mut self, text: &str) {
        self.0.borrow_mut().push_str(text);
    }
    fn now_ms(&self) -> f64 {
        self.1.get()
    }
}

#[test]
fn busy_spinner_rotates_and_stops_with_uri_forwarding() {
    let source = r#"sub Main()
        spinner = CreateObject("roSGNode", "BusySpinner")
        spinner.uri = "pkg:/first.png"
        print spinner.poster.uri
        spinner.poster.uri = "pkg:/second.png"
        print spinner.uri
        sleep(1000)
        print abs(spinner.poster.rotation + 3.14159265) < 0.001
        spinner.control = "stop"
        angle = spinner.poster.rotation
        sleep(1000)
        print spinner.poster.rotation = angle
    end sub"#;
    let channel = Channel::from_files(std::collections::BTreeMap::from([
        ("manifest".into(), b"title=Spinner clock test\n".to_vec()),
        ("source/main.brs".into(), source.as_bytes().to_vec()),
    ]))
    .unwrap();
    let output = Rc::new(RefCell::new(String::new()));
    let now = Rc::new(std::cell::Cell::new(0.0));
    let mut vm = brs_core::launch(&channel, Box::new(Clock(output.clone(), now.clone()))).unwrap();
    assert!(matches!(vm.run(10000), Status::Sleeping { .. }));
    now.set(1000.0);
    assert!(matches!(vm.run(10000), Status::Sleeping { .. }));
    now.set(2000.0);
    assert_eq!(vm.run(10000), Status::Finished);
    assert!(output
        .borrow()
        .contains("\npkg:/first.png\npkg:/first.png\ntrue\ntrue\n"));
}

#[test]
fn typed_parameters_unbox_scenegraph_strings_and_booleans() {
    use brs_core::{ast::TypeName, value::Value, vm::coerce};
    let string = coerce(Value::boxed(Value::string("hello")), TypeName::String).unwrap();
    assert!(matches!(string, Value::String(ref s) if &**s == "hello"));
    let boolean = coerce(Value::boxed(Value::Boolean(true)), TypeName::Boolean).unwrap();
    assert!(matches!(boolean, Value::Boolean(true)));
    let boxed = Value::boxed(Value::string("object"));
    assert!(coerce(boxed.clone(), TypeName::Object).unwrap().is_boxed());
    assert!(coerce(boxed, TypeName::Integer).is_err());
}

#[test]
fn radio_selection_respects_check_on_select_and_keyboard_has_initial_bounds() {
    use brs_core::{
        sg::{runtime, widgets},
        value::Value,
    };
    let channel = Channel::from_files(std::collections::BTreeMap::from([
        ("manifest".into(), b"title=Widget test\n".to_vec()),
        ("source/main.brs".into(), b"sub Main()\nend sub".to_vec()),
    ]))
    .unwrap();
    let output = Rc::new(RefCell::new(String::new()));
    let mut vm = brs_core::launch(&channel, Box::new(Output(output))).unwrap();
    let radio = runtime::create_node(&mut vm, "RadioButtonList")
        .unwrap()
        .unwrap();
    let root = runtime::create_node(&mut vm, "ContentNode")
        .unwrap()
        .unwrap();
    for _ in 0..2 {
        runtime::node_method(
            &mut vm,
            &root,
            "createchild",
            vec![Value::string("ContentNode")],
        )
        .unwrap();
    }
    runtime::set_field(&mut vm, &radio, "content", Value::Object(root)).unwrap();
    runtime::set_field(&mut vm, &radio, "jumpToItem", Value::Integer(1)).unwrap();
    widgets::key(&mut vm, &radio, "OK", true).unwrap();
    assert_eq!(
        runtime::get_field(&radio, "checkedItem").as_f64(),
        Some(1.0)
    );
    runtime::set_field(&mut vm, &radio, "checkOnSelect", Value::Boolean(false)).unwrap();
    runtime::set_field(&mut vm, &radio, "jumpToItem", Value::Integer(0)).unwrap();
    widgets::key(&mut vm, &radio, "OK", true).unwrap();
    assert_eq!(
        runtime::get_field(&radio, "checkedItem").as_f64(),
        Some(1.0)
    );
    let keyboard = runtime::create_node(&mut vm, "Keyboard").unwrap().unwrap();
    let bounds = runtime::node_method(&mut vm, &keyboard, "boundingrect", vec![]).unwrap();
    let Value::Object(bounds) = bounds else {
        panic!("bounds expected")
    };
    let brs_core::Object::AssocArray(bounds) = &*bounds.borrow() else {
        panic!("bounds expected")
    };
    assert!(bounds.get("width").unwrap().as_f64().unwrap() > 0.0);
    assert!(bounds.get("height").unwrap().as_f64().unwrap() > 0.0);
}

#[test]
fn registry_tools_update_only_selected_keys_without_pausing_the_channel() {
    let channel = Channel::from_files(std::collections::BTreeMap::from([
        ("manifest".into(), b"title=Registry tool test\n".to_vec()),
        (
            "source/main.brs".into(),
            b"sub Main()\n port=CreateObject(\"roMessagePort\")\n event=wait(0,port)\nend sub"
                .to_vec(),
        ),
    ]))
    .unwrap();
    let output = Rc::new(RefCell::new(String::new()));
    let mut vm = brs_core::launch(&channel, Box::new(Output(output.clone()))).unwrap();
    assert_eq!(vm.run(10000), Status::Waiting);
    vm.env
        .registry
        .entry("test".into())
        .or_default()
        .insert("keep".into(), "existing".into());
    assert!(!vm.debugger_command(
        r#"__emu {"id":"edit","op":"registrySet","section":"test","key":"new","value":"updated"}"#
    ));
    assert_eq!(vm.env.registry["test"]["new"], "updated");
    assert_eq!(vm.env.registry["test"]["keep"], "existing");
    assert!(vm.env.registry_dirty);
    vm.debugger_command(
        r#"__emu {"id":"bad","op":"registrySet","section":"test","key":"new","value":123}"#,
    );
    assert_eq!(vm.env.registry["test"]["new"], "updated");
    vm.debugger_command(
        r#"__emu {"id":"delete","op":"registrySet","section":"test","key":"new","value":null}"#,
    );
    assert!(!vm.env.registry["test"].contains_key("new"));
    assert_eq!(vm.env.registry["test"]["keep"], "existing");
    assert_eq!(vm.run(10000), Status::Waiting);
    assert!(output
        .borrow()
        .contains("Registry values must be strings or null"));
}

#[test]
fn get_interface_autoboxes_intrinsics_for_type_guards() {
    let source = r#"sub Main()
        print GetInterface("demo", "ifString") <> invalid
        print GetInterface("demo", "ifArray") = invalid
        print GetInterface(Box("demo"), "ifString") <> invalid
        print GetInterface(invalid, "ifString") = invalid
    end sub"#;
    let channel = Channel::from_files(std::collections::BTreeMap::from([
        ("manifest".into(), b"title=Interface test\n".to_vec()),
        ("source/main.brs".into(), source.as_bytes().to_vec()),
    ]))
    .unwrap();
    let output = Rc::new(RefCell::new(String::new()));
    let mut vm = brs_core::launch(&channel, Box::new(Output(output.clone()))).unwrap();
    assert_eq!(vm.run(10000), Status::Finished);
    assert!(output.borrow().contains("\ntrue\ntrue\ntrue\ntrue\n"));
}

#[test]
fn focusing_the_same_node_twice_does_not_borrow_its_self_reference() {
    let source = r#"sub Main()
        node = CreateObject("roSGNode", "Group")
        node.setFocus(true)
        node.setFocus(true)
        print node.hasFocus()
        print node.focusedChild.isSameNode(node)
        node.addField("self", "node", false)
        node.self = node
        node.self = node
        print node.self.isSameNode(node)
    end sub"#;
    let channel = Channel::from_files(std::collections::BTreeMap::from([
        ("manifest".into(), b"title=Focus regression\n".to_vec()),
        ("source/main.brs".into(), source.as_bytes().to_vec()),
    ]))
    .unwrap();
    let output = Rc::new(RefCell::new(String::new()));
    let mut vm = brs_core::launch(&channel, Box::new(Output(output.clone()))).unwrap();
    assert_eq!(vm.run(10000), Status::Finished);
    assert!(output.borrow().contains("\ntrue\ntrue\ntrue\n"));
}

#[test]
fn poster_preserves_aspect_ratio_and_clips_zoom_without_changing_frame_bounds() {
    use brs_core::{
        sg::{render, runtime, ScreenState},
        value::ObjectRef,
        Value, Vm,
    };
    let channel = Channel::from_files(std::collections::BTreeMap::from([
        ("manifest".into(), b"title=Poster layout\n".to_vec()),
        ("source/main.brs".into(), b"sub Main()\nend sub".to_vec()),
    ]))
    .unwrap();
    let mut vm = brs_core::launch(
        &channel,
        Box::new(Output(Rc::new(RefCell::new(String::new())))),
    )
    .unwrap();
    // The device captures loadDisplayMode when uri is assigned
    // (fidelity/scenegraph/poster-display-mode), so the mode goes first.
    let poster_with_mode = |vm: &mut Vm, mode: &str| -> ObjectRef {
        let poster = runtime::create_node(vm, "Poster").unwrap().unwrap();
        runtime::set_field(vm, &poster, "loadDisplayMode", Value::string(mode)).unwrap();
        runtime::set_field(vm, &poster, "uri", Value::string("pkg:/aspect.png")).unwrap();
        for (name, value) in [
            ("width", 200.0),
            ("height", 100.0),
            ("bitmapWidth", 100.0),
            ("bitmapHeight", 100.0),
        ] {
            runtime::set_field(vm, &poster, name, Value::Float(value)).unwrap();
        }
        vm.sg.screens.clear();
        vm.sg.screens.push(ScreenState {
            scene: Some(poster.clone()),
            shown: true,
            ..Default::default()
        });
        poster
    };
    let display_list = |vm: &Vm| -> serde_json::Value {
        serde_json::from_str(&render::display_list(vm).unwrap()).unwrap()
    };

    let poster = poster_with_mode(&mut vm, "scaleToFit");
    let image = &display_list(&vm)["items"][0];
    assert_eq!(image[1], 50.0);
    assert_eq!(image[2], 0.0);
    assert_eq!(image[3], 100.0);
    assert_eq!(image[4], 100.0);
    assert_eq!(render::local_bounds(&vm, &poster).width, 200.0);

    // A mode assigned after the uri does not affect the loaded bitmap.
    runtime::set_field(
        &mut vm,
        &poster,
        "loadDisplayMode",
        Value::string("scaleToZoom"),
    )
    .unwrap();
    assert_eq!(display_list(&vm)["items"][0][3], 100.0);

    poster_with_mode(&mut vm, "scaleToZoom");
    let list = display_list(&vm);
    assert_eq!(list["items"][0], serde_json::json!(["c", 0, 0, 200, 100]));
    assert_eq!(list["items"][1][2], -50.0);
    assert_eq!(list["items"][1][3], 200.0);
    assert_eq!(list["items"][2], serde_json::json!(["C"]));
}

#[test]
fn host_text_input_edits_unicode_at_the_cursor_and_obeys_the_field_limit() {
    use brs_core::{
        sg::{runtime, widgets},
        Value,
    };
    let channel = Channel::from_files(std::collections::BTreeMap::from([
        ("manifest".into(), b"title=Text input test\n".to_vec()),
        ("source/main.brs".into(), b"sub Main()\nend sub".to_vec()),
    ]))
    .unwrap();
    let mut vm = brs_core::launch(
        &channel,
        Box::new(Output(Rc::new(RefCell::new(String::new())))),
    )
    .unwrap();
    let field = runtime::create_node(&mut vm, "TextEditBox")
        .unwrap()
        .unwrap();
    runtime::set_field(&mut vm, &field, "maxTextLength", Value::Integer(5)).unwrap();
    runtime::node_method(&mut vm, &field, "setfocus", vec![Value::Boolean(true)]).unwrap();
    assert!(widgets::text_input(&mut vm, "ação!extra", "").unwrap());
    assert_eq!(
        runtime::get_field(&field, "text").unboxed().as_str(),
        Some("ação!")
    );
    widgets::text_input(&mut vm, "", "Home").unwrap();
    widgets::text_input(&mut vm, "", "Delete").unwrap();
    widgets::text_input(&mut vm, "", "ArrowRight").unwrap();
    widgets::text_input(&mut vm, "", "Backspace").unwrap();
    widgets::text_input(&mut vm, "é", "").unwrap();
    assert_eq!(
        runtime::get_field(&field, "text").unboxed().as_str(),
        Some("éão!")
    );
    assert_eq!(
        runtime::get_field(&field, "cursorPosition").as_i64(),
        Some(1)
    );
    runtime::node_method(&mut vm, &field, "setfocus", vec![Value::Boolean(false)]).unwrap();
    assert!(!widgets::text_input(&mut vm, "ignored", "").unwrap());
}
