use num_bigint::BigInt;
use num_rational::BigRational;

type Unit = (&'static str, &'static str, i64, i64, i64, i64);
// The base value is input × scale + offset. All ratios are exact.
const UNITS: &[Unit] = &[
    ("mm", "length", 1, 1000, 0, 1),
    ("cm", "length", 1, 100, 0, 1),
    ("m", "length", 1, 1, 0, 1),
    ("km", "length", 1000, 1, 0, 1),
    ("in", "length", 254, 10000, 0, 1),
    ("ft", "length", 3048, 10000, 0, 1),
    ("yd", "length", 9144, 10000, 0, 1),
    ("mi", "length", 1609344, 1000, 0, 1),
    ("g", "mass", 1, 1, 0, 1),
    ("kg", "mass", 1000, 1, 0, 1),
    ("oz", "mass", 45359237, 1600000, 0, 1),
    ("lb", "mass", 45359237, 100000, 0, 1),
    ("ml", "volume", 1, 1000, 0, 1),
    ("l", "volume", 1, 1, 0, 1),
    ("s", "time", 1, 1, 0, 1),
    ("min", "time", 60, 1, 0, 1),
    ("h", "time", 3600, 1, 0, 1),
    ("c", "temperature", 1, 1, 0, 1),
    ("f", "temperature", 5, 9, -160, 9),
    ("k", "temperature", 1, 1, -27315, 100),
];

pub const PRESETS: &[(&str, &str)] = &[
    ("mi", "km"),
    ("km", "mi"),
    ("ft", "m"),
    ("in", "cm"),
    ("lb", "kg"),
    ("kg", "lb"),
    ("oz", "g"),
    ("L", "mL"),
    ("h", "min"),
    ("°F", "°C"),
    ("°C", "°F"),
];

pub fn convert(input: BigRational, suffix: &str) -> Result<BigRational, &'static str> {
    let mut words = suffix.split_whitespace();
    let find = |s: Option<&str>| {
        UNITS
            .iter()
            .find(|u| s.is_some_and(|s| s.trim_start_matches('°').eq_ignore_ascii_case(u.0)))
            .ok_or("Unknown unit. Choose Units for common conversions.")
    };
    let from = find(words.next())?;
    if words.next() != Some("to") {
        return Err("Enter a conversion such as 3 mi to km.");
    }
    let to = find(words.next())?;
    if words.next().is_some() {
        return Err("Unexpected text after the target unit.");
    }
    if from.1 != to.1 {
        return Err("Choose two units of the same kind.");
    }
    let ratio = |n: i64, d: i64| BigRational::new(BigInt::from(n), BigInt::from(d));
    Ok(
        (input * ratio(from.2, from.3) + ratio(from.4, from.5) - ratio(to.4, to.5))
            / ratio(to.2, to.3),
    )
}
