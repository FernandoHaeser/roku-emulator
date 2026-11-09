//! BrightScript engine for the Roku emulator: lexer, parser, compiler and a
//! resumable bytecode VM. Host independent: compiled natively for the desktop
//! and CLI, and to WebAssembly for the browser.

pub mod ast;
pub mod builtins;
pub mod bytecode;
pub mod channel;
pub mod compiler;
pub mod components;
pub mod environment;
pub mod error;
pub mod json;
pub mod lexer;
pub mod md5;
pub mod ops;
pub mod parser;
pub mod sg;
pub mod value;
pub mod vm;
pub mod xml;

use std::rc::Rc;

pub use channel::{Channel, PackageError};
pub use value::{AssocArray, Object, Value};
pub use vm::{ChannelInfo, Host, Status, Vm};

use compiler::{compile_program, compile_scope, SourceFile};

/// Compiles a channel (source/ plus every SceneGraph component) and prepares
/// its entry point, ready for `Vm::run`.
pub fn launch(channel: &Channel, host: Box<dyn Host>) -> Result<Vm, String> {
    let mut files = Vec::new();
    for (path, text) in channel.source_files() {
        files.push(parse_file(&format!("pkg:/{path}"), &text)?);
    }
    let mut program = compile_program(files).map_err(|e| {
        format!(
            "{} (compile error &h02) in {}({})",
            e.message, e.file, e.line
        )
    })?;

    // SceneGraph components: one scope per component, holding the scripts of
    // its whole `extends` chain (base first, so derived functions override).
    let (definitions, component_errors) = sg::component::load_components(channel);
    let by_name: std::collections::HashMap<String, sg::ComponentDef> = definitions
        .into_iter()
        .map(|def| (def.name.to_lowercase(), def))
        .collect();
    let mut components = std::collections::HashMap::new();
    for (key, def) in &by_name {
        let mut chain = vec![def];
        while let Some(parent) = by_name.get(&chain.last().expect("chain").extends.to_lowercase()) {
            if chain.len() > 64 || chain.iter().any(|c| c.name == parent.name) {
                break;
            }
            chain.push(parent);
        }
        let mut levels = Vec::new();
        for level in chain.iter().rev() {
            let mut level_files = Vec::new();
            for script in &level.scripts {
                level_files.push(parse_file(&script.path, &script.text)?);
            }
            levels.push(level_files);
        }
        let level_refs: Vec<&[SourceFile]> = levels.iter().map(Vec::as_slice).collect();
        let (scope, own) = compile_scope(&mut program, &level_refs).map_err(|e| {
            format!(
                "{} (compile error &h02) in {}({})",
                e.message, e.file, e.line
            )
        })?;
        let mut compiled = def.clone();
        compiled.scope = scope;
        compiled.init = own.get("init").copied();
        components.insert(key.clone(), Rc::new(compiled));
    }

    let info = ChannelInfo {
        title: channel.title(),
        version: channel.version(),
        package_md5: channel.package_md5.clone(),
    };

    let entry = ["main", "runuserinterface"]
        .into_iter()
        .find(|name| program.function_index.contains_key(*name))
        .ok_or_else(|| "No Main() or RunUserInterface() entry point found".to_string())?;
    let takes_args = !program.functions[program.function_index[entry] as usize]
        .params
        .is_empty();

    let mut vm = Vm::new(program, host, info);
    vm.env.manifest = channel.manifest.clone();
    vm.env.files = Rc::new(channel.files.clone());
    vm.sg.components = components;
    let fhd = channel
        .manifest
        .get("ui_resolutions")
        .is_some_and(|r| r.to_lowercase().contains("fhd"));
    (vm.sg.design_width, vm.sg.design_height) = if fhd {
        (1920.0, 1080.0)
    } else {
        (1280.0, 720.0)
    };

    vm.write(&format!(
        "\n------ Running dev '{}' main ------\n",
        channel.title()
    ));
    for error in component_errors {
        vm.write(&format!("{error}\n"));
    }
    let args = if takes_args {
        vec![Value::object(Object::AssocArray(AssocArray::new()))]
    } else {
        Vec::new()
    };
    vm.start(entry, args)?;
    Ok(vm)
}

fn parse_file(device_path: &str, text: &str) -> Result<SourceFile, String> {
    let functions = parser::parse_source(text).map_err(|e| {
        format!(
            "{} (compile error &h02) in {device_path}({})",
            e.message, e.line
        )
    })?;
    Ok(SourceFile {
        path: Rc::from(device_path),
        text: Rc::from(text),
        functions,
    })
}
