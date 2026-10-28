//! Small SVG gauges, regenerated whenever the numbers change.

use cosmic::widget::svg;

/// Anthropic brand orange.
pub const ORANGE: &str = "#D97757";

/// Square gauge that fills from the bottom. `pace` (0–1) draws a tick at how far
/// through the window we are, so a fill above the tick means burning faster than even.
pub fn square(percent: f32, pace: Option<f32>, tick_color: &str) -> svg::Handle {
    let h = 100.0 * (percent / 100.0).clamp(0.0, 1.0);
    let mut s = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
<defs><clipPath id="c"><rect x="0" y="0" width="100" height="100" rx="14"/></clipPath></defs>
<g clip-path="url(#c)">
<rect x="0" y="0" width="100" height="100" fill="{ORANGE}" fill-opacity="0.18"/>
<rect x="0" y="{y:.2}" width="100" height="{h:.2}" fill="{ORANGE}"/>"#,
        y = 100.0 - h,
    );
    if let Some(p) = pace {
        let y = 100.0 - 100.0 * p.clamp(0.0, 1.0);
        s.push_str(&format!(
            r#"<line x1="0" y1="{y:.2}" x2="100" y2="{y:.2}" stroke="{tick_color}" stroke-width="6" stroke-opacity="0.75"/>"#
        ));
    }
    s.push_str(&format!(
        r#"</g><rect x="3" y="3" width="94" height="94" rx="12" fill="none" stroke="{ORANGE}" stroke-width="6"/></svg>"#
    ));
    svg::Handle::from_memory(s.into_bytes())
}

/// Horizontal bar for the popup, same styling as the square.
pub fn bar(percent: f32, pace: Option<f32>, tick_color: &str) -> svg::Handle {
    let w = 300.0 * (percent / 100.0).clamp(0.0, 1.0);
    let mut s = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 12">
<defs><clipPath id="c"><rect x="0" y="0" width="300" height="12" rx="6"/></clipPath></defs>
<g clip-path="url(#c)">
<rect x="0" y="0" width="300" height="12" fill="{ORANGE}" fill-opacity="0.18"/>
<rect x="0" y="0" width="{w:.2}" height="12" fill="{ORANGE}"/>"#
    );
    if let Some(p) = pace {
        let x = 300.0 * p.clamp(0.0, 1.0);
        s.push_str(&format!(
            r#"<line x1="{x:.2}" y1="0" x2="{x:.2}" y2="12" stroke="{tick_color}" stroke-width="2.5" stroke-opacity="0.8"/>"#
        ));
    }
    s.push_str("</g></svg>");
    svg::Handle::from_memory(s.into_bytes())
}
