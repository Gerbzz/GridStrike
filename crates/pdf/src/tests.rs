use super::*;

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}
fn rfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).rposition(|w| w == needle)
}

/// Parses the xref table back and checks each in-use entry points at `N 0 obj`.
fn check_xref(pdf: &[u8]) -> usize {
    let sx = rfind(pdf, b"startxref\n").unwrap();
    let rest = std::str::from_utf8(&pdf[sx + 10..]).unwrap();
    let off: usize = rest.lines().next().unwrap().trim().parse().unwrap();
    assert!(pdf[off..].starts_with(b"xref\n"), "startxref points at xref");
    let table = std::str::from_utf8(&pdf[off..sx]).unwrap();
    let mut lines = table.lines();
    assert_eq!(lines.next(), Some("xref"));
    let hdr: Vec<usize> = lines.next().unwrap().split(' ').map(|x| x.parse().unwrap()).collect();
    assert_eq!(hdr[0], 0);
    let count = hdr[1];
    let mut checked = 0;
    for i in 0..count {
        let e = lines.next().unwrap();
        assert_eq!(e.len(), 19, "entry `{e}` (20 bytes with newline)");
        let o: usize = e[..10].parse().unwrap();
        if e.trim_end().ends_with('n') {
            let want = format!("{i} 0 obj");
            assert!(pdf[o..].starts_with(want.as_bytes()), "object {i} at {o}");
            checked += 1;
        }
    }
    assert!(table.contains("trailer"));
    assert!(table.contains(&format!("/Size {count}")));
    checked
}

#[test]
fn minimal_document() {
    let pdf = PdfDoc::new().finish();
    assert!(pdf.starts_with(b"%PDF-1.7"));
    assert!(pdf.ends_with(b"%%EOF\n"));
    assert!(check_xref(&pdf) >= 5);
}

#[test]
fn drawing_and_text() {
    let mut d = PdfDoc::new();
    d.info.title = "Report (Q1)".into();
    let p = d.add_page(612.0, 792.0);
    p.fill_rect(10.0, 10.0, 100.0, 20.0, [255, 255, 0]);
    p.rect_stroke(10.0, 10.0, 100.0, 20.0, 0.5, [0, 0, 0]);
    p.stroke_line(0.0, 0.0, 612.0, 792.0, 1.0, [255, 0, 0]);
    p.stroke_line_dashed(0.0, 5.0, 100.0, 5.0, 1.0, [0, 0, 0], &[3.0, 2.0]);
    p.polygon(&[[0.0, 0.0], [10.0, 0.0], [5.0, 8.0]], Some([0, 0, 255]), Some(([0, 0, 0], 1.0)));
    p.circle(300.0, 300.0, 20.0, Some([0, 128, 0]), None);
    p.save();
    p.clip_rect(0.0, 0.0, 50.0, 50.0);
    p.text(72.0, 72.0, 12.0, Font::Helvetica, [0, 0, 0], "Hello (world) \\ café €5");
    p.restore();
    p.text_rotated(100.0, 100.0, 9.0, Font::TimesBold, [0, 0, 0], "Up", -std::f32::consts::FRAC_PI_2);
    let p2 = d.add_page(842.0, 595.0);
    p2.text(10.0, 20.0, 10.0, Font::CourierBold, [0, 0, 0], "Second");
    assert_eq!(d.page_count(), 2);
    let pdf = d.finish();
    check_xref(&pdf);
    assert!(find(&pdf, b"(Hello \\(world\\) \\\\ caf\\351 \\2005) Tj").is_some());
    assert!(find(&pdf, b"(Second) Tj").is_some());
    assert!(find(&pdf, b"/BaseFont /Helvetica /Encoding /WinAnsiEncoding").is_some());
    assert!(find(&pdf, b"/BaseFont /Courier-Bold").is_some());
    assert!(find(&pdf, b"/BaseFont /Times-Bold").is_some());
    assert!(find(&pdf, b"/Count 2").is_some());
    assert!(find(&pdf, b"/MediaBox [0 0 842 595]").is_some());
    assert!(find(&pdf, b"/Title (Report \\(Q1\\))").is_some());
    // Top-left origin: a line from (0,0) starts at the top of the page.
    assert!(find(&pdf, b"0 792 m 612 0 l S").is_some());
}

