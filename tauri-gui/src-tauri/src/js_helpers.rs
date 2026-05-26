/// Reusable JavaScript helper snippets injected into PayPal pages.
/// Centralised here so every Rust file that builds JS strings can reference
/// a single, tested copy instead of copy-pasting the same 15-line helpers.

/// `isVisible(el)` – returns true when an element is rendered and not hidden.
pub const JS_IS_VISIBLE: &str = r#"
const isVisible = (el) => {
    if (!el) return false;
    const rect = el.getBoundingClientRect();
    const style = window.getComputedStyle(el);
    return rect.width > 0 &&
           rect.height > 0 &&
           style.display !== 'none' &&
           style.visibility !== 'hidden' &&
           style.opacity !== '0';
};
"#;

/// `findEl(doc, sel)` – recursively searches the document and all iframes
/// for an element by id, name attribute, or CSS selector.
pub const JS_FIND_EL: &str = r#"
const findEl = (doc, sel) => {
    if (!doc) return null;
    let el = doc.getElementById(sel) || doc.querySelector('[name="' + sel + '"]') || doc.querySelector(sel);
    if (el) return el;
    let frames = doc.querySelectorAll('iframe');
    for (let i = 0; i < frames.length; i++) {
        try { el = findEl(frames[i].contentDocument, sel); if (el) return el; } catch(e) {}
    }
    return null;
};
"#;
