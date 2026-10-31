use acorde_render_svg::{SvgRenderOptions, render_svg};

fn options(measures_per_system: usize) -> SvgRenderOptions {
    SvgRenderOptions {
        width: 1400.0,
        staff_size: 24.0,
        measures_per_system,
        interactive: true,
    }
}

fn hit_width(svg: &str, measure: usize) -> f32 {
    let marker = format!(
        "data-acorde-kind=\"measure-hit-region\" data-part=\"0\" data-staff=\"0\" data-measure=\"{measure}\""
    );
    let suffix = svg.split_once(&marker).expect("measure hit region").1;
    suffix
        .split_once("width=\"")
        .expect("hit width")
        .1
        .split_once('"')
        .expect("width value")
        .0
        .parse()
        .expect("numeric width")
}

#[test]
fn abc_inline_meter_changes_render_and_reserve_width() {
    let score = acorde_io::parse_abc(
        "X:1\nT:Inline meter change\nM:3/4\nL:1/4\nK:C\nC3 | [M:4/4] D4 | [M:3/4] E3 | [M:4/4] F4 |]",
    )
    .expect("ABC parses");
    let svg = render_svg(&score, &options(4)).expect("SVG renders");
    assert_eq!(svg.matches("class=\"acorde-time-sig\"").count(), 4);
    for measure in 1..4 {
        assert!(
            svg.contains(&format!("class=\"acorde-time-sig-change\" data-part=\"0\" data-staff=\"0\" data-measure=\"{measure}\"")),
            "missing meter change in measure {measure}"
        );
    }
    assert!(hit_width(&svg, 1) > hit_width(&svg, 0) + 1.0);
    assert!(hit_width(&svg, 2) > hit_width(&svg, 0) + 1.0);
    assert!(hit_width(&svg, 3) > hit_width(&svg, 0) + 1.0);
}

#[test]
fn inline_and_system_start_meters_work_on_multiple_staves() {
    let mut score = acorde_io::parse_abc(
        "X:1\nM:3/4\nL:1/4\nK:C\nC3 | [M:4/4] D4 | [M:3/4] E3 | [M:4/4] F4 | G4 | A4 |]",
    )
    .expect("ABC parses");
    let second_staff = score.parts[0].staves[0].clone();
    score.parts[0].staves.push(second_staff);
    let svg = render_svg(&score, &options(2)).expect("SVG renders");
    assert_eq!(svg.matches("class=\"acorde-time-sig\"").count(), 8);
    for measure in [1, 3] {
        for staff in [0, 1] {
            assert!(svg.contains(&format!(
                "class=\"acorde-time-sig-change\" data-part=\"0\" data-staff=\"{staff}\" data-measure=\"{measure}\""
            )));
        }
    }
    assert_eq!(svg.matches("class=\"acorde-time-sig-change\"").count(), 4);
}
