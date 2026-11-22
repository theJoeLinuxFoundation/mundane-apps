use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

type Parsed = Result<BigRational, &'static str>;
const MAX_BYTES: usize = 1024;

pub fn evaluate(expression: &str) -> Result<String, &'static str> {
    if expression.len() > MAX_BYTES {
        return Err("Expression is too long.");
    }
    let mut parser = Parser(expression);
    let value = parser.sum(0)?;
    let value = if parser.0.trim().is_empty() {
        value
    } else {
        super::units::convert(value, parser.0)?
    };

    let mut answer = format_value(&value);
    // A terminating decimal can be much longer than its reduced fraction.
    if operand(&answer).len() > MAX_BYTES {
        answer = value.to_string();
        if operand(&answer).len() > MAX_BYTES {
            return Err("Result is too long to reuse.");
        }
    }
    Ok(answer)
}

struct Parser<'a>(&'a str);

impl Parser<'_> {
    fn take(&mut self, operators: &str) -> Option<char> {
        self.0 = self.0.trim_start();
        let op = self.0.chars().next().filter(|c| operators.contains(*c))?;
        self.0 = &self.0[op.len_utf8()..];
        Some(op)
    }

    fn sum(&mut self, depth: u8) -> Parsed {
        let (mut value, _) = self.product(depth)?;
        while let Some(op) = self.take("+-−") {
            let (mut rhs, percent) = self.product(depth)?;
            if percent {
                rhs *= &value;
            }
            if op == '+' {
                value += rhs
            } else {
                value -= rhs
            }
        }
        Ok(value)
    }

    fn product(&mut self, depth: u8) -> Result<(BigRational, bool), &'static str> {
        let (mut value, mut percent) = self.percent_factor(depth)?;
        while let Some(op) = self.take("*/×÷") {
            let (rhs, _) = self.percent_factor(depth)?;
            percent = false;
            match op {
                '*' | '×' => value *= rhs,
                _ if rhs.is_zero() => return Err("Cannot divide by zero."),
                _ => value /= rhs,
            }
        }
        Ok((value, percent))
    }

    fn percent_factor(&mut self, depth: u8) -> Result<(BigRational, bool), &'static str> {
        let value = self.factor(depth)?;
        if self.take("%").is_some() {
            Ok((value / BigInt::from(100), true))
        } else {
            Ok((value, false))
        }
    }

    fn factor(&mut self, depth: u8) -> Parsed {
        let mut negative = false;
        while let Some(sign) = self.take("+-−") {
            if sign != '+' {
                negative = !negative;
            }
        }
        let value = if self.take("(").is_some() {
            if depth >= 32 {
                return Err("Too many nested parentheses.");
            }
            let value = self.sum(depth + 1)?;
            self.take(")").ok_or("Missing closing parenthesis.")?;
            value
        } else {
            let length = self
                .0
                .bytes()
                .take_while(|b| b.is_ascii_digit() || *b == b'.')
                .count();
            let (number, rest) = self.0.split_at(length);
            self.0 = rest;
            let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
            let digits = format!("{whole}{fraction}");
            let numerator = BigInt::parse_bytes(digits.as_bytes(), 10)
                .ok_or("Expected a decimal number or parentheses.")?;
            BigRational::new(numerator, BigInt::from(10u8).pow(fraction.len() as u32))
        };
        Ok(if negative { -value } else { value })
    }
}

fn format_value(value: &BigRational) -> String {
    // A reduced fraction terminates in base ten precisely when its denominator
    // divides 10^n. Keep other results as fractions so they can be reused exactly.
    let mut denominator = value.denom().clone();
    let mut places = 0;
    for factor in [2u8, 5] {
        let mut power = 0;
        while (&denominator % factor).is_zero() {
            denominator /= factor;
            power += 1;
        }
        places = places.max(power);
    }
    if !denominator.is_one() || places == 0 {
        return value.to_string();
    }
    let scaled = value.numer().abs() * BigInt::from(10u8).pow(places) / value.denom();
    let digits = format!("{scaled:0width$}", width = places as usize + 1);
    let (whole, fraction) = digits.split_at(digits.len() - places as usize);
    format!(
        "{}{whole}.{fraction}",
        if value.is_negative() { "-" } else { "" }
    )
}

pub fn operand(answer: &str) -> String {
    if answer.contains('/') {
        format!("({answer})")
    } else {
        answer.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precedence_decimals_and_exact_reuse() {
        for (source, expected) in [
            ("12 - 2 * (3 + .25)", "5.5"),
            ("0.1 + 0.2", "0.3"),
            ("200 + 15%", "230"),
            ("200 - 15%", "170"),
            ("200 × 15%", "30"),
            ("(80 + 20) * 5%", "5"),
            ("50%", "0.5"),
            ("(3 + 2) mi to km", "8.04672"),
            ("32 °F to °C", "0"),
            ("-40 C to F", "-40"),
            ("1 lb to g", "453.59237"),
            ("1 h to min", "60"),
            ("5 ft to in", "60"),
            ("18 / 3 / 2", "3"),
            ("−.004 × -2", "0.008"),
            ("1 ÷ -3", "-1/3"),
            ("9007199254740993 + 1", "9007199254740994"),
            ("1. - 1", "0"),
            ("1/40", "0.025"),
        ] {
            assert_eq!(evaluate(source), Ok(expected.into()), "{source}");
        }
        let third = evaluate("1/3").unwrap();
        assert_eq!(
            evaluate(&format!("2 / {}", operand(&third))),
            Ok("6".into())
        );
    }
    #[test]
    fn long_terminating_fraction_stays_reusable() {
        let expression = format!("1/{}", BigInt::from(2).pow(1500));
        let answer = evaluate(&expression).unwrap();
        assert_eq!(answer, expression);
        assert_eq!(evaluate(&operand(&answer)), Ok(answer));
    }

    #[test]
    fn errors_and_input_limits() {
        assert_eq!(evaluate("1/(4-4)"), Err("Cannot divide by zero."));
        for input in [
            "",
            ".",
            "2a",
            "1.2.3",
            "1 2",
            "1+",
            "()",
            "(2",
            "2)",
            "2(5)",
            "1 kg to km",
            "1 ft to madeup",
        ] {
            assert!(evaluate(input).is_err(), "{input}");
        }
        assert!(evaluate(&format!("{}1{}", "(".repeat(100), ")".repeat(100))).is_err());
        assert!(evaluate(&"9".repeat(1025)).is_err());
    }
}
