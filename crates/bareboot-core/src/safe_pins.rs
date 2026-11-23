//! Initial safe pin execution on target microcontroller reset.
//!
//! Applies configured safe pin levels (e.g. gate driver sleep low, bridge enables low)
//! immediately on boot before peripheral clocks or transport listeners are started.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinLevel {
    Low,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafePin {
    pub port: char,
    pub pin: u8,
    pub level: PinLevel,
}

impl SafePin {
    pub const fn new(port: char, pin: u8, level: PinLevel) -> Self {
        Self { port, pin, level }
    }

    /// Parse a pin specification string such as "PB15:low" or "PA7:high" or "PC13".
    pub fn parse(s: &str) -> Result<Self, ()> {
        let trimmed = s.trim();
        let (pin_part, level_part) = if let Some((p, l)) = trimmed.split_once(':') {
            (p.trim(), l.trim())
        } else {
            (trimmed, "low")
        };

        let bytes = pin_part.as_bytes();
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
        let pin_num: u8 = num_str.parse().map_err(|_| ())?;
        if pin_num > 31 {
            return Err(());
        }

        let level = match level_part {
            "low" | "LOW" | "0" => PinLevel::Low,
            "high" | "HIGH" | "1" => PinLevel::High,
            _ => return Err(()),
        };

        Ok(Self {
            port: port_char,
            pin: pin_num,
            level,
        })
    }
}

pub trait PinDriver {
    fn set_level(&mut self, port: char, pin: u8, level: PinLevel);
}

impl<F> PinDriver for F
where
    F: FnMut(char, u8, PinLevel),
{
    fn set_level(&mut self, port: char, pin: u8, level: PinLevel) {
        self(port, pin, level);
    }
}

/// Apply configured initial safe pins using a PinDriver.
pub fn apply_safe_pins<D: PinDriver>(pins: &[SafePin], driver: &mut D) {
    for pin in pins {
        driver.set_level(pin.port, pin.pin, pin.level);
    }
}

/// Macro for declarative static definition of initial safe pins.
#[macro_export]
macro_rules! initial_safe_pins {
    ($( ($port:expr, $pin:expr, $level:ident) ),* $(,)?) => {
        [
            $(
                $crate::safe_pins::SafePin {
                    port: $port,
                    pin: $pin,
                    level: $crate::safe_pins::PinLevel::$level,
                },
            )*
        ]
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pin() {
        let p1 = SafePin::parse("PB15:low").unwrap();
        assert_eq!(p1, SafePin::new('B', 15, PinLevel::Low));

        let p2 = SafePin::parse("PA7:high").unwrap();
        assert_eq!(p2, SafePin::new('A', 7, PinLevel::High));

        let p3 = SafePin::parse("B1:0").unwrap();
        assert_eq!(p3, SafePin::new('B', 1, PinLevel::Low));

        let p4 = SafePin::parse("PC13").unwrap();
        assert_eq!(p4, SafePin::new('C', 13, PinLevel::Low));

        assert!(SafePin::parse("invalid").is_err());
        assert!(SafePin::parse("PB99:low").is_err());
    }

    #[test]
    fn test_macro_and_apply() {
        let pins = initial_safe_pins![
            ('B', 15, Low),
            ('B', 1, Low),
            ('B', 0, Low),
            ('A', 7, Low),
        ];
        assert_eq!(pins.len(), 4);

        let mut applied = [( ' ', 0u8, PinLevel::High ); 4];
        let mut count = 0;
        apply_safe_pins(&pins, &mut |port, pin, level| {
            applied[count] = (port, pin, level);
            count += 1;
        });

        assert_eq!(count, 4);
        assert_eq!(applied[0], ('B', 15, PinLevel::Low));
        assert_eq!(applied[1], ('B', 1, PinLevel::Low));
        assert_eq!(applied[2], ('B', 0, PinLevel::Low));
        assert_eq!(applied[3], ('A', 7, PinLevel::Low));
    }
}
