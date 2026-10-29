//! A meter's own bill formula: `(показание - предыдущее) * 3.5 * 10`.
//!
//! Decimal only (money rule), never panics: a wrong formula, an unknown name
//! or a division by zero is a [`FormulaIssue`] with the place it happened.
//! Numbers take `.` or `,`; function arguments are split by `;`.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::Amount;

/// What is wrong with a formula, worded for the form, and where (1-based character).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct FormulaIssue {
    pub position: u32,
    pub text_ru: String,
    pub text_en: String,
}

impl FormulaIssue {
    fn at(position: usize, ru: impl Into<String>, en: impl Into<String>) -> Self {
        Self {
            position: u32::try_from(position + 1).unwrap_or(u32::MAX),
            text_ru: ru.into(),
            text_en: en.into(),
        }
    }
}

/// The names a formula reads, Russian first, then the Latin synonym.
pub const VARIABLES: [(&str, &str); 5] = [
    ("показание", "current"),
    ("предыдущее", "previous"),
    ("расход", "used"),
    ("тариф", "tariff"),
    ("дней", "days"),
];

/// One meter zone (or the whole meter): where it stood and what it used.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ZoneValues {
    pub current: Amount,
    pub previous: Amount,
}

/// What a formula is given: the whole meter, its zones by name, the tariff and the days.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormulaInput {
    pub total: ZoneValues,
    pub zones: Vec<(String, ZoneValues)>,
    pub tariff: Amount,
    pub days: Amount,
}

/// A zone's name as it goes into a variable: "peak night" → "peak_night" (Russian zone names).
pub fn zone_key(zone: &str) -> String {
    zone.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
}

impl FormulaInput {
    /// Every variable the formula may read, Russian and Latin names alike.
    pub fn variables(&self) -> BTreeMap<String, Decimal> {
        let mut out = BTreeMap::new();
        let mut put = |suffix: &str, values: &ZoneValues| {
            let used = values.current - values.previous;
            for ((ru, en), value) in VARIABLES.iter().zip([
                values.current,
                values.previous,
                used,
                self.tariff,
                self.days,
            ]) {
                out.insert(format!("{ru}{suffix}"), value.into_inner());
                out.insert(format!("{en}{suffix}"), value.into_inner());
            }
        };
        put("", &self.total);
        for (zone, values) in &self.zones {
            put(&format!("_{}", zone_key(zone)), values);
        }
        out
    }
}

/// Check a formula against the names it may read, without numbers.
pub fn check(formula: &str, zones: &[String]) -> Result<(), FormulaIssue> {
    let input = FormulaInput {
        zones: zones
            .iter()
            .map(|zone| (zone.clone(), ZoneValues::default()))
            .collect(),
        ..FormulaInput::default()
    };
    let expr = parse(formula)?;
    expr.names(&input.variables())
}

/// The bill by a formula, rounded to kopecks.
pub fn evaluate(formula: &str, input: &FormulaInput) -> Result<Amount, FormulaIssue> {
    let expr = parse(formula)?;
    let vars = input.variables();
    expr.names(&vars)?;
    Ok(Amount::new(expr.eval(&vars)?).round(2))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Gt,
    Lt,
    Ge,
    Le,
    Eq,
    Ne,
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Number(Decimal),
    Name(String, usize),
    Neg(Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>, usize),
    Call(String, Vec<Expr>, usize),
}

const FUNCTIONS: [(&str, &str); 4] = [
    ("min", "мин"),
    ("max", "макс"),
    ("round", "округл"),
    ("if", "если"),
];

fn function(name: &str) -> Option<&'static str> {
    FUNCTIONS
        .iter()
        .find(|(en, ru)| *en == name || *ru == name)
        .map(|(en, _)| *en)
}

fn bool_of(value: bool) -> Decimal {
    if value { Decimal::ONE } else { Decimal::ZERO }
}

