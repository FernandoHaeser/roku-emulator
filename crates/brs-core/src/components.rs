//! Non-visual components (F2): roList, roByteArray, roString & boxed values,
//! roMessagePort, roDateTime, roAppInfo, roDeviceInfo, roRegistry(Section),
//! roXMLElement/roXMLList. Behaviors follow fidelity/runtime goldens.

use std::rc::Rc;

use crate::error::RuntimeError;
use crate::value::{AssocArray, Object, ObjectRef, Value};
use crate::vm::Vm;
use crate::xml::{XmlElement, XmlNode};

fn arg(args: &[Value], index: usize) -> &Value {
    args.get(index).unwrap_or(&Value::Invalid)
}

fn arg_str(args: &[Value], index: usize) -> String {
    arg(args, index).as_str().unwrap_or_default().to_string()
}

fn arg_int(args: &[Value], index: usize) -> i64 {
    arg(args, index).as_i64().unwrap_or(0)
}

fn list_value(items: Vec<Value>) -> Value {
    Value::object(Object::List(items))
}

fn strings_list(items: impl IntoIterator<Item = String>) -> Value {
    list_value(items.into_iter().map(Value::string).collect())
}

/// Device warning for deprecated APIs (fidelity/runtime/appinfo).
fn deprecated(vm: &mut Vm, api: &str) {
    let location = vm.current_location();
    vm.write(&format!(
        "BRIGHTSCRIPT: WARNING: {api}: This function is deprecated: {location}\n"
    ));
}

/// Dispatches a method on a component object. `None` means "not handled
/// here" (the caller falls back to other interfaces).
pub fn call(
    vm: &mut Vm,
    object: &ObjectRef,
    name: &str,
    args: &[Value],
) -> Option<Result<Value, RuntimeError>> {
    let class = object.borrow().class_name();
    Some(match class {
        "roList" => list_method(object, name, args)?,
        "roByteArray" => byte_array_method(object, name, args),
        "roMessagePort" => {
            let Object::MessagePort(port) = &*object.borrow() else {
                unreachable!()
            };
            let port = *port;
            match name {
                "getmessage" => Ok(vm.take_message(port).unwrap_or(Value::Invalid)),
                "peekmessage" => Ok(vm.peek_message(port).unwrap_or(Value::Invalid)),
                "waitmessage" => {
                    let timeout = arg(args, 0).as_f64().unwrap_or(0.0);
                    Ok(vm.wait_for_message(port, timeout))
                }
                _ => Err(RuntimeError::member_not_found()),
            }
        }
        "roDateTime" => date_time_method(vm, object, name, args),
        "roAppInfo" => app_info_method(vm, name, args),
        "roDeviceInfo" => device_info_method(vm, name, args),
        "roRegistry" => registry_method(vm, name, args),
        "roRegistrySection" => {
            let Object::RegistrySection(section) = &*object.borrow() else {
                unreachable!()
            };
            let section = section.clone();
            registry_section_method(vm, &section, name, args)
        }
        "roXMLElement" => xml_element_method(object, name, args),
        "roXMLList" => xml_list_method(object, name, args),
        "roSGScreenEvent" => match name {
            "isscreenclosed" => {
                let Object::ScreenEvent { closed } = &*object.borrow() else {
                    unreachable!()
                };
                Ok(Value::Boolean(*closed))
            }
            _ => Err(RuntimeError::member_not_found()),
        },
        _ if class.starts_with("ro") && matches!(&*object.borrow(), Object::Boxed(_)) => {
            boxed_method(object, name, args)?
        }
        _ => return None,
    })
}

// ---- roList ------------------------------------------------------------------