#[test]
fn images_with_alpha() {
    let mut d = PdfDoc::new();
    let p = d.add_page(200.0, 200.0);
    let mut px = Vec::new();
    for i in 0..16u8 {
        px.extend_from_slice(&[i * 10, 0, 0, if i < 8 { 255 } else { 128 }]);
    }
    p.image_rgba(10.0, 10.0, 40.0, 40.0, &px, 4, 4);
    p.image_rgba(10.0, 10.0, 40.0, 40.0, &[255; 16], 2, 2); // opaque: no mask
    p.image_rgba(0.0, 0.0, 1.0, 1.0, &[1, 2, 3], 4, 4); // too short: ignored
    let pdf = d.finish();
    check_xref(&pdf);
    assert!(find(&pdf, b"/SMask").is_some());
    assert!(find(&pdf, b"/Im1 Do").is_some());
    assert!(find(&pdf, b"/Im2 Do").is_some());
    assert!(find(&pdf, b"/Im3").is_none());
}

#[test]
fn run_length_roundtrip() {
    fn decode(d: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < d.len() {
            let l = d[i];
            i += 1;
            if l == 128 {
                break;
            } else if l < 128 {
                out.extend_from_slice(&d[i..i + l as usize + 1]);
                i += l as usize + 1;
            } else {
                out.extend(std::iter::repeat_n(d[i], 257 - l as usize));
                i += 1;
            }
        }
        out
    }
    let cases: Vec<Vec<u8>> = vec![
        vec![],
        vec![1],
        vec![1, 1],
        vec![1, 2, 3, 3, 3, 3, 4, 5, 5],
        (0..1000).map(|i| (i / 7) as u8).collect(),
        (0..1000).map(|i| (i * 31 % 251) as u8).collect(),
        vec![9; 300],
    ];
    for c in cases {
        assert_eq!(decode(&run_length(&c)), c);
    }
}

#[test]
fn widths() {
    assert!((text_width(Font::Helvetica, 10.0, "Hello") - 22.78).abs() < 0.01);
    assert!(text_width(Font::HelveticaBold, 10.0, "Hello") > text_width(Font::Helvetica, 10.0, "Hello"));
    assert_eq!(text_width(Font::Courier, 10.0, "abc"), 18.0);
    assert!(text_width(Font::TimesRoman, 12.0, "é") > 0.0);
    assert_eq!(text_width(Font::Helvetica, 10.0, ""), 0.0);
    assert_eq!(Font::family_for("Calibri"), Family::Sans);
    assert_eq!(Font::family_for("Times New Roman"), Family::Serif);
    assert_eq!(Font::family_for("Consolas"), Family::Mono);
    assert_eq!(Font::styled(Family::Serif, true, true), Font::TimesBoldItalic);
}

#[test]
fn hostile_inputs_never_panic() {
    let mut d = PdfDoc::new();
    let p = d.add_page(f32::NAN, -5.0);
    p.text(f32::INFINITY, f32::NAN, 1e30, Font::Helvetica, [0, 0, 0], "x");
    p.fill_rect(0.0, 0.0, -1.0, 5.0, [0, 0, 0]);
    p.polygon(&[], None, None);
    p.circle(0.0, 0.0, -3.0, Some([0, 0, 0]), None);
    p.restore();
    p.restore();
    p.save();
    p.image_rgba(0.0, 0.0, 10.0, 10.0, &[], u32::MAX, u32::MAX);
    let pdf = d.finish();
    check_xref(&pdf);
}
