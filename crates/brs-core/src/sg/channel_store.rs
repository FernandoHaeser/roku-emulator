//! Offline ChannelStore results. Roku account services are unavailable locally;
//! complete requests with an explicit error instead of leaving observers waiting.
use std::rc::{Rc, Weak};

use super::runtime::{create_node, set_field};
use super::{with_node, Object, ObjectRef, Value};
use crate::{error::RuntimeError, Vm};

#[derive(Default)]
pub struct ChannelStores {
    pending: Vec<(Weak<std::cell::RefCell<Object>>, String)>,
}

pub fn changed(vm: &mut Vm, node: &ObjectRef, field: &str) {
    if field.eq_ignore_ascii_case("command")
        && with_node(node, |n| n.ancestry.iter().any(|a| &**a == "ChannelStore"))
    {
        let command = with_node(node, |n| n.text("command").to_ascii_lowercase());
        vm.sg
            .channel_stores
            .pending
            .push((Rc::downgrade(node), command));
    }
}

pub fn service(vm: &mut Vm) -> Result<(), RuntimeError> {
    let pending = std::mem::take(&mut vm.sg.channel_stores.pending);
    for (node, command) in pending {
        let Some(node) = node.upgrade() else { continue };
        let field = match command.as_str() {
            "getchannelcred" => "channelCred",
            "storechannelcreddata" => "storeChannelCredDataStatus",
            "getcatalog" => "catalog",
            "getstorecatalog" => "storeCatalog",
            "getpurchases" | "getallpurchases" => "purchases",
            "doorder" => "orderStatus",
            "requestpartnerorder" => "requestPartnerOrderStatus",
            "confirmpartnerorder" => "confirmPartnerOrderStatus",
            "getdeviceattestationtoken" => "deviceAttestationToken",
            "getuserdata" => "userData",
            "getuserregiondata" => "userRegionData",
            _ => continue,
        };
        if matches!(field, "userData" | "userRegionData") {
            set_field(vm, &node, field, Value::Invalid)?;
            continue;
        }
        let result = create_node(vm, "ContentNode")?.expect("ContentNode");
        let message = "Roku account services are unavailable in the emulator";
        set_field(vm, &result, "status", Value::Integer(-1))?;
        set_field(vm, &result, "errorCode", Value::string(message))?;
        set_field(vm, &result, "channelID", Value::string("dev"))?;
        set_field(vm, &result, "publisherDeviceID", Value::string(""))?;
        set_field(vm, &result, "json", Value::string(""))?;
        set_field(
            vm,
            &result,
            "response",
            Value::string(
                serde_json::json!({"status":"failure","error":message,"error_detail":message})
                    .to_string(),
            ),
        )?;
        set_field(vm, &node, field, Value::Object(result))?;
    }
    Ok(())
}
