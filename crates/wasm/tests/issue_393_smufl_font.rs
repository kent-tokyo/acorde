// Error results carry a JsValue, which only exists on wasm32; the success path is covered
// natively in `native_format_bindings.rs`.
#![cfg(all(
    target_arch = "wasm32",
    not(all(
        feature = "playback-only",
        not(any(feature = "full", feature = "viewer"))
    ))
))]

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test::wasm_bindgen_test]
fn smufl_font_option_draws_text_glyphs_and_rejects_an_empty_name() {
    let score = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC4|").unwrap();
    let svg = acorde_wasm::render_score_svg(&score, r#"{"smufl_font":"Bravura"}"#).unwrap();
    assert!(svg.contains(r#"font-family="Bravura""#));
    assert!(acorde_wasm::render_score_svg(&score, r#"{"smufl_font":""}"#).is_err());
}
