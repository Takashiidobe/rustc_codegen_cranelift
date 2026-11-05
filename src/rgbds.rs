use std::fmt::Write;

pub(crate) fn new_source(module: &str) -> String {
    format!("SECTION \"{}\", ROM0\n\n", symbol_name(&format!("cgu_{module}")))
}

pub(crate) fn append_function(source: &mut String, symbol: &str, vcode: &str) {
    let symbol = symbol_name(symbol);
    writeln!(source, "EXPORT {symbol}\n{symbol}::").unwrap();
    for line in vcode.lines() {
        writeln!(source, "{}", localize_block_labels(line)).unwrap();
    }
    source.push('\n');
}

pub(crate) fn symbol_name(symbol: &str) -> String {
    let mut encoded = String::from("__rust_");
    for byte in symbol.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'_' {
            encoded.push(char::from(byte));
        } else {
            write!(encoded, "_{byte:02X}").unwrap();
        }
    }
    encoded
}

fn localize_block_labels(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut localized = String::with_capacity(line.len() + 1);
    let mut offset = 0;

    while offset < bytes.len() {
        let starts_block = bytes[offset..].starts_with(b"block")
            && (offset == 0 || !is_symbol_byte(bytes[offset - 1]));
        if starts_block {
            let digits_start = offset + b"block".len();
            let mut digits_end = digits_start;
            while digits_end < bytes.len() && bytes[digits_end].is_ascii_digit() {
                digits_end += 1;
            }
            if digits_end > digits_start
                && (digits_end == bytes.len() || !is_symbol_byte(bytes[digits_end]))
            {
                localized.push('.');
                localized.push_str(&line[offset..digits_end]);
                offset = digits_end;
                continue;
            }
        }

        let ch = line[offset..].chars().next().unwrap();
        localized.push(ch);
        offset += ch.len_utf8();
    }

    localized
}

fn is_symbol_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use super::{append_function, symbol_name};

    #[test]
    fn writes_rgbds_functions_and_local_block_labels() {
        let mut source = super::new_source("test");
        append_function(
            &mut source,
            "_RNvCs3foo3bar",
            "block0:\n  ld a, $01\n  jr block1\nblock1:\n  ret",
        );

        assert!(source.contains("EXPORT __rust__RNvCs3foo3bar"));
        assert!(source.contains(".block0:"));
        assert!(source.contains("jr .block1"));
        assert!(source.contains(".block1:"));
    }

    #[test]
    fn encodes_symbols_using_rgbds_identifier_characters() {
        assert_eq!(symbol_name("a.$b"), "__rust_a_2E_24b");
    }

    #[test]
    fn emitted_source_assembles_with_rgbasm() {
        if Command::new("rgbasm").arg("--version").output().is_err() {
            return;
        }

        let dir = std::env::temp_dir().join(format!("gbc-rs-rgbds-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let source_path = dir.join("test.asm");
        let object_path = dir.join("test.o");
        let mut source = super::new_source("rgbasm_test");
        append_function(
            &mut source,
            "test_function",
            "block0:\n  ld b, $2A\n  ld hl, sp - 2\n  ld a, [hl]\n  ret",
        );
        fs::write(&source_path, source).unwrap();

        let output =
            Command::new("rgbasm").arg("-o").arg(&object_path).arg(&source_path).output().unwrap();
        assert!(
            output.status.success(),
            "rgbasm failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn source_units_from_separate_codegen_units_link_with_rgblink() {
        if Command::new("rgbasm").arg("--version").output().is_err()
            || Command::new("rgblink").arg("--version").output().is_err()
        {
            return;
        }

        let dir = std::env::temp_dir().join(format!("gbc-rs-rgblink-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let mut object_paths = Vec::new();
        for unit in ["one", "two"] {
            let source_path = dir.join(format!("{unit}.asm"));
            let object_path = dir.join(format!("{unit}.o"));
            let mut source = super::new_source(unit);
            append_function(&mut source, &format!("function_{unit}"), "block0:\n  ret");
            fs::write(&source_path, source).unwrap();
            let output = Command::new("rgbasm")
                .arg("-o")
                .arg(&object_path)
                .arg(&source_path)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "rgbasm failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            object_paths.push(object_path);
        }

        let output = Command::new("rgblink")
            .arg("-o")
            .arg(dir.join("test.gb"))
            .args(&object_paths)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "rgblink failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
