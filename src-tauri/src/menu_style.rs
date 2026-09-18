//! macOS: puts each reminder's time left in a right-aligned, secondary-colour
//! column of the tray menu. NSMenu has no such column for plain text, so this
//! sets an attributed title with a right tab stop on the native items.

use objc2::{rc::Retained, runtime::AnyObject, AllocAnyThread};
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSColor, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSMenu, NSMutableParagraphStyle, NSParagraphStyleAttributeName,
    NSTextAlignment, NSTextTab,
};
use objc2_foundation::{NSArray, NSDictionary, NSMutableAttributedString, NSRange, NSString};

/// Space between the longest name and the time column, in points.
const GAP: f64 = 28.0;
/// Reserve room for the widest time we show, so the column doesn't jump.
const WIDEST_TIME: &str = "88h 88m";

/// `rows`: (menu title as created, time to show on the right).
pub fn apply(menu: &NSMenu, rows: &[(String, Option<String>)]) {
    let font = NSFont::menuFontOfSize(0.0);
    let widest_name = rows
        .iter()
        .map(|(name, _)| width(name, &font))
        .fold(0.0, f64::max);
    let tab_at = widest_name + GAP + width(WIDEST_TIME, &font);

    let style = NSMutableParagraphStyle::new();
    let tab = unsafe {
        NSTextTab::initWithTextAlignment_location_options(
            NSTextTab::alloc(),
            NSTextAlignment::Right,
            tab_at,
            &NSDictionary::new(),
        )
    };
    style.setTabStops(Some(&NSArray::from_retained_slice(&[tab])));
    let secondary = NSColor::secondaryLabelColor();

    for item in menu.itemArray().iter() {
        let title = item.title().to_string();
        let row = rows
            .iter()
            .find(|(name, _)| title == *name || title.starts_with(&format!("{name}\t")));
        let Some((name, time)) = row else {
            continue;
        };
        let text = match time {
            Some(time) => format!("{name}\t{time}"),
            None => name.clone(),
        };
        let attributed = styled(&text, &font);
        unsafe {
            add(&attributed, NSParagraphStyleAttributeName, &style, None);
            if time.is_some() {
                let start = name.encode_utf16().count() + 1;
                add(
                    &attributed,
                    NSForegroundColorAttributeName,
                    &secondary,
                    Some(start),
                );
            }
        }
        item.setAttributedTitle(Some(&attributed));
    }
}

fn styled(text: &str, font: &NSFont) -> Retained<NSMutableAttributedString> {
    let s = NSMutableAttributedString::initWithString(
        NSMutableAttributedString::alloc(),
        &NSString::from_str(text),
    );
    unsafe { add(&s, NSFontAttributeName, font, None) };
    s
}

/// Applies an attribute from UTF-16 offset `from` (or 0) to the end.
unsafe fn add(
    s: &NSMutableAttributedString,
    key: &objc2_foundation::NSAttributedStringKey,
    value: &impl AsRef<AnyObject>,
    from: Option<usize>,
) {
    let start = from.unwrap_or(0).min(s.length());
    let range = NSRange::new(start, s.length() - start);
    s.addAttribute_value_range(key, value.as_ref(), range);
}

fn width(text: &str, font: &NSFont) -> f64 {
    styled(text, font).size().width
}
