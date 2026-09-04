use quick_xml::events::BytesStart;

/// Read a UTF-8 string-valued attribute by name.
pub(super) fn attr_str(e: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| String::from_utf8(a.value.to_vec()).ok())
}

/// True if attribute `key` exists with the literal value `"yes"`.
pub(super) fn attr_is_yes(e: &BytesStart<'_>, key: &[u8]) -> bool {
    e.attributes()
        .filter_map(|a| a.ok())
        .any(|a| a.key.as_ref() == key && a.value.as_ref() == b"yes")
}

/// True if attribute `key` is present (value ignored).
pub(super) fn attr_present(e: &BytesStart<'_>, key: &[u8]) -> bool {
    e.attributes()
        .filter_map(|a| a.ok())
        .any(|a| a.key.as_ref() == key)
}