impl Expr {
    /// Every name is a variable or a function with the right number of arguments.
    fn names(&self, vars: &BTreeMap<String, Decimal>) -> Result<(), FormulaIssue> {
        match self {
            Expr::Number(_) => Ok(()),
            Expr::Name(name, _) if vars.contains_key(name) => Ok(()),
            Expr::Name(name, at) => Err(FormulaIssue::at(
                *at,
                format!("неизвестное имя «{name}»"),
                format!("unknown name «{name}»"),
            )),
            Expr::Neg(inner) => inner.names(vars),
            Expr::Binary(_, left, right, _) => {
                left.names(vars)?;
                right.names(vars)
            }
            Expr::Call(name, args, at) => {
                let fits = match function(name) {
                    Some("min" | "max") => !args.is_empty(),
                    Some("round") => (1..=2).contains(&args.len()),
                    Some(_) => args.len() == 3,
                    None => {
                        return Err(FormulaIssue::at(
                            *at,
                            format!("неизвестная функция «{name}»"),
                            format!("unknown function «{name}»"),
                        ));
                    }
                };
                if !fits {
                    return Err(FormulaIssue::at(
                        *at,
                        format!("у «{name}» не то число значений"),
                        format!("«{name}» takes another number of values"),
                    ));
                }
                args.iter().try_for_each(|arg| arg.names(vars))
            }
        }
    }

