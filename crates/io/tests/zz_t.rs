#[test]
fn t() {
    let paths = std::process::Command::new("find")
        .args([
            "/tmp/claude-0/alphatab/packages/alphatab/test-data",
            "-name",
            "*.gp*",
        ])
        .output()
        .unwrap();
    let mut shown = 0;
    for path in String::from_utf8(paths.stdout).unwrap().lines() {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let Ok(score) = acorde_io::parse_gp(&bytes) else {
            continue;
        };
        let Some(back) = acorde_io::serialize_musicxml(&score)
            .ok()
            .and_then(|x| acorde_io::parse_musicxml(&x).ok())
        else {
            continue;
        };
        let f = |s: &acorde_core::Score| {
            s.parts
                .iter()
                .enumerate()
                .flat_map(|(pi, p)| {
                    p.staves.iter().enumerate().flat_map(move |(si, st)| {
                        st.measures.iter().enumerate().flat_map(move |(mi, m)| {
                            m.texts
                                .iter()
                                .map(move |t| format!("p{pi}s{si}m{mi} {:?} {}", t.style, t.text))
                        })
                    })
                })
                .collect::<Vec<_>>()
        };
        let (a, b) = (f(&score), f(&back));
        if a.len() != b.len() && shown < 3 {
            shown += 1;
            println!(
                "{path}\n A {:?}\n B {:?}",
                &a[..a.len().min(5)],
                &b[..b.len().min(8)]
            );
        }
    }
}
