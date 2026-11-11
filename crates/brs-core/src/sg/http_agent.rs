//! SceneGraph HTTP-agent access and inherited request settings.
use super::{with_node, with_node_mut};
use crate::error::RuntimeError;
use crate::value::{ObjectRef, Value};
use crate::vm::Vm;

fn create(vm: &mut Vm) -> ObjectRef {
    match crate::runtime_objects::create(vm, "rohttpagent", &[])
        .expect("registered roHttpAgent")
        .expect("roHttpAgent construction cannot fail")
    {
        Value::Object(agent) => agent,
        _ => unreachable!(),
    }
}

pub fn get(vm: &mut Vm, node: &ObjectRef) -> ObjectRef {
    if let Some(agent) = with_node(node, |n| n.http_agent.clone()) {
        return agent;
    }
    let parent = with_node(node, |n| n.parent.as_ref().and_then(|p| p.upgrade()));
    if let Some(parent) = parent {
        return get(vm, &parent);
    }
    if let Some(agent) = &vm.sg.default_http_agent {
        return agent.clone();
    }
    let agent = create(vm);
    vm.sg.default_http_agent = Some(agent.clone());
    agent
}

pub fn call(
    vm: &mut Vm,
    node: &ObjectRef,
    name: &str,
    args: &[Value],
) -> Option<Result<Value, RuntimeError>> {
    match name {
        "gethttpagent" => Some(Ok(Value::Object(get(vm, node)))),
        "sethttpagent" => {
            let valid = args.first().and_then(|value| match value {
                Value::Object(agent) if agent.borrow().class_name() == "roHttpAgent" => {
                    Some(agent.clone())
                }
                _ => None,
            });
            let accepted = valid.is_some();
            if let Some(agent) = valid {
                with_node_mut(node, |n| n.http_agent = Some(agent));
            }
            Some(Ok(Value::Boolean(accepted)))
        }
        "addheader"
        | "setheaders"
        | "initclientcertificates"
        | "setcertificatesfile"
        | "setcertificatesdepth"
        | "enablecookies"
        | "getcookies"
        | "addcookies"
        | "clearcookies" => {
            let agent = get(vm, node);
            Some(crate::runtime_objects::call(vm, &agent, name, args))
        }
        _ => None,
    }
}