    fn eval(&self, vars: &BTreeMap<String, Decimal>) -> Result<Decimal, FormulaIssue> {
        let too_big =
            |at: usize| FormulaIssue::at(at, "слишком большое число", "the number is too big");
        match self {
            Expr::Number(value) => Ok(*value),
            Expr::Name(name, at) => vars.get(name).copied().ok_or_else(|| {
                FormulaIssue::at(
                    *at,
                    format!("неизвестное имя «{name}»"),
                    format!("unknown name «{name}»"),
                )
            }),
            Expr::Neg(inner) => Ok(-inner.eval(vars)?),
            Expr::Binary(op, left, right, at) => {
                let (left, right) = (left.eval(vars)?, right.eval(vars)?);
                match op {
                    Op::Add => left.checked_add(right).ok_or_else(|| too_big(*at)),
                    Op::Sub => left.checked_sub(right).ok_or_else(|| too_big(*at)),
                    Op::Mul => left.checked_mul(right).ok_or_else(|| too_big(*at)),
                    Op::Div if right.is_zero() => {
                        Err(FormulaIssue::at(*at, "деление на ноль", "division by zero"))
                    }
                    Op::Div => left.checked_div(right).ok_or_else(|| too_big(*at)),
                    Op::Gt => Ok(bool_of(left > right)),
                    Op::Lt => Ok(bool_of(left < right)),
                    Op::Ge => Ok(bool_of(left >= right)),
                    Op::Le => Ok(bool_of(left <= right)),
                    Op::Eq => Ok(bool_of(left == right)),
                    Op::Ne => Ok(bool_of(left != right)),
                }
            }
            Expr::Call(name, args, at) => match function(name) {
                Some("if") => {
                    let [condition, yes, no] = args.as_slice() else {
                        return Err(FormulaIssue::at(
                            *at,
                            "«если» ждёт три значения",
                            "«if» takes three values",
                        ));
                    };
                    if condition.eval(vars)?.is_zero() {
                        no.eval(vars)
                    } else {
                        yes.eval(vars)
                    }
                }
                Some("round") => {
                    let value = args
                        .first()
                        .map(|arg| arg.eval(vars))
                        .transpose()?
                        .unwrap_or_default();
                    let digits = match args.get(1) {
                        Some(arg) => arg.eval(vars)?,
                        None => Decimal::ZERO,
                    };
                    let digits = u32::try_from(digits.trunc().clamp(Decimal::ZERO, Decimal::TEN))
                        .unwrap_or(0);
                    Ok(value.round_dp(digits))
                }
                Some(which) => {
                    let mut values = args.iter().map(|arg| arg.eval(vars));
                    let first = values.next().transpose()?.unwrap_or_default();
                    values.try_fold(first, |best, next| {
                        let next = next?;
                        Ok(if (which == "min") == (next < best) {
                            next
                        } else {
                            best
                        })
                    })
                }
                None => Err(FormulaIssue::at(
                    *at,
                    format!("неизвестная функция «{name}»"),
                    format!("unknown function «{name}»"),
                )),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(Decimal),
    Name(String),
    Op(Op),
    Open,
    Close,
    Semicolon,
}

fn tokens(source: &str) -> Result<Vec<(Token, usize)>, FormulaIssue> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut index = 0;
    while let Some(&current) = chars.get(index) {
        let start = index;
        let next = chars.get(index + 1).copied();
        let token = match current {
            _ if current.is_whitespace() => {
                index += 1;
                continue;
            }
            '0'..='9' => {
                let mut text = String::new();
                while let Some(&digit) = chars.get(index) {
                    let separator = matches!(digit, '.' | ',')
                        && chars.get(index + 1).is_some_and(char::is_ascii_digit);
                    if digit.is_ascii_digit() {
                        text.push(digit);
                    } else if separator && !text.contains('.') {
                        text.push('.');
                    } else {
                        break;
                    }
                    index += 1;
                }
                let value = text.parse::<Decimal>().map_err(|_| {
                    FormulaIssue::at(
                        start,
                        format!("не число «{text}»"),
                        format!("not a number «{text}»"),
                    )
                })?;
                out.push((Token::Number(value), start));
                continue;
            }
            _ if current.is_alphabetic() || current == '_' => {
                let mut text = String::new();
                while let Some(&letter) = chars.get(index) {
                    if !(letter.is_alphanumeric() || letter == '_') {
                        break;
                    }
                    text.extend(letter.to_lowercase());
                    index += 1;
                }
                out.push((Token::Name(text.replace('ё', "е")), start));
                continue;
            }
            '+' => Token::Op(Op::Add),
            '-' | '−' => Token::Op(Op::Sub),
            '*' | '×' => Token::Op(Op::Mul),
            '/' | ':' => Token::Op(Op::Div),
            '(' => Token::Open,
            ')' => Token::Close,
            ';' => Token::Semicolon,
            '>' if next == Some('=') => Token::Op(Op::Ge),
            '<' if next == Some('=') => Token::Op(Op::Le),
            '<' if next == Some('>') => Token::Op(Op::Ne),
            '!' if next == Some('=') => Token::Op(Op::Ne),
            '>' => Token::Op(Op::Gt),
            '<' => Token::Op(Op::Lt),
            '=' => Token::Op(Op::Eq),
            other => {
                return Err(FormulaIssue::at(
                    start,
                    format!("непонятный знак «{other}»"),
                    format!("unexpected sign «{other}»"),
                ));
            }
        };
        let wide = matches!(token, Token::Op(Op::Ge | Op::Le | Op::Ne));
        index += if wide { 2 } else { 1 };
        out.push((token, start));
    }
    Ok(out)
}

struct Parser {
    tokens: Vec<(Token, usize)>,
    index: usize,
    end: usize,
}

const MAX_DEPTH: usize = 64;

/// Parse a formula into an expression tree.
fn parse(source: &str) -> Result<Expr, FormulaIssue> {
    if source.trim().is_empty() {
        return Err(FormulaIssue::at(
            0,
            "формула пустая",
            "the formula is empty",
        ));
    }
    let mut parser = Parser {
        tokens: tokens(source)?,
        index: 0,
        end: source.chars().count(),
    };
    let expr = parser.comparison(0)?;
    match parser.peek() {
        None => Ok(expr),
        Some((Token::Close, at)) => Err(FormulaIssue::at(at, "лишняя «)»", "an extra «)»")),
        Some((_, at)) => Err(FormulaIssue::at(
            at,
            "здесь не хватает знака действия",
            "a sign is missing here",
        )),
    }
}

impl Parser {
    fn peek(&self) -> Option<(Token, usize)> {
        self.tokens.get(self.index).cloned()
    }

    fn here(&self) -> usize {
        self.peek().map_or(self.end, |(_, at)| at)
    }

    fn take_op(&mut self, ops: &[Op]) -> Option<(Op, usize)> {
        match self.peek() {
            Some((Token::Op(op), at)) if ops.contains(&op) => {
                self.index += 1;
                Some((op, at))
            }
            _ => None,
        }
    }

    fn deep(&self, depth: usize) -> Result<(), FormulaIssue> {
        if depth > MAX_DEPTH {
            return Err(FormulaIssue::at(
                self.here(),
                "слишком глубокие скобки",
                "brackets nest too deep",
            ));
        }
        Ok(())
    }

    fn comparison(&mut self, depth: usize) -> Result<Expr, FormulaIssue> {
        self.deep(depth)?;
        let left = self.sum(depth)?;
        match self.take_op(&[Op::Gt, Op::Lt, Op::Ge, Op::Le, Op::Eq, Op::Ne]) {
            Some((op, at)) => Ok(Expr::Binary(
                op,
                Box::new(left),
                Box::new(self.sum(depth)?),
                at,
            )),
            None => Ok(left),
        }
    }

    fn sum(&mut self, depth: usize) -> Result<Expr, FormulaIssue> {
        let mut left = self.product(depth)?;
        while let Some((op, at)) = self.take_op(&[Op::Add, Op::Sub]) {
            left = Expr::Binary(op, Box::new(left), Box::new(self.product(depth)?), at);
        }
        Ok(left)
    }

    fn product(&mut self, depth: usize) -> Result<Expr, FormulaIssue> {
        let mut left = self.unary(depth)?;
        while let Some((op, at)) = self.take_op(&[Op::Mul, Op::Div]) {
            left = Expr::Binary(op, Box::new(left), Box::new(self.unary(depth)?), at);
        }
        Ok(left)
    }

    fn unary(&mut self, depth: usize) -> Result<Expr, FormulaIssue> {
        self.deep(depth)?;
        if self.take_op(&[Op::Sub]).is_some() {
            return Ok(Expr::Neg(Box::new(self.unary(depth + 1)?)));
        }
        if self.take_op(&[Op::Add]).is_some() {
            return self.unary(depth + 1);
        }
        self.primary(depth)
    }

    fn close(&mut self, open_at: usize) -> Result<(), FormulaIssue> {
        match self.peek() {
            Some((Token::Close, _)) => {
                self.index += 1;
                Ok(())
            }
            _ => Err(FormulaIssue::at(
                open_at,
                "скобка не закрыта",
                "the bracket is not closed",
            )),
        }
    }

    fn primary(&mut self, depth: usize) -> Result<Expr, FormulaIssue> {
        let Some((token, at)) = self.peek() else {
            return Err(FormulaIssue::at(
                self.end,
                "формула оборвалась",
                "the formula ends too early",
            ));
        };
        self.index += 1;
        match token {
            Token::Number(value) => Ok(Expr::Number(value)),
            Token::Open => {
                let inner = self.comparison(depth + 1)?;
                self.close(at)?;
                Ok(inner)
            }
            Token::Name(name) => {
                if !matches!(self.peek(), Some((Token::Open, _))) {
                    return Ok(Expr::Name(name, at));
                }
                let open_at = self.here();
                self.index += 1;
                let mut args = Vec::new();
                if !matches!(self.peek(), Some((Token::Close, _))) {
                    args.push(self.comparison(depth + 1)?);
                    while matches!(self.peek(), Some((Token::Semicolon, _))) {
                        self.index += 1;
                        args.push(self.comparison(depth + 1)?);
                    }
                }
                self.close(open_at)?;
                Ok(Expr::Call(name, args, at))
            }
            Token::Close => Err(FormulaIssue::at(at, "лишняя «)»", "an extra «)»")),
            Token::Semicolon => Err(FormulaIssue::at(
                at,
                "«;» разделяет значения функции — например, мин(расход; 150)",
                "«;» splits a function's values, like min(used; 150)",
            )),
            Token::Op(_) => Err(FormulaIssue::at(
                at,
                "здесь ждали число или имя",
                "a number or a name was expected here",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amount(raw: &str) -> Amount {
        Amount::parse(raw).unwrap()
    }

    fn meter(previous: &str, current: &str, tariff: &str) -> FormulaInput {
        FormulaInput {
            total: ZoneValues {
                current: amount(current),
                previous: amount(previous),
            },
            zones: Vec::new(),
            tariff: amount(tariff),
            days: amount("30"),
        }
    }

    fn calc(formula: &str, input: &FormulaInput) -> Amount {
        evaluate(formula, input).unwrap_or_else(|issue| panic!("{formula}: {issue:?}"))
    }

    #[test]
    fn the_users_own_formula() {
        let input = meter("1200", "1345", "0");
        assert_eq!(
            calc("(показание - предыдущее) * 3.5 * 10", &input),
            amount("5075")
        );
        assert_eq!(
            calc("(Показание − предыдущее) × 3,5 × 10", &input),
            amount("5075")
        );
        assert_eq!(calc("used * 35", &input), amount("5075"));
    }

    #[test]
    fn precedence_and_signs() {
        let input = meter("0", "0", "0");
        assert_eq!(calc("2 + 3 * 4", &input), amount("14"));
        assert_eq!(calc("(2 + 3) * 4", &input), amount("20"));
        assert_eq!(calc("-2 * -3", &input), amount("6"));
        assert_eq!(calc("10 / 4", &input), amount("2.5"));
        assert_eq!(calc("1 / 3", &input), amount("0.33"));
    }

    #[test]
    fn tiers_fee_and_conditions() {
        let tiers = "мин(расход; 150) * тариф + макс(расход - 150; 0) * 7,1 + 120";
        assert_eq!(calc(tiers, &meter("0", "100", "5")), amount("620"));
        assert_eq!(calc(tiers, &meter("0", "200", "5")), amount("1225"));
        let gate = "если(расход > 100; расход * 2; расход)";
        assert_eq!(calc(gate, &meter("0", "50", "0")), amount("50"));
        assert_eq!(calc(gate, &meter("0", "150", "0")), amount("300"));
        assert_eq!(
            calc("округл(10 / 3; 1)", &meter("0", "0", "0")),
            amount("3.3")
        );
        assert_eq!(calc("round(2.5)", &meter("0", "0", "0")), amount("2"));
        assert_eq!(calc("дней * 4", &meter("0", "0", "0")), amount("120"));
    }

    #[test]
    fn zones_have_their_own_names() {
        let input = FormulaInput {
            total: ZoneValues {
                current: amount("1300"),
                previous: amount("1000"),
            },
            zones: vec![
                (
                    "День".into(),
                    ZoneValues {
                        current: amount("800"),
                        previous: amount("600"),
                    },
                ),
                (
                    "ночь".into(),
                    ZoneValues {
                        current: amount("500"),
                        previous: amount("400"),
                    },
                ),
            ],
            tariff: amount("0"),
            days: amount("30"),
        };
        assert_eq!(
            calc("расход_день * 6 + расход_ночь * 3", &input),
            amount("1500")
        );
        assert_eq!(
            calc("used_день * 6 + used_ночь * 3", &input),
            amount("1500")
        );
        assert_eq!(calc("расход", &input), amount("300"));
        assert!(check("расход_день + расход_вечер", &["день".into()]).is_err());
    }

    #[test]
    fn mistakes_say_where() {
        let issue = check("расход * х", &[]).unwrap_err();
        assert_eq!(issue.position, 10);
        assert!(issue.text_ru.contains("«х»"), "{issue:?}");
        assert_eq!(check("(расход * 2", &[]).unwrap_err().position, 1);
        assert_eq!(check("расход 2", &[]).unwrap_err().position, 8);
        assert!(check("", &[]).is_err());
        assert!(check("расход * ", &[]).is_err());
        assert!(check("мин()", &[]).is_err());
        assert!(check("корень(4)", &[]).is_err());
        assert!(check("2 # 3", &[]).is_err());
        assert!(check("1;2", &[]).is_err());
        assert!(check(&"(".repeat(200), &[]).is_err());
    }

    #[test]
    fn division_by_zero_is_an_issue_not_a_panic() {
        let issue = evaluate("расход / (тариф - тариф)", &meter("0", "10", "5")).unwrap_err();
        assert!(issue.text_en.contains("zero"), "{issue:?}");
        assert!(
            evaluate(
                "99999999999999999999 * 99999999999999999999",
                &meter("0", "0", "0")
            )
            .is_err()
        );
    }
}
