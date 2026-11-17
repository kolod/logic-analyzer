//! The icon files and the declarations that use them (spec 014, user story 6, task T045).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn icons() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icon")
}

/// A small check that `text` is well-formed XML: the tags nest and close and there is one root.
fn assert_well_formed(text: &str, what: &str) {
    let mut stack: Vec<String> = Vec::new();
    let mut roots = 0;
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        let after = &rest[start + 1..];
        let end = after
            .find('>')
            .unwrap_or_else(|| panic!("{what}: unterminated tag"));
        let tag = &after[..end];
        rest = &after[end + 1..];
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            assert_eq!(
                stack.pop().as_deref(),
                Some(name.trim()),
                "{what}: closing {name}"
            );
        } else {
            let name = tag
                .split(|c: char| c.is_whitespace() || c == '/')
                .next()
                .unwrap()
                .to_owned();
            if stack.is_empty() {
                roots += 1;
            }
            if !tag.ends_with('/') {
                stack.push(name);
            }
        }
    }
    assert!(stack.is_empty(), "{what}: unclosed {stack:?}");
    assert_eq!(roots, 1, "{what}: one root element");
}

#[test]
fn the_svg_files_are_well_formed_and_have_a_view_box() {
    for name in ["app.svg", "session.svg"] {
        let text = std::fs::read_to_string(icons().join(name)).unwrap();
        assert_well_formed(&text, name);
        assert!(text.contains("viewBox=\"0 0 256 256\""), "{name}");
        // Original artwork with no raster or font inside.
        assert!(
            !text.contains("<image") && !text.contains("<text"),
            "{name}"
        );
    }
}

#[test]
fn the_png_files_have_the_size_in_their_names() {
    for kind in ["app", "session"] {
        for size in [16_u32, 32, 48, 64, 128, 256] {
            let bytes = std::fs::read(icons().join(format!("{kind}-{size}.png"))).unwrap();
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{kind}-{size}");
            assert_eq!(&bytes[12..16], b"IHDR", "{kind}-{size}");
            let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
            let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
            assert_eq!((width, height), (size, size), "{kind}-{size}");
        }
    }
}

#[test]
fn the_ico_files_hold_the_four_sizes() {
    for name in ["app.ico", "session.ico"] {
        let bytes = std::fs::read(icons().join(name)).unwrap();
        assert_eq!(&bytes[..4], &[0, 0, 1, 0], "{name}: ICO header");
        let count = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
        assert_eq!(count, 4, "{name}");
        let mut sizes: Vec<u32> = (0..count)
            .map(|i| {
                let width = bytes[6 + 16 * i];
                if width == 0 { 256 } else { u32::from(width) }
            })
            .collect();
        sizes.sort_unstable();
        assert_eq!(sizes, [16, 32, 48, 256], "{name}");
        // Every entry points inside the file.
        for i in 0..count {
            let entry = &bytes[6 + 16 * i..22 + 16 * i];
            let length = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
            assert!(offset + length <= bytes.len(), "{name}: entry {i}");
        }
    }
}

#[test]
fn the_window_icon_is_64_by_64_raw_pixels_and_not_one_colour() {
    let bytes = std::fs::read(icons().join("app-64.rgba")).unwrap();
    assert_eq!(bytes.len(), 64 * 64 * 4);
    assert!(bytes.iter().any(|b| *b != bytes[0]), "not all one value");
    // The corner is clear (a rounded square) and the middle is not.
    assert_eq!(bytes[3], 0, "the top left corner is transparent");
    let middle = (32 * 64 + 32) * 4;
    assert_eq!(bytes[middle + 3], 255, "the middle is opaque");
}

#[test]
fn the_desktop_file_declares_the_application_and_the_session_type() {
    let text =
        std::fs::read_to_string(root().join("packaging/linux/logic-analyzer.desktop")).unwrap();
    assert!(text.starts_with("[Desktop Entry]"));
    for line in [
        "Type=Application",
        "Exec=logic-analyzer %f",
        "MimeType=application/x-logic-analyzer-session;",
    ] {
        assert!(text.lines().any(|l| l == line), "{line}");
    }
    for key in ["Name=", "Icon="] {
        assert!(text.lines().any(|l| l.starts_with(key)), "{key}");
    }
}

#[test]
fn the_mime_type_matches_the_extension_of_the_session_format() {
    let text =
        std::fs::read_to_string(root().join("packaging/linux/logic-analyzer-session.xml")).unwrap();
    assert_well_formed(&text, "logic-analyzer-session.xml");
    assert!(text.contains("type=\"application/x-logic-analyzer-session\""));
    let glob = format!("<glob pattern=\"*.{}\"/>", la_session::FILE_EXTENSION);
    assert!(text.contains(&glob), "{glob}");
    // The icon it names is one of the files.
    let name = text
        .split("<icon name=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("an icon name");
    assert!(icons().join(format!("{name}.svg")).exists(), "{name}");
}

#[test]
fn the_windows_notes_name_the_extension_and_the_icon_file() {
    let text = std::fs::read_to_string(root().join("packaging/windows/README.md")).unwrap();
    assert!(text.contains(&format!(".{}", la_session::FILE_EXTENSION)));
    assert!(text.contains("app.ico") && text.contains("session.ico"));
}
