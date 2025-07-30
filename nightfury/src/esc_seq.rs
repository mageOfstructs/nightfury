use core::panic;

use EscapeSequenceState::*;

enum EscapeSequenceState {
    Nothing,
    Backslash,
    HighHex,
    LowHex,
}
impl EscapeSequenceState {
    fn advance(&mut self) {
        *self = match self {
            Nothing => Backslash,
            Backslash => HighHex,
            HighHex => LowHex,
            LowHex => panic!("cannot advance LowHex"),
        };
    }
    fn reset(&mut self) {
        *self = Nothing;
    }
}

pub fn resolve_escape_sequences(input: &str) -> String {
    let mut state = Nothing;
    let mut tmpc = 0;
    let ret = input.chars().filter_map(|c| match state {
        Nothing if c == '\\' => {
            state.advance();
            None
        }
        Backslash => {
            let mut reset = true;
            let ret = match c {
                'x' => {
                    state.advance();
                    reset = false;
                    None
                }
                'n' => Some('\n'),
                't' => Some('\t'),
                'r' => Some('\r'),
                '\\' => Some('\\'),
                _ => panic!("Invalid escape sequence {}", c),
            };
            if reset {
                state.reset();
            }
            ret
        }
        HighHex if let Some(d) = c.to_digit(16) => {
            tmpc = (d as u8) << 4;
            state.advance();
            None
        }
        LowHex if let Some(d) = c.to_digit(16) => {
            state.reset();
            tmpc |= d as u8;
            Some(tmpc as char)
        }
        LowHex => {
            state.reset();
            tmpc >>= 4;
            Some(tmpc as char)
        }
        _ => Some(c),
    });
    let mut ret: String = ret.collect();
    if let LowHex = state {
        ret.push((tmpc >> 4) as char);
    }
    ret
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
}