fn list_method(
    object: &ObjectRef,
    name: &str,
    args: &[Value],
) -> Option<Result<Value, RuntimeError>> {
    let mut borrowed = object.borrow_mut();
    let Object::List(items) = &mut *borrowed else {
        unreachable!()
    };
    let value = arg(args, 0).clone();
    Some(Ok(match name {
        "addtail" | "push" => {
            items.push(value);
            Value::Invalid
        }
        "addhead" | "unshift" => {
            items.insert(0, value);
            Value::Invalid
        }
        "removehead" | "shift" => {
            if items.is_empty() {
                Value::Invalid
            } else {
                items.remove(0)
            }
        }
        "removetail" | "pop" => items.pop().unwrap_or(Value::Invalid),
        "gethead" => items.first().cloned().unwrap_or(Value::Invalid),
        "gettail" | "peek" => items.last().cloned().unwrap_or(Value::Invalid),
        "count" => Value::Integer(items.len() as i32),
        "isempty" => Value::Boolean(items.is_empty()),
        "clear" => {
            items.clear();
            Value::Invalid
        }
        "getentry" => usize::try_from(arg_int(args, 0))
            .ok()
            .and_then(|i| items.get(i).cloned())
            .unwrap_or(Value::Invalid),
        "removeindex" | "delete" => {
            let index = arg_int(args, 0);
            Value::Boolean(if index >= 0 && (index as usize) < items.len() {
                items.remove(index as usize);
                true
            } else {
                false
            })
        }
        _ => return None,
    }))
}

// ---- roByteArray ------------------------------------------------------------------

