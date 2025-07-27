pub fn resolve_escape_sequences(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next_char) = chars.peek() {
                match next_char {
                    'n' => {
                        chars.next(); // consume the 'n'
                        result.push('\n');
                    }
                    't' => {
                        chars.next(); // consume the 't'
                        result.push('\t');
                    }
                    'r' => {
                        chars.next(); // consume the 'r'
                        result.push('\r');
                    }
                    '\\' => {
                        chars.next(); // consume the second '\'
                        result.push('\\');
                    }
                    'x' => {
                        chars.next(); // consume the 'x'
                        let mut hex_value = 0u8;
                        let mut hex_digits = 0;
                        
                        // Try to read up to 2 hex digits
                        for _ in 0..2 {
                            if let Some(&hex_char) = chars.peek() {
                                if let Some(digit) = hex_char.to_digit(16) {
                                    chars.next();
                                    hex_value = (hex_value << 4) | (digit as u8);
                                    hex_digits += 1;
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        
                        if hex_digits > 0 {
                            result.push(hex_value as char);
                        } else {
                            // No valid hex digits after \x, treat as invalid sequence
                            result.push('\\');
                            result.push('x');
                        }
                    }
                    _ => {
                        // Invalid escape sequence, keep backslash and character as-is
                        result.push('\\');
                        result.push(next_char);
                        chars.next(); // consume the invalid character
                    }
                }
            } else {
                // Backslash at end of string
                result.push('\\');
            }
        } else {
            result.push(c);
        }
    }
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nothing() {
        assert_eq!("asdf", resolve_escape_sequences("asdf"));
    }

    #[test]
    fn test_newline() {
        assert_eq!("asdf\n", resolve_escape_sequences("asdf\\n"));
    }

    #[test]
    fn test_hex() {
        assert_eq!("asdf\n", resolve_escape_sequences("asdf\\x0A"));
    }

    #[test]
    fn test_hex_single() {
        assert_eq!("asdf\n", resolve_escape_sequences("asdf\\xA"));
    }

    #[test]
    fn test_multiple() {
        assert_eq!("\n\t", resolve_escape_sequences("\\n\\t"));
    }

    // Tests for invalid escape sequences
    #[test]
    fn test_invalid_escape_q() {
        assert_eq!("asdf\\q", resolve_escape_sequences("asdf\\q"));
    }

    #[test]
    fn test_invalid_escape_number() {
        assert_eq!("test\\1", resolve_escape_sequences("test\\1"));
    }

    #[test]
    fn test_invalid_escape_symbol() {
        assert_eq!("hello\\$world", resolve_escape_sequences("hello\\$world"));
    }

    #[test]
    fn test_invalid_escape_space() {
        assert_eq!("text\\ space", resolve_escape_sequences("text\\ space"));
    }

    // Tests for edge cases
    #[test]
    fn test_empty_string() {
        assert_eq!("", resolve_escape_sequences(""));
    }

    #[test]
    fn test_single_backslash() {
        assert_eq!("\\", resolve_escape_sequences("\\"));
    }

    #[test]
    fn test_only_backslashes() {
        assert_eq!("\\\\", resolve_escape_sequences("\\\\\\\\"));
    }

    #[test]
    fn test_backslash_at_end() {
        assert_eq!("abc\\", resolve_escape_sequences("abc\\"));
    }

    #[test]
    fn test_backslash_at_start() {
        assert_eq!("\\abc", resolve_escape_sequences("\\abc"));
    }

    #[test]
    fn test_consecutive_backslashes() {
        assert_eq!("\\\\\\", resolve_escape_sequences("\\\\\\\\\\"));
    }

    // Tests for mixed valid and invalid sequences
    #[test]
    fn test_mixed_valid_invalid() {
        assert_eq!("valid\nand\\qinvalid", resolve_escape_sequences("valid\\nand\\qinvalid"));
    }

    #[test]
    fn test_mixed_sequences_complex() {
        assert_eq!("line1\n\\zline2\tand\\@more", resolve_escape_sequences("line1\\n\\zline2\\tand\\@more"));
    }

    #[test]
    fn test_valid_after_invalid() {
        assert_eq!("\\x\nvalid", resolve_escape_sequences("\\x\\nvalid"));
    }

    // Tests for hex edge cases
    #[test]
    fn test_hex_invalid_digits() {
        assert_eq!("test\\xGH", resolve_escape_sequences("test\\xGH"));
    }

    #[test]
    fn test_hex_invalid_single_digit() {
        assert_eq!("test\\xZ", resolve_escape_sequences("test\\xZ"));
    }

    #[test]
    fn test_hex_incomplete_at_end() {
        assert_eq!("test\\x", resolve_escape_sequences("test\\x"));
    }

    #[test]
    fn test_hex_zero() {
        assert_eq!("test\x00", resolve_escape_sequences("test\\x00"));
    }

    #[test]
    fn test_hex_uppercase() {
        assert_eq!("test\x0A", resolve_escape_sequences("test\\x0A"));
    }

    #[test]
    fn test_hex_lowercase() {
        assert_eq!("test\x0a", resolve_escape_sequences("test\\x0a"));
    }

    #[test]
    fn test_hex_mixed_case() {
        assert_eq!("test\x0B", resolve_escape_sequences("test\\x0b"));
    }

    // Tests for all standard escape sequences
    #[test]
    fn test_tab_escape() {
        assert_eq!("before\tafter", resolve_escape_sequences("before\\tafter"));
    }

    #[test]
    fn test_carriage_return_escape() {
        assert_eq!("before\rafter", resolve_escape_sequences("before\\rafter"));
    }

    #[test]
    fn test_backslash_escape() {
        assert_eq!("before\\after", resolve_escape_sequences("before\\\\after"));
    }

    #[test]
    fn test_all_standard_escapes() {
        assert_eq!("newline:\ntab:\tcarriage:\rbackslash:\\", 
                   resolve_escape_sequences("newline:\\ntab:\\tcarriage:\\rbackslash:\\\\"));
    }

    // Performance test with long strings containing numerous escape sequences
    #[test]
    fn test_performance_long_string() {
        let mut input = String::new();
        let mut expected = String::new();
        
        // Create a string with 1000 escape sequences
        for i in 0..1000 {
            match i % 4 {
                0 => {
                    input.push_str("\\n");
                    expected.push('\n');
                }
                1 => {
                    input.push_str("\\t");
                    expected.push('\t');
                }
                2 => {
                    input.push_str("\\x41"); // 'A'
                    expected.push('A');
                }
                3 => {
                    input.push_str("\\\\");
                    expected.push('\\');
                }
                _ => unreachable!(),
            }
            input.push('x'); // Add regular character between escapes
            expected.push('x');
        }
        
        assert_eq!(expected, resolve_escape_sequences(&input));
    }

    #[test]
    fn test_performance_mixed_valid_invalid() {
        let mut input = String::new();
        let mut expected = String::new();
        
        // Create a string with mix of valid and invalid sequences
        for i in 0..500 {
            match i % 6 {
                0 => {
                    input.push_str("\\n");
                    expected.push('\n');
                }
                1 => {
                    input.push_str("\\q"); // invalid
                    expected.push_str("\\q");
                }
                2 => {
                    input.push_str("\\t");
                    expected.push('\t');
                }
                3 => {
                    input.push_str("\\@"); // invalid
                    expected.push_str("\\@");
                }
                4 => {
                    input.push_str("\\x42"); // 'B'
                    expected.push('B');
                }
                5 => {
                    input.push_str("\\z"); // invalid
                    expected.push_str("\\z");
                }
                _ => unreachable!(),
            }
            input.push_str("text");
            expected.push_str("text");
        }
        
        assert_eq!(expected, resolve_escape_sequences(&input));
    }

    // Additional edge case tests
    #[test]
    fn test_multiple_backslashes_before_valid() {
        assert_eq!("\\\n", resolve_escape_sequences("\\\\\\n"));
    }

    #[test]
    fn test_hex_with_more_than_two_digits() {
        // Should only consume first two hex digits (0x41 = 'A')
        assert_eq!("A", resolve_escape_sequences("\\x41"));
    }

    #[test]
    fn test_unicode_preservation() {
        assert_eq!("hello 世界\\q test", resolve_escape_sequences("hello 世界\\q test"));
    }
}
