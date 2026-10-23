//! Rules for numeric fields. JMeter fields that look numeric also accept `${variable}` and
//! `${__function()}` expressions, so those count as valid and are never stepped or clamped.

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    /// A variable or function reference, resolved when the test runs.
    Expression(String),
    Empty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    NotANumber,
    NotAWholeNumber,
    TooSmall { min: i64 },
    TooLarge { max: i64 },
}

impl Problem {
    pub fn message(&self) -> String {
        match self {
            Problem::NotANumber => "Enter a number, or a variable such as ${threads}".into(),
            Problem::NotAWholeNumber => "Enter a whole number".into(),
            Problem::TooSmall { min } => format!("Must be at least {min}"),
            Problem::TooLarge { max } => format!("Must be at most {max}"),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NumberSpec {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: f64,
    pub integer: bool,
    /// Whether an empty field is acceptable (JMeter treats empty as "use the default").
    pub allow_empty: bool,
}

impl NumberSpec {
    pub const fn whole(min: i64, max: i64) -> NumberSpec {
        NumberSpec { min: Some(min as f64), max: Some(max as f64), step: 1.0, integer: true, allow_empty: false }
    }

    pub const fn decimal(min: f64, max: f64, step: f64) -> NumberSpec {
        NumberSpec { min: Some(min), max: Some(max), step, integer: false, allow_empty: false }
    }

    pub fn optional(mut self) -> NumberSpec {
        self.allow_empty = true;
        self
    }

    /// Read the field text.
    pub fn parse(&self, text: &str) -> Result<Value, Problem> {
        let t = text.trim();
        if t.is_empty() {
            return if self.allow_empty { Ok(Value::Empty) } else { Err(Problem::NotANumber) };
        }
        if is_expression(t) {
            return Ok(Value::Expression(t.to_string()));
        }
        let v: f64 = t.parse().map_err(|_| Problem::NotANumber)?;
        if !v.is_finite() {
            return Err(Problem::NotANumber);
        }
        if self.integer && v.fract() != 0.0 {
            return Err(Problem::NotAWholeNumber);
        }
        if let Some(min) = self.min
            && v < min {
                return Err(Problem::TooSmall { min: min as i64 });
            }
        if let Some(max) = self.max
            && v > max {
                return Err(Problem::TooLarge { max: max as i64 });
            }
        Ok(Value::Number(v))
    }

    /// Could `text` become valid with more typing? Used to accept keystrokes such as `-` or `1.`.
    pub fn accepts_partial(&self, text: &str) -> bool {
        let t = text.trim();
        if t.is_empty() || is_expression(t) || t.starts_with('$') && "${".starts_with(&t[..t.len().min(2)]) {
            return true;
        }
        if t.starts_with("${") {
            return true; // still typing an expression
        }
        let negative_ok = self.min.is_none_or(|m| m < 0.0);
        let body = match t.strip_prefix('-') {
            Some(rest) if negative_ok => rest,
            Some(_) => return false,
            None => t,
        };
        let mut dots = 0;
        for c in body.chars() {
            match c {
                '0'..='9' => {}
                '.' if !self.integer => {
                    dots += 1;
                    if dots > 1 {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        true
    }

    pub fn clamp(&self, v: f64) -> f64 {
        let v = self.min.map_or(v, |m| v.max(m));
        self.max.map_or(v, |m| v.min(m))
    }

    /// The text after pressing up (`+1`) or down (`-1`); `big` steps by ten. Expressions and invalid
    /// text are left alone. An empty field starts from the minimum, or zero.
    pub fn step(&self, text: &str, direction: i32, big: bool) -> String {
        let current = match self.parse_for_step(text) {
            Some(v) => v,
            None => return text.to_string(),
        };
        let amount = self.step * if big { 10.0 } else { 1.0 } * f64::from(direction);
        self.format(self.clamp(current + amount))
    }

    fn parse_for_step(&self, text: &str) -> Option<f64> {
        let t = text.trim();
        if t.is_empty() {
            return Some(self.min.unwrap_or(0.0) - self.step * 0.0);
        }
        if is_expression(t) {
            return None;
        }
        t.parse::<f64>().ok().filter(|v| v.is_finite())
    }

    pub fn format(&self, v: f64) -> String {
        if self.integer || v.fract() == 0.0 {
            format!("{}", v as i64)
        } else {
            let s = format!("{v:.6}");
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
    }
}

/// Does the text contain a `${...}` reference?
pub fn is_expression(t: &str) -> bool {
    match t.find("${") {
        Some(i) => t[i + 2..].contains('}'),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USERS: NumberSpec = NumberSpec::whole(1, 1_000_000);
    const RATIO: NumberSpec = NumberSpec::decimal(0.0, 1.0, 0.1);

    #[test]
    fn parses_numbers() {
        assert_eq!(USERS.parse("50"), Ok(Value::Number(50.0)));
        assert_eq!(USERS.parse("  7 "), Ok(Value::Number(7.0)));
        assert_eq!(RATIO.parse("0.25"), Ok(Value::Number(0.25)));
    }

    #[test]
    fn rejects_bad_input_with_a_reason() {
        assert_eq!(USERS.parse("abc"), Err(Problem::NotANumber));
        assert_eq!(USERS.parse(""), Err(Problem::NotANumber));
        assert_eq!(USERS.parse("1.5"), Err(Problem::NotAWholeNumber));
        assert_eq!(USERS.parse("0"), Err(Problem::TooSmall { min: 1 }));
        assert_eq!(USERS.parse("2000000"), Err(Problem::TooLarge { max: 1_000_000 }));
        assert_eq!(USERS.parse("NaN"), Err(Problem::NotANumber));
        assert_eq!(USERS.parse("inf"), Err(Problem::NotANumber));
        assert!(Problem::TooSmall { min: 1 }.message().contains("at least 1"));
        assert!(Problem::NotANumber.message().contains("${threads}"));
    }

    #[test]
    fn variables_and_functions_are_valid() {
        assert_eq!(USERS.parse("${threads}"), Ok(Value::Expression("${threads}".into())));
        assert!(matches!(USERS.parse("${__P(users,10)}"), Ok(Value::Expression(_))));
        assert_eq!(USERS.parse("${unclosed"), Err(Problem::NotANumber));
        assert!(is_expression("a${b}c"));
        assert!(!is_expression("$ {x}"));
    }

    #[test]
    fn empty_is_allowed_only_when_optional() {
        assert_eq!(USERS.optional().parse(""), Ok(Value::Empty));
        assert_eq!(USERS.optional().parse("  "), Ok(Value::Empty));
    }

    #[test]
    fn partial_input_is_accepted_while_typing() {
        assert!(RATIO.accepts_partial("0."));
        assert!(RATIO.accepts_partial(""));
        assert!(USERS.accepts_partial("12"));
        assert!(!USERS.accepts_partial("1.5"), "integers take no dot");
        assert!(!USERS.accepts_partial("12a"));
        assert!(!RATIO.accepts_partial("1.2.3"));
        assert!(!USERS.accepts_partial("-"), "minimum is 1, so a minus can never become valid");
        let signed = NumberSpec { min: Some(-5.0), ..USERS };
        assert!(signed.accepts_partial("-"));
        assert!(signed.accepts_partial("-3"));
    }

    #[test]
    fn partial_expressions_are_accepted() {
        assert!(USERS.accepts_partial("$"));
        assert!(USERS.accepts_partial("${"));
        assert!(USERS.accepts_partial("${thr"));
        assert!(USERS.accepts_partial("${threads}"));
        assert!(!USERS.accepts_partial("$x"));
    }

    #[test]
    fn stepping() {
        assert_eq!(USERS.step("50", 1, false), "51");
        assert_eq!(USERS.step("50", -1, false), "49");
        assert_eq!(USERS.step("50", 1, true), "60");
        assert_eq!(USERS.step("1", -1, false), "1", "clamped at the minimum");
        assert_eq!(USERS.step("999999", 1, true), "1000000", "clamped at the maximum");
        assert_eq!(RATIO.step("0.5", 1, false), "0.6");
        assert_eq!(RATIO.step("0.95", 1, false), "1");
        assert_eq!(USERS.step("", 1, false), "2", "an empty field starts from the minimum");
    }

    #[test]
    fn expressions_and_junk_are_not_stepped() {
        assert_eq!(USERS.step("${threads}", 1, false), "${threads}");
        assert_eq!(USERS.step("abc", 1, false), "abc");
    }

    #[test]
    fn formatting() {
        assert_eq!(RATIO.format(0.30000000000000004), "0.3");
        assert_eq!(RATIO.format(1.0), "1");
        assert_eq!(USERS.format(12.0), "12");
    }

    #[test]
    fn clamp_handles_open_ranges() {
        let open = NumberSpec { min: None, max: None, step: 1.0, integer: false, allow_empty: false };
        assert_eq!(open.clamp(-1e9), -1e9);
        assert_eq!(USERS.clamp(0.0), 1.0);
    }
}
