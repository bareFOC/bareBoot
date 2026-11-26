//! Initial safe pin execution on target microcontroller reset.
//!
//! Applies configured Klipper-style safe pin states (`!pin` = OutputLow,
//! `pin` = OutputHigh, `^pin` = InputPullUp, `~pin` = InputPullDown)
//! immediately on boot before peripheral clocks or transport listeners are started.

/// Pin configuration mode at bootloader startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinMode {
    /// Push-pull output driven low (`!PA9`).
    OutputLow,
    /// Push-pull output driven high (`PA9`).
    OutputHigh,
    /// Input with internal pull-up (`^PB12` / `^!PB12`).
    InputPullUp,
    /// Input with internal pull-down (`~PB12` / `~!PB12`).
    InputPullDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafePin {
    pub port: char,
    pub pin: u8,
    pub mode: PinMode,
}

impl SafePin {
    pub const fn new(port: char, pin: u8, mode: PinMode) -> Self {
        Self { port, pin, mode }
    }

    /// Parse a Klipper-style pin specification string:
    /// - `"!PB15"` -> `OutputLow`
    /// - `"PA7"`   -> `OutputHigh`
    /// - `"^PB12"` or `"^!PB12"` -> `InputPullUp`
    /// - `"~PC13"` or `"~!PC13"` -> `InputPullDown`
    pub fn parse(s: &str) -> Result<Self, ()> {
        let mut rest = s.trim();
        if rest.is_empty() {
            return Err(());
        }

        let mut inverted = false;
        let mut pull_up = false;
        let mut pull_down = false;

        while let Some(first) = rest.as_bytes().first().copied() {
            match first {
                b'!' => inverted = true,
                b'^' => pull_up = true,
                b'~' => pull_down = true,
                _ => break,
            }
            rest = &rest[1..];
        }

        if pull_up && pull_down {
            return Err(());
        }

        let mode = if pull_up {
            PinMode::InputPullUp
        } else if pull_down {
            PinMode::InputPullDown
        } else if inverted {
            PinMode::OutputLow
        } else {
            PinMode::OutputHigh
        };

        let bytes = rest.as_bytes();
        if bytes.len() < 2 {
            return Err(());
        }

        let (port_char, num_start) = if bytes[0] == b'P' || bytes[0] == b'p' {
            let p = (bytes[1] as char).to_ascii_uppercase();
            (p, 2)
        } else {
            let p = (bytes[0] as char).to_ascii_uppercase();
            (p, 1)
        };

        if !('A'..='Z').contains(&port_char) {
            return Err(());
        }

        let num_str = core::str::from_utf8(&bytes[num_start..]).map_err(|_| ())?;
        if num_str.is_empty() {
            return Err(());
        }
        let pin_num: u8 = num_str.parse().map_err(|_| ())?;
        if pin_num > 31 {
            return Err(());
        }

        Ok(Self {
            port: port_char,
            pin: pin_num,
            mode,
        })
    }
}

pub trait PinDriver {
    fn configure(&mut self, port: char, pin: u8, mode: PinMode);
}

impl<F> PinDriver for F
where
    F: FnMut(char, u8, PinMode),
{
    fn configure(&mut self, port: char, pin: u8, mode: PinMode) {
        self(port, pin, mode);
    }
}

/// Apply configured initial safe pins using a [`PinDriver`].
pub fn apply_safe_pins<D: PinDriver>(pins: &[SafePin], driver: &mut D) {
    for pin in pins {
        driver.configure(pin.port, pin.pin, pin.mode);
    }
}

/// Macro for declarative static definition of initial safe pins.
#[macro_export]
macro_rules! initial_safe_pins {
    ($( ($port:expr, $pin:expr, $mode:ident) ),* $(,)?) => {
        [
            $(
                $crate::safe_pins::SafePin {
                    port: $port,
                    pin: $pin,
                    mode: $crate::safe_pins::PinMode::$mode,
                },
            )*
        ]
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_klipper_pin() {
        assert_eq!(
            SafePin::parse("!PB15").unwrap(),
            SafePin::new('B', 15, PinMode::OutputLow)
        );
        assert_eq!(
            SafePin::parse("PA7").unwrap(),
            SafePin::new('A', 7, PinMode::OutputHigh)
        );
        assert_eq!(
            SafePin::parse("^!PB12").unwrap(),
            SafePin::new('B', 12, PinMode::InputPullUp)
        );
        assert_eq!(
            SafePin::parse("~PC13").unwrap(),
            SafePin::new('C', 13, PinMode::InputPullDown)
        );
        assert_eq!(
            SafePin::parse("!B1").unwrap(),
            SafePin::new('B', 1, PinMode::OutputLow)
        );

        assert!(SafePin::parse("PB15:low").is_err());
        assert!(SafePin::parse("^~PB12").is_err());
        assert!(SafePin::parse("invalid").is_err());
        assert!(SafePin::parse("PB99").is_err());
    }

    #[test]
    fn test_macro_and_apply() {
        let pins = initial_safe_pins![
            ('B', 15, OutputLow),
            ('B', 1, OutputHigh),
            ('B', 12, InputPullUp),
            ('C', 13, InputPullDown),
        ];
        assert_eq!(pins.len(), 4);

        let mut applied = [(' ', 0u8, PinMode::OutputHigh); 4];
        let mut count = 0;
        apply_safe_pins(&pins, &mut |port, pin, mode| {
            applied[count] = (port, pin, mode);
            count += 1;
        });

        assert_eq!(count, 4);
        assert_eq!(applied[0], ('B', 15, PinMode::OutputLow));
        assert_eq!(applied[1], ('B', 1, PinMode::OutputHigh));
        assert_eq!(applied[2], ('B', 12, PinMode::InputPullUp));
        assert_eq!(applied[3], ('C', 13, PinMode::InputPullDown));
    }
}
