//! Порт nth-check 2.1.1 (`parse` + `compile`). Числа — `f64`, как в JS.

/// Скомпилированная проверка индекса (индекс с нуля) или константы boolbase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Nth {
    False,
    True,
    Le(f64),
    Eq(f64),
    Ge(f64),
    ModGe { b: f64, abs_a: f64, b_mod: f64 },
    ModLe { b: f64, abs_a: f64, b_mod: f64 },
}

impl Nth {
    #[inline]
    pub(crate) fn test(self, index: usize) -> bool {
        let index = index as f64;
        match self {
            Nth::False => false,
            Nth::True => true,
            Nth::Le(b) => index <= b,
            Nth::Eq(b) => index == b,
            Nth::Ge(b) => index >= b,
            Nth::ModGe { b, abs_a, b_mod } => index >= b && index % abs_a == b_mod,
            Nth::ModLe { b, abs_a, b_mod } => index <= b && index % abs_a == b_mod,
        }
    }
}

pub(crate) fn parse(formula: &str) -> Result<(f64, f64), String> {
    let formula = dom::js::trim(formula).to_lowercase();
    if formula == "even" {
        return Ok((2.0, 0.0));
    }
    if formula == "odd" {
        return Ok((2.0, 1.0));
    }
    let s: Vec<char> = formula.chars().collect();
    let mut idx = 0;
    let read_sign = |idx: &mut usize| -> f64 {
        match s.get(*idx) {
            Some('-') => {
                *idx += 1;
                -1.0
            }
            Some('+') => {
                *idx += 1;
                1.0
            }
            _ => 1.0,
        }
    };
    let read_number = |idx: &mut usize| -> Option<f64> {
        let start = *idx;
        let mut value = 0.0f64;
        while let Some(d) = s.get(*idx).and_then(|c| c.to_digit(10)) {
            value = value * 10.0 + d as f64;
            *idx += 1;
        }
        (*idx != start).then_some(value)
    };
    let skip_ws = |idx: &mut usize| {
        while matches!(s.get(*idx), Some('\t' | '\n' | '\u{0C}' | '\r' | ' ')) {
            *idx += 1;
        }
    };

    let mut a = 0.0;
    let mut sign = read_sign(&mut idx);
    let mut number = read_number(&mut idx);
    if s.get(idx) == Some(&'n') {
        idx += 1;
        a = sign * number.unwrap_or(1.0);
        skip_ws(&mut idx);
        if idx < s.len() {
            sign = read_sign(&mut idx);
            skip_ws(&mut idx);
            number = read_number(&mut idx);
        } else {
            sign = 0.0;
            number = Some(0.0);
        }
    }
    match number {
        Some(number) if idx >= s.len() => Ok((a, sign * number)),
        _ => Err(format!("n-th rule couldn't be parsed ('{formula}')")),
    }
}

pub(crate) fn compile((a, b): (f64, f64)) -> Nth {
    let b = b - 1.0;
    if b < 0.0 && a <= 0.0 {
        return Nth::False;
    }
    if a == -1.0 {
        return Nth::Le(b);
    }
    if a == 0.0 {
        return Nth::Eq(b);
    }
    if a == 1.0 {
        return if b < 0.0 { Nth::True } else { Nth::Ge(b) };
    }
    let abs_a = a.abs();
    let b_mod = ((b % abs_a) + abs_a) % abs_a;
    if a > 1.0 {
        Nth::ModGe { b, abs_a, b_mod }
    } else {
        Nth::ModLe { b, abs_a, b_mod }
    }
}

pub(crate) fn nth_check(formula: &str) -> Result<Nth, String> {
    parse(formula).map(compile)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(formula: &str) -> Vec<usize> {
        let f = nth_check(formula).unwrap();
        (0..12).filter(|&i| f.test(i)).collect()
    }

    #[test]
    fn formulas() {
        assert_eq!(matches("odd"), [0, 2, 4, 6, 8, 10]);
        assert_eq!(matches(" 2n + 1 "), [0, 2, 4, 6, 8, 10]);
        assert_eq!(matches("-n+3"), [0, 1, 2]);
        assert_eq!(matches("3"), [2]);
        assert_eq!(matches("-2n+5"), [0, 2, 4]);
        assert_eq!(nth_check("n"), Ok(Nth::True));
        assert_eq!(nth_check("-n"), Ok(Nth::False));
        assert_eq!(
            nth_check("2n+"),
            Err("n-th rule couldn't be parsed ('2n+')".to_owned())
        );
        assert!(nth_check("+ 2n").is_err());
    }
}
