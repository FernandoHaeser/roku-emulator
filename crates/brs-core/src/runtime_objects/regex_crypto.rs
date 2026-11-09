use super::*;
use cipher::{
    block_padding::{NoPadding, Pkcs7},
    BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit, StreamCipher,
};
use fancy_regex::{Regex, RegexBuilder};
use hmac::Mac;
use sha2::Digest;

pub(super) fn regex(pattern: &str, flags: &str) -> Result<Regex, String> {
    let mut options = String::new();
    for flag in ['i', 'm', 's', 'x'] {
        if flags.contains(flag) {
            options.push(flag);
        }
    }
    let pattern = if options.is_empty() {
        pattern.to_string()
    } else {
        format!("(?{options}){pattern}")
    };
    RegexBuilder::new(&pattern)
        .backtrack_limit(1_000_000)
        .build()
        .map_err(|e| e.to_string())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| {
            let a = (v[0] as char).to_digit(16)?;
            let b = (v[1] as char).to_digit(16)?;
            Some((a * 16 + b) as u8)
        })
        .collect()
}
fn digest(algorithm: &str, data: &[u8]) -> Option<Vec<u8>> {
    Some(match algorithm {
        "md5" => md5_digest::Md5::digest(data).to_vec(),
        "sha1" | "sha-1" => sha1::Sha1::digest(data).to_vec(),
        "sha224" | "sha-224" => sha2::Sha224::digest(data).to_vec(),
        "sha256" | "sha-256" => sha2::Sha256::digest(data).to_vec(),
        "sha384" | "sha-384" => sha2::Sha384::digest(data).to_vec(),
        "sha512" | "sha-512" => sha2::Sha512::digest(data).to_vec(),
        _ => return None,
    })
}
fn mac(algorithm: &str, key: &[u8], data: &[u8]) -> Option<Vec<u8>> {
    macro_rules! make {
        ($ty:ty) => {{
            let mut mac = <hmac::Hmac<$ty> as Mac>::new_from_slice(key).ok()?;
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        }};
    }
    Some(match algorithm {
        "md5" => make!(md5_digest::Md5),
        "sha1" | "sha-1" => make!(sha1::Sha1),
        "sha224" | "sha-224" => make!(sha2::Sha224),
        "sha256" | "sha-256" => make!(sha2::Sha256),
        "sha384" | "sha-384" => make!(sha2::Sha384),
        "sha512" | "sha-512" => make!(sha2::Sha512),
        _ => return None,
    })
}
fn crypt(state: &AssocArray, data: &[u8]) -> Option<Vec<u8>> {
    let format = get_str(state, "format");
    let key = unhex(&get_str(state, "key"))?;
    let iv = unhex(&get_str(state, "iv"))?;
    let encrypt = matches!(state.get("encrypt"), Some(Value::Boolean(true)));
    let padding = get_int(state, "padding") != 0;
    let mut out = vec![0; data.len() + 32];
    macro_rules! crypt_aes {
        ($aes:ty) => {{
            if format.ends_with("-ctr") {
                let mut cipher = ctr::Ctr128BE::<$aes>::new_from_slices(&key, &iv).ok()?;
                let mut out = data.to_vec();
                cipher.apply_keystream(&mut out);
                return Some(out);
            }
            if format.ends_with("-cbc") {
                if encrypt {
                    let c = cbc::Encryptor::<$aes>::new_from_slices(&key, &iv).ok()?;
                    if padding {
                        c.encrypt_padded_b2b_mut::<Pkcs7>(data, &mut out)
                            .ok()?
                            .to_vec()
                    } else {
                        c.encrypt_padded_b2b_mut::<NoPadding>(data, &mut out)
                            .ok()?
                            .to_vec()
                    }
                } else {
                    let c = cbc::Decryptor::<$aes>::new_from_slices(&key, &iv).ok()?;
                    if padding {
                        c.decrypt_padded_b2b_mut::<Pkcs7>(data, &mut out)
                            .ok()?
                            .to_vec()
                    } else {
                        c.decrypt_padded_b2b_mut::<NoPadding>(data, &mut out)
                            .ok()?
                            .to_vec()
                    }
                }
            } else if format.ends_with("-ecb") {
                if encrypt {
                    let c = ecb::Encryptor::<$aes>::new_from_slice(&key).ok()?;
                    if padding {
                        c.encrypt_padded_b2b_mut::<Pkcs7>(data, &mut out)
                            .ok()?
                            .to_vec()
                    } else {
                        c.encrypt_padded_b2b_mut::<NoPadding>(data, &mut out)
                            .ok()?
                            .to_vec()
                    }
                } else {
                    let c = ecb::Decryptor::<$aes>::new_from_slice(&key).ok()?;
                    if padding {
                        c.decrypt_padded_b2b_mut::<Pkcs7>(data, &mut out)
                            .ok()?
                            .to_vec()
                    } else {
                        c.decrypt_padded_b2b_mut::<NoPadding>(data, &mut out)
                            .ok()?
                            .to_vec()
                    }
                }
            } else {
                return None;
            }
        }};
    }
    Some(if format.starts_with("aes-128-") {
        crypt_aes!(aes::Aes128)
    } else if format.starts_with("aes-192-") {
        crypt_aes!(aes::Aes192)
    } else if format.starts_with("aes-256-") {
        crypt_aes!(aes::Aes256)
    } else {
        return None;
    })
}
pub(super) fn call(_: &mut Vm, object: &ObjectRef, method: &str, args: &[Value]) -> ResultValue {
    let class = object.borrow().class_name();
    let state = state(object);
    if class == "roRegex" {
        let regex = regex(&get_str(&state, "pattern"), &get_str(&state, "flags"))
            .map_err(|_| RuntimeError::type_mismatch("Invalid regex".into()))?;
        let input = text(args, 0);
        let captures = |c: fancy_regex::Captures<'_>| {
            Value::object(Object::Array(
                (0..c.len())
                    .map(|i| Value::string(c.get(i).map_or("", |m| m.as_str())))
                    .collect(),
            ))
        };
        return Ok(match method {
            "ismatch" => Value::Boolean(regex.is_match(&input).unwrap_or(false)),
            "match" => regex
                .captures(&input)
                .ok()
                .flatten()
                .map(captures)
                .unwrap_or_else(|| Value::object(Object::Array(vec![]))),
            "matchall" => Value::object(Object::Array(
                regex
                    .captures_iter(&input)
                    .filter_map(Result::ok)
                    .take(10000)
                    .map(captures)
                    .collect(),
            )),
            "split" => {
                let mut end = 0;
                let mut parts = Vec::new();
                for matched in regex.find_iter(&input).filter_map(Result::ok).take(10000) {
                    parts.push(Value::string(&input[end..matched.start()]));
                    end = matched.end();
                }
                parts.push(Value::string(&input[end..]));
                Value::object(Object::Array(parts))
            }
            "replace" | "replaceall" => {
                let replacement = text(args, 1);
                let mut converted = String::new();
                let mut chars = replacement.chars().peekable();
                while let Some(ch) = chars.next() {
                    if ch == '\\' && chars.peek().is_some_and(char::is_ascii_digit) {
                        let digit = chars.next().unwrap();
                        converted.push_str(&format!("${{{digit}}}"));
                    } else {
                        if ch == '$' {
                            converted.push('$');
                        }
                        converted.push(ch);
                    }
                }
                Value::string(if method == "replace" {
                    regex.replace(&input, converted.as_str()).into_owned()
                } else {
                    regex.replace_all(&input, converted.as_str()).into_owned()
                })
            }
            _ => return Err(RuntimeError::member_not_found()),
        });
    }
    if method == "setup" {
        if class == "roEVPCipher" {
            store(object, "encrypt", argument(args, 0));
            store(
                object,
                "format",
                Value::string(text(args, 1).to_lowercase()),
            );
            store(object, "key", Value::string(text(args, 2)));
            store(object, "iv", Value::string(text(args, 3)));
            store(object, "padding", argument(args, 4));
            let current = super::state(object);
            let valid =
                crypt(&current, if number(args, 4) == 0 { &[0; 16] } else { &[] }).is_some();
            store(object, "data", byte_array(vec![]));
            return Ok(Value::Integer(if valid { 0 } else { 1 }));
        }
        let algorithm = text(args, 0).to_lowercase();
        let valid = digest(&algorithm, b"").is_some();
        store(object, "algorithm", Value::string(algorithm));
        if class == "roHMAC" {
            store(object, "keybytes", byte_array(bytes(&argument(args, 1))));
        }
        store(object, "data", byte_array(vec![]));
        return Ok(Value::Integer(if valid { 0 } else { 1 }));
    }
    let algorithm = get_str(&state, "algorithm");
    match method {
        "reinit" => {
            store(object, "data", byte_array(vec![]));
            Ok(Value::Integer(0))
        }
        "update" => {
            let mut data = state.get("data").map(bytes).unwrap_or_default();
            let input = bytes(&argument(args, 0));
            if data.len() + input.len() > 16 * 1024 * 1024 {
                return Ok(Value::Invalid);
            }
            data.extend(input);
            store(object, "data", byte_array(data));
            Ok(if class == "roEVPCipher" {
                byte_array(vec![])
            } else {
                Value::Invalid
            })
        }
        "process" | "final" => {
            let data = if method == "process" {
                bytes(&argument(args, 0))
            } else {
                state.get("data").map(bytes).unwrap_or_default()
            };
            if class == "roEVPCipher" {
                Ok(crypt(&state, &data)
                    .map(byte_array)
                    .unwrap_or(Value::Invalid))
            } else if class == "roHMAC" {
                Ok(mac(
                    &algorithm,
                    &state.get("keybytes").map(bytes).unwrap_or_default(),
                    &data,
                )
                .map(byte_array)
                .unwrap_or(Value::Invalid))
            } else {
                Ok(digest(&algorithm, &data)
                    .map(|v| Value::string(hex(&v)))
                    .unwrap_or(Value::Invalid))
            }
        }
        _ => Err(RuntimeError::member_not_found()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn digest_and_mac_vectors() {
        assert_eq!(
            hex(&digest("sha256", b"abc").unwrap()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&mac(
                "sha256",
                b"key",
                b"The quick brown fox jumps over the lazy dog"
            )
            .unwrap()),
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
    }
    #[test]
    fn aes_nist_vector() {
        let mut s = AssocArray::new();
        s.set("format", Value::string("aes-128-cbc"));
        s.set("key", Value::string("2b7e151628aed2a6abf7158809cf4f3c"));
        s.set("iv", Value::string("000102030405060708090a0b0c0d0e0f"));
        s.set("encrypt", Value::Boolean(true));
        assert_eq!(
            hex(&crypt(&s, &unhex("6bc1bee22e409f96e93d7e117393172a").unwrap()).unwrap()),
            "7649abac8119b246cee98e9b12e9197d"
        );
    }
}