fn byte_array_method(
    object: &ObjectRef,
    name: &str,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let mut borrowed = object.borrow_mut();
    let Object::ByteArray(bytes) = &mut *borrowed else {
        unreachable!()
    };
    Ok(match name {
        "fromasciistring" => {
            *bytes = arg_str(args, 0).into_bytes();
            Value::Invalid
        }
        "toasciistring" => Value::string(String::from_utf8_lossy(bytes).into_owned()),
        "fromhexstring" => {
            let text = arg_str(args, 0);
            *bytes = (0..text.len() / 2)
                .filter_map(|i| u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok())
                .collect();
            Value::Invalid
        }
        "tohexstring" => {
            Value::string(bytes.iter().map(|b| format!("{b:02X}")).collect::<String>())
        }
        "frombase64string" => {
            *bytes = base64_decode(&arg_str(args, 0));
            Value::Invalid
        }
        "tobase64string" => Value::string(base64_encode(bytes)),
        "count" => Value::Integer(bytes.len() as i32),
        "isempty" => Value::Boolean(bytes.is_empty()),
        "clear" => {
            bytes.clear();
            Value::Invalid
        }
        "push" => {
            bytes.push(arg_int(args, 0) as u8);
            Value::Invalid
        }
        "pop" => bytes
            .pop()
            .map_or(Value::Invalid, |b| Value::Integer(i32::from(b))),
        "getsignedbyte" => usize::try_from(arg_int(args, 0))
            .ok()
            .and_then(|i| bytes.get(i))
            .map_or(Value::Integer(0), |b| Value::Integer(i32::from(*b as i8))),
        "getsignedlong" => {
            let i = arg_int(args, 0).max(0) as usize * 4;
            let word = bytes
                .get(i..i + 4)
                .map_or(0, |w| i32::from_le_bytes([w[0], w[1], w[2], w[3]]));
            Value::Integer(word)
        }
        "getcrc32" => Value::Integer(crc32(bytes) as i32),
        "setresize" => {
            bytes.resize(arg_int(args, 0).max(0) as usize, 0);
            Value::Invalid
        }
        "append" => {
            if let Value::Object(other) = arg(args, 0) {
                if let Object::ByteArray(extra) = &*other.borrow() {
                    bytes.extend_from_slice(extra);
                }
            }
            Value::Invalid
        }
        _ => return Err(RuntimeError::member_not_found()),
    })
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(BASE64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn base64_decode(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for c in text.bytes() {
        let Some(value) = BASE64.iter().position(|b| *b == c) else {
            continue;
        };
        buffer = buffer << 6 | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                crc >> 1 ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

// ---- boxed values --------------------------------------------------------------

fn boxed_method(
    object: &ObjectRef,
    name: &str,
    args: &[Value],
) -> Option<Result<Value, RuntimeError>> {
    let mut borrowed = object.borrow_mut();
    let Object::Boxed(inner) = &mut *borrowed else {
        unreachable!()
    };
    Some(Ok(match name {
        "setstring" => {
            *inner = Value::string(arg_str(args, 0));
            Value::Invalid
        }
        "appendstring" => {
            let text = format!("{}{}", inner.as_str().unwrap_or_default(), arg_str(args, 0));
            *inner = Value::string(text);
            Value::Invalid
        }
        "getstring" => Value::string(inner.as_str().unwrap_or_default()),
        "setint" | "setfloat" | "setdouble" | "setboolean" | "setlonginteger" => {
            *inner = arg(args, 0).clone();
            Value::Invalid
        }
        "getint" | "getfloat" | "getdouble" | "getboolean" | "getlonginteger" => inner.clone(),
        _ => return None,
    }))
}

// ---- roDateTime ----------------------------------------------------------------------

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (
        if month <= 2 {
            yoe + era * 400 + 1
        } else {
            yoe + era * 400
        },
        month,
        day,
    )
}

struct Civil {
    year: i64,
    month: i64,
    day: i64,
    hours: i64,
    minutes: i64,
    seconds: i64,
    millis: i64,
    weekday: i64,
}

fn civil(epoch_ms: i64) -> Civil {
    let days = epoch_ms.div_euclid(86_400_000);
    let rest = epoch_ms.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    Civil {
        year,
        month,
        day,
        hours: rest / 3_600_000,
        minutes: rest / 60_000 % 60,
        seconds: rest / 1000 % 60,
        millis: rest % 1000,
        weekday: (days + 4).rem_euclid(7),
    }
}

fn last_day_of_month(year: i64, month: i64) -> i64 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    days_from_civil(next_year, next_month, 1) - days_from_civil(year, month, 1)
}

/// "2024-02-29T23:59:58Z", "2024-01-02 03:04:05", with optional fraction/offset.
fn parse_iso8601(text: &str) -> Option<i64> {
    let text = text.trim();
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hours, minutes, seconds) = if text.len() >= 19 {
        (number(11..13)?, number(14..16)?, number(17..19)?)
    } else {
        (0, 0, 0)
    };
    let mut millis = 0;
    if let Some(fraction) = text.get(19..).and_then(|r| r.strip_prefix('.')) {
        let digits: String = fraction
            .chars()
            .take_while(char::is_ascii_digit)
            .take(3)
            .collect();
        millis = format!("{digits:0<3}").parse().unwrap_or(0);
    }
    let days = days_from_civil(year, month, day);
    Some(((days * 24 + hours) * 60 + minutes) * 60_000 + seconds * 1000 + millis)
}

struct LocaleNames {
    weekdays: [&'static str; 7],
    weekdays_short: [&'static str; 7],
    months: [&'static str; 12],
    months_short: [&'static str; 12],
}

/// pt_BR names from fidelity/runtime/datetime ("Terça-feira novembro 14,
/// 2023", "Ter. nov. 14, 2023"); entries not in a golden are provisional.
const PT_BR: LocaleNames = LocaleNames {
    weekdays: [
        "Domingo",
        "Segunda-feira",
        "Terça-feira",
        "Quarta-feira",
        "Quinta-feira",
        "Sexta-feira",
        "Sábado",
    ],
    weekdays_short: ["Dom.", "Seg.", "Ter.", "Qua.", "Qui.", "Sex.", "Sáb."],
    months: [
        "janeiro",
        "fevereiro",
        "março",
        "abril",
        "maio",
        "junho",
        "julho",
        "agosto",
        "setembro",
        "outubro",
        "novembro",
        "dezembro",
    ],
    months_short: [
        "jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.", "nov.",
        "dez.",
    ],
};

const EN_US: LocaleNames = LocaleNames {
    weekdays: [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ],
    weekdays_short: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
    months: [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ],
    months_short: [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ],
};

fn date_string(vm: &Vm, c: &Civil, format: &str) -> String {
    let names = if vm.env.profile.locale.starts_with("pt") {
        &PT_BR
    } else {
        &EN_US
    };
    let weekday = names.weekdays[c.weekday as usize];
    let weekday_short = names.weekdays_short[c.weekday as usize];
    let month = names.months[(c.month - 1) as usize];
    let month_short = names.months_short[(c.month - 1) as usize];
    let (d, y) = (c.day, c.year);
    match format {
        "short-weekday" => format!("{weekday_short} {month} {d}, {y}"),
        "no-weekday" => format!("{month} {d}, {y}"),
        "short-month" => format!("{weekday} {month_short} {d}, {y}"),
        "short-month-short-weekday" => format!("{weekday_short} {month_short} {d}, {y}"),
        "short-month-no-weekday" => format!("{month_short} {d}, {y}"),
        "short-date" => format!("{}/{}/{:02}", c.month, d, y % 100),
        "short-date-dashes" => format!("{}-{}-{:02}", c.month, d, y % 100),
        _ => format!("{weekday} {month} {d}, {y}"),
    }
}

fn date_time_method(
    vm: &mut Vm,
    object: &ObjectRef,
    name: &str,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let epoch_ms = match &*object.borrow() {
        Object::DateTime { epoch_ms } => *epoch_ms,
        _ => unreachable!(),
    };
    let set = |value: i64| {
        if let Object::DateTime { epoch_ms } = &mut *object.borrow_mut() {
            *epoch_ms = value;
        }
    };
    let c = civil(epoch_ms);
    Ok(match name {
        "mark" => {
            set(vm.epoch_ms() as i64);
            Value::Invalid
        }
        "fromseconds" | "fromsecondslong" => {
            set(arg_int(args, 0) * 1000);
            Value::Invalid
        }
        "fromiso8601string" => {
            if let Some(value) = parse_iso8601(&arg_str(args, 0)) {
                set(value);
            }
            Value::Invalid
        }
        "tolocaltime" => {
            set(epoch_ms + i64::from(vm.env.profile.utc_offset_minutes) * 60_000);
            Value::Invalid
        }
        "asseconds" => Value::Integer(epoch_ms.div_euclid(1000) as i32),
        "assecondslong" => Value::LongInteger(epoch_ms.div_euclid(1000)),
        "toisostring" => {
            let base = format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                c.year, c.month, c.day, c.hours, c.minutes, c.seconds
            );
            if arg_str(args, 0) == "milliseconds" {
                Value::string(format!("{base}.{:03}Z", c.millis))
            } else {
                Value::string(format!("{base}Z"))
            }
        }
        "getyear" => Value::Integer(c.year as i32),
        "getmonth" => Value::Integer(c.month as i32),
        "getdayofmonth" => Value::Integer(c.day as i32),
        "gethours" => Value::Integer(c.hours as i32),
        "getminutes" => Value::Integer(c.minutes as i32),
        "getseconds" => Value::Integer(c.seconds as i32),
        "getmilliseconds" => Value::Integer(c.millis as i32),
        "getdayofweek" => Value::Integer(c.weekday as i32),
        "getweekday" => Value::string(EN_US.weekdays[c.weekday as usize]),
        "getlastdayofmonth" => Value::Integer(last_day_of_month(c.year, c.month) as i32),
        "gettimezoneoffset" => Value::Integer(-vm.env.profile.utc_offset_minutes),
        "asdatestring" => Value::string(date_string(vm, &c, &arg_str(args, 0))),
        "asdatestringnoparam" => Value::string(date_string(vm, &c, "long-date")),
        "astimestringloc" => Value::string(format!("{:02}:{:02}", c.hours, c.minutes)),
        _ => return Err(RuntimeError::member_not_found()),
    })
}

// ---- roAppInfo / roDeviceInfo ---------------------------------------------------------

fn app_info_method(vm: &mut Vm, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(match name {
        "getid" => Value::string("dev"),
        "isdev" => Value::Boolean(true),
        "gettitle" => Value::string(vm.env.manifest_value("title")),
        "getversion" => Value::string(vm.channel_info().version.clone()),
        "getvalue" => Value::string(vm.env.manifest_value(&arg_str(args, 0))),
        "getsubtitle" => {
            deprecated(vm, "roAppInfo.GetSubtitle");
            Value::string(vm.env.manifest_value("subtitle"))
        }
        "getdevid" => Value::string("emulator"),
        _ => return Err(RuntimeError::member_not_found()),
    })
}

fn aa(entries: &[(&str, Value)]) -> Value {
    let mut map = AssocArray::new();
    for (key, value) in entries {
        map.set(key, value.clone());
    }
    Value::object(Object::AssocArray(map))
}

fn device_info_method(vm: &mut Vm, name: &str, _args: &[Value]) -> Result<Value, RuntimeError> {
    let p = vm.env.profile.clone();
    Ok(match name {
        "getmodel" => Value::string(p.model),
        "getmodeldisplayname" => Value::string(p.model_display_name),
        "getmodeltype" => Value::string(p.model_type),
        "getmodeldetails" => aa(&[
            ("ModelNumber", Value::string(p.model)),
            ("VendorName", Value::string("Roku")),
        ]),
        "getfriendlyname" => Value::string("Roku Emulator"),
        "getdisplaysize" => aa(&[
            ("h", Value::Integer(p.ui_height as i32)),
            ("w", Value::Integer(p.ui_width as i32)),
        ]),
        "getuiresolution" => aa(&[
            ("height", Value::Integer(p.ui_height as i32)),
            ("name", Value::string(p.ui_name)),
            ("width", Value::Integer(p.ui_width as i32)),
        ]),
        "getdisplaymode" => Value::string(p.display_mode),
        "getdisplaytype" => Value::string(p.display_type),
        "getdisplayaspectratio" => Value::string(p.aspect_ratio),
        "getvideomode" => Value::string(p.video_mode),
        "getcurrentlocale" => Value::string(p.locale),
        "getcountrycode" | "getuserCountryCode" | "getusercountrycode" => Value::string(p.country),
        "getpreferredcaptionlanguage" => Value::string("por"),
        "gettimezone" => Value::string(p.time_zone),
        "getosversion" => aa(&[
            ("build", Value::string(p.os_build)),
            ("major", Value::string(p.os_major)),
            ("minor", Value::string(p.os_minor)),
            ("revision", Value::string(p.os_revision)),
        ]),
        "getversion" => Value::string(format!("999.{}E0{}A", p.os_major, p.os_build)),
        "candecodevideo" | "candecodeaudio" => aa(&[("result", Value::Boolean(true))]),
        "isridadisabled" => Value::Boolean(false),
        "getconnectiontype" => Value::string("WiFiConnection"),
        "getlinkstatus" | "getinternetstatus" => Value::Boolean(true),
        "getconnectioninfo" => aa(&[
            ("type", Value::string("WiFiConnection")),
            ("name", Value::string("emulator")),
        ]),
        "getchannelclientid"
        | "getrida"
        | "getrandomuuid"
        | "getclientTrackingId"
        | "getclienttrackingid" => Value::string("00000000-0000-4000-8000-000000000000"),
        "getdrminfo" => {
            deprecated(vm, "roDeviceInfo.GetDrmInfo");
            aa(&[("PlayReady", Value::string("tee"))])
        }
        "getdrminfoex" => aa(&[
            ("PlayReady", Value::string("tee")),
            ("Widevine", Value::string("L1")),
        ]),
        "getgraphicsplatform" => Value::string("opengl"),
        "hasfeature" => Value::Boolean(false),
        "getgeneralmemorylevel" => Value::string("normal"),
        "getaudiooutputchannel" => Value::string("Stereo"),
        "enableappfocusevent"
        | "enablescreensaverexitedevent"
        | "enablelowgeneralmemoryevent"
        | "enablelinkstatusevent"
        | "setmessageport" => Value::Boolean(true),
        _ => return Err(RuntimeError::member_not_found()),
    })
}

// ---- registry ------------------------------------------------------------------------

/// Registry size cap per channel (16 KB on the device).
const REGISTRY_LIMIT: usize = 16 * 1024;

fn registry_method(vm: &mut Vm, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(match name {
        "getsectionlist" => strings_list(vm.env.registry.keys().cloned().collect::<Vec<_>>()),
        "delete" => {
            vm.env.registry.remove(&arg_str(args, 0));
            vm.env.registry_dirty = true;
            Value::Boolean(true)
        }
        "flush" => Value::Boolean(true),
        "getspaceavailable" => {
            let used: usize = vm
                .env
                .registry
                .iter()
                .map(|(s, e)| s.len() + e.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>())
                .sum();
            Value::Integer(REGISTRY_LIMIT.saturating_sub(used) as i32)
        }
        _ => return Err(RuntimeError::member_not_found()),
    })
}

fn registry_section_method(
    vm: &mut Vm,
    section: &str,
    name: &str,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    let entries = vm.env.registry.get(section);
    Ok(match name {
        "read" => Value::string(
            entries
                .and_then(|e| e.get(&arg_str(args, 0)))
                .cloned()
                .unwrap_or_default(),
        ),
        "readmulti" => {
            let mut result = AssocArray::new();
            if let Value::Object(keys) = arg(args, 0) {
                if let Object::Array(keys) = &*keys.borrow() {
                    for key in keys {
                        let key = key.unboxed().as_str().unwrap_or_default().to_string();
                        if let Some(value) = entries.and_then(|e| e.get(&key)) {
                            result.set(&key, Value::string(value.as_str()));
                        }
                    }
                }
            }
            Value::object(Object::AssocArray(result))
        }
        "exists" => Value::Boolean(entries.is_some_and(|e| e.contains_key(&arg_str(args, 0)))),
        "getkeylist" => strings_list(
            entries
                .map(|e| e.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default(),
        ),
        "write" => {
            let (key, value) = (arg_str(args, 0), arg_str(args, 1));
            vm.env
                .registry
                .entry(section.to_string())
                .or_default()
                .insert(key, value);
            vm.env.registry_dirty = true;
            Value::Boolean(true)
        }
        "writemulti" => {
            if let Value::Object(map) = arg(args, 0) {
                if let Object::AssocArray(map) = &*map.borrow() {
                    let target = vm.env.registry.entry(section.to_string()).or_default();
                    for (key, value) in map.sorted_entries() {
                        target.insert(key, crate::value::to_display_string(&value));
                    }
                }
            }
            vm.env.registry_dirty = true;
            Value::Boolean(true)
        }
        "delete" => {
            if let Some(entries) = vm.env.registry.get_mut(section) {
                entries.remove(&arg_str(args, 0));
            }
            vm.env.registry_dirty = true;
            Value::Boolean(true)
        }
        "flush" => Value::Boolean(true),
        _ => return Err(RuntimeError::member_not_found()),
    })
}

// ---- XML -------------------------------------------------------------------------------

fn element_of(object: &ObjectRef) -> Rc<XmlElement> {
    match &*object.borrow() {
        Object::XmlElement(element) => element.clone(),
        _ => unreachable!(),
    }
}

fn element_value(element: &Rc<XmlElement>) -> Value {
    Value::object(Object::XmlElement(element.clone()))
}

fn xml_list(elements: Vec<Rc<XmlElement>>) -> Value {
    Value::object(Object::XmlList(
        elements.iter().map(element_value).collect(),
    ))
}

/// `element.name` (dot access): child elements with that name.
pub fn named_children(element: &XmlElement, name: &str) -> Value {
    xml_list(
        element
            .elements()
            .filter(|e| e.name.eq_ignore_ascii_case(name))
            .cloned()
            .collect(),
    )
}

fn attributes_value(element: &XmlElement) -> Value {
    let mut map = AssocArray::new();
    for (key, value) in &element.attributes {
        map.set(key, Value::string(value.as_str()));
    }
    Value::object(Object::AssocArray(map))
}

fn xml_element_method(
    object: &ObjectRef,
    name: &str,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    Ok(match name {
        "parse" => match crate::xml::parse(&arg_str(args, 0)) {
            Ok(root) => {
                *object.borrow_mut() = Object::XmlElement(root);
                Value::Boolean(true)
            }
            Err(_) => Value::Boolean(false),
        },
        "getname" => Value::string(element_of(object).name.as_str()),
        "getattributes" => attributes_value(&element_of(object)),
        "hasattribute" => Value::Boolean(element_of(object).attribute(&arg_str(args, 0)).is_some()),
        "$attr" => Value::string(
            element_of(object)
                .attribute(&arg_str(args, 0))
                .unwrap_or_default(),
        ),
        "gettext" => Value::string(element_of(object).text()),
        "getbody" => {
            let element = element_of(object);
            if element.elements().next().is_some() {
                xml_list(element.elements().cloned().collect())
            } else {
                Value::string(element.text())
            }
        }
        "getchildelements" => {
            let element = element_of(object);
            if element.elements().next().is_none() {
                Value::Invalid
            } else {
                xml_list(element.elements().cloned().collect())
            }
        }
        "getchildnodes" => {
            let element = element_of(object);
            Value::object(Object::List(
                element
                    .children
                    .iter()
                    .map(|child| match child {
                        XmlNode::Element(e) => element_value(e),
                        XmlNode::Text(text) => Value::string(text.as_str()),
                    })
                    .collect(),
            ))
        }
        "getnamedelements" | "getnamedelementsci" => {
            named_children(&element_of(object), &arg_str(args, 0))
        }
        "isname" => Value::Boolean(element_of(object).name == arg_str(args, 0)),
        _ => return Err(RuntimeError::member_not_found()),
    })
}

fn xml_list_method(object: &ObjectRef, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
    let items = match &*object.borrow() {
        Object::XmlList(items) => items.clone(),
        _ => unreachable!(),
    };
    let elements: Vec<Rc<XmlElement>> = items
        .iter()
        .filter_map(|v| match v {
            Value::Object(o) => match &*o.borrow() {
                Object::XmlElement(e) => Some(e.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    Ok(match name {
        "count" => Value::Integer(items.len() as i32),
        "isempty" => Value::Boolean(items.is_empty()),
        "getentry" => usize::try_from(arg_int(args, 0))
            .ok()
            .and_then(|i| items.get(i).cloned())
            .unwrap_or(Value::Invalid),
        "gethead" => items.first().cloned().unwrap_or(Value::Invalid),
        "gettail" => items.last().cloned().unwrap_or(Value::Invalid),
        "gettext" => Value::string(elements.iter().map(|e| e.text()).collect::<String>()),
        "getattributes" => elements
            .first()
            .map_or(Value::Invalid, |e| attributes_value(e)),
        "$attr" => Value::string(
            elements
                .first()
                .and_then(|e| e.attribute(&arg_str(args, 0)).map(str::to_string))
                .unwrap_or_default(),
        ),
        "getchildelements" => xml_list(
            elements
                .iter()
                .flat_map(|e| e.elements().cloned().collect::<Vec<_>>())
                .collect(),
        ),
        "getnamedelements" | "getnamedelementsci" => {
            let target = arg_str(args, 0);
            xml_list(
                elements
                    .iter()
                    .flat_map(|e| {
                        e.elements()
                            .filter(|c| c.name.eq_ignore_ascii_case(&target))
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .collect(),
            )
        }
        "simplify" => {
            if items.len() == 1 {
                items[0].clone()
            } else {
                Value::Object(object.clone())
            }
        }
        _ => return Err(RuntimeError::member_not_found()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_and_crc_match_device() {
        assert_eq!(base64_encode(b"Hello"), "SGVsbG8=");
        assert_eq!(base64_decode("Um9rdQ=="), b"Roku");
        // fidelity/runtime/bytearray: "Roku!" -> -1122552369
        assert_eq!(crc32(b"Roku!") as i32, -1_122_552_369);
    }

    #[test]
    fn civil_dates() {
        let c = civil(1_700_000_000_000);
        assert_eq!(
            (c.year, c.month, c.day, c.hours, c.minutes, c.seconds),
            (2023, 11, 14, 22, 13, 20)
        );
        assert_eq!(c.weekday, 2);
        assert_eq!(
            parse_iso8601("2024-02-29T23:59:58Z"),
            Some(1_709_251_198_000)
        );
        assert_eq!(
            parse_iso8601("2024-01-02 03:04:05"),
            Some(1_704_164_645_000)
        );
        assert_eq!(last_day_of_month(2023, 11), 30);
    }
}
