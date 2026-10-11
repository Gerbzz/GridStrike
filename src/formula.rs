use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::{CellAddr, CellError, CellInput, CellValue, Workbook};

const MAX_TOKENS: usize = 1024;
const MAX_DEPTH: usize = 64;
const MAX_RANGE: usize = 100_000;

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Comma,
    Colon,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug)]
pub(crate) enum Expr {
    Number(f64),
    Ref(CellAddr),
    BadRef,
    Range(CellAddr, CellAddr),
    UnaryMinus(Box<Expr>),
    Binary(char, Box<Expr>, Box<Expr>),
    Compare(&'static str, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

impl Expr {
    pub(crate) fn references(&self, wanted: CellAddr) -> bool {
        match self {
            Self::Ref(address) => *address == wanted,
            Self::Range(start, end) => {
                wanted.row >= start.row
                    && wanted.row <= end.row
                    && wanted.col >= start.col
                    && wanted.col <= end.col
            }
            Self::UnaryMinus(expr) => expr.references(wanted),
            Self::Binary(_, left, right) | Self::Compare(_, left, right) => {
                left.references(wanted) || right.references(wanted)
            }
            Self::Call(_, args) => args.iter().any(|arg| arg.references(wanted)),
            Self::Number(_) | Self::BadRef => false,
        }
    }
}

pub(crate) fn parse(source: &str) -> Result<Expr, CellError> {
    let text = source.strip_prefix('=').ok_or(CellError::Formula)?;
    let tokens = lex(text)?;
    let mut parser = Parser {
        tokens,
        pos: 0,
        depth: 0,
    };
    let expr = parser.comparison()?;
    if parser.pos != parser.tokens.len() {
        return Err(CellError::Formula);
    }
    Ok(expr)
}

fn lex(source: &str) -> Result<Vec<Token>, CellError> {
    if source.len() > 4096 || !source.is_ascii() {
        return Err(CellError::Formula);
    }
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        let token = match bytes[i] {
            b'0'..=b'9' | b'.' => {
                let mut dots = 0;
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    if bytes[i] == b'.' {
                        dots += 1;
                    }
                    i += 1;
                }
                if dots > 1 {
                    return Err(CellError::Formula);
                }
                let number = source[start..i]
                    .parse::<f64>()
                    .map_err(|_| CellError::Formula)?;
                if !number.is_finite() {
                    return Err(CellError::Num);
                }
                Token::Number(number)
            }
            b'A'..=b'Z' | b'a'..=b'z' => {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
                    i += 1;
                }
                Token::Ident(source[start..i].to_ascii_uppercase())
            }
            b'+' => {
                i += 1;
                Token::Plus
            }
            b'-' => {
                i += 1;
                Token::Minus
            }
            b'*' => {
                i += 1;
                Token::Star
            }
            b'/' => {
                i += 1;
                Token::Slash
            }
            b'(' => {
                i += 1;
                Token::LParen
            }
            b')' => {
                i += 1;
                Token::RParen
            }
            b',' => {
                i += 1;
                Token::Comma
            }
            b':' => {
                i += 1;
                Token::Colon
            }
            b'=' => {
                i += 1;
                Token::Eq
            }
            b'<' if bytes.get(i + 1) == Some(&b'>') => {
                i += 2;
                Token::Ne
            }
            b'<' if bytes.get(i + 1) == Some(&b'=') => {
                i += 2;
                Token::Le
            }
            b'>' if bytes.get(i + 1) == Some(&b'=') => {
                i += 2;
                Token::Ge
            }
            b'<' => {
                i += 1;
                Token::Lt
            }
            b'>' => {
                i += 1;
                Token::Gt
            }
            _ => return Err(CellError::Formula),
        };
        out.push(token);
        if out.len() > MAX_TOKENS {
            return Err(CellError::Formula);
        }
    }
    Ok(out)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }
    fn take(&mut self) -> Option<Token> {
        let token = self.peek()?.clone();
        self.pos += 1;
        Some(token)
    }
    fn consume(&mut self, token: Token) -> bool {
        if self.peek() == Some(&token) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn comparison(&mut self) -> Result<Expr, CellError> {
        let mut left = self.add()?;
        while let Some(op) = match self.peek() {
            Some(Token::Eq) => Some("="),
            Some(Token::Ne) => Some("<>"),
            Some(Token::Lt) => Some("<"),
            Some(Token::Le) => Some("<="),
            Some(Token::Gt) => Some(">"),
            Some(Token::Ge) => Some(">="),
            _ => None,
        } {
            self.take();
            left = Expr::Compare(op, Box::new(left), Box::new(self.add()?));
        }
        Ok(left)
    }

    fn add(&mut self) -> Result<Expr, CellError> {
        let mut left = self.multiply()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => '+',
                Some(Token::Minus) => '-',
                _ => break,
            };
            self.take();
            left = Expr::Binary(op, Box::new(left), Box::new(self.multiply()?));
        }
        Ok(left)
    }

    fn multiply(&mut self) -> Result<Expr, CellError> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => '*',
                Some(Token::Slash) => '/',
                _ => break,
            };
            self.take();
            left = Expr::Binary(op, Box::new(left), Box::new(self.unary()?));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, CellError> {
        if self.consume(Token::Minus) {
            return Ok(Expr::UnaryMinus(Box::new(self.unary()?)));
        }
        if self.consume(Token::Plus) {
            return self.unary();
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, CellError> {
        if self.depth >= MAX_DEPTH {
            return Err(CellError::Formula);
        }
        match self.take().ok_or(CellError::Formula)? {
            Token::Number(number) => Ok(Expr::Number(number)),
            Token::LParen => {
                self.depth += 1;
                let expr = self.comparison()?;
                self.depth -= 1;
                if !self.consume(Token::RParen) {
                    return Err(CellError::Formula);
                }
                Ok(expr)
            }
            Token::Ident(ident) if self.consume(Token::LParen) => {
                self.depth += 1;
                let mut args = Vec::new();
                if !self.consume(Token::RParen) {
                    loop {
                        args.push(self.comparison()?);
                        if args.len() > 64 {
                            return Err(CellError::Formula);
                        }
                        if self.consume(Token::RParen) {
                            break;
                        }
                        if !self.consume(Token::Comma) {
                            return Err(CellError::Formula);
                        }
                    }
                }
                self.depth -= 1;
                Ok(Expr::Call(ident, args))
            }
            Token::Ident(ident) => {
                let reference = CellAddr::parse(&ident);
                if self.consume(Token::Colon) {
                    let Token::Ident(last) = self.take().ok_or(CellError::Formula)? else {
                        return Err(CellError::Formula);
                    };
                    let end = CellAddr::parse(&last);
                    match (reference, end) {
                        (Ok(start), Ok(end)) => Ok(Expr::Range(start, end)),
                        _ => Ok(Expr::BadRef),
                    }
                } else {
                    match reference {
                        Ok(address) => Ok(Expr::Ref(address)),
                        Err(_) if ident.chars().any(|c| c.is_ascii_digit()) => Ok(Expr::BadRef),
                        Err(_) => Err(CellError::Formula),
                    }
                }
            }
            _ => Err(CellError::Formula),
        }
    }
}

pub(crate) struct Evaluator<'a> {
    book: &'a Workbook,
    sheet: Uuid,
    cache: BTreeMap<CellAddr, CellValue>,
    stack: Vec<CellAddr>,
    cycle: BTreeSet<CellAddr>,
}

impl<'a> Evaluator<'a> {
    pub(crate) fn new(book: &'a Workbook, sheet: Uuid) -> Self {
        Self {
            book,
            sheet,
            cache: BTreeMap::new(),
            stack: Vec::new(),
            cycle: BTreeSet::new(),
        }
    }

    pub(crate) fn cell(&mut self, address: CellAddr) -> CellValue {
        if address.validate().is_err() {
            return CellValue::Error(CellError::Ref);
        }
        if let Some(value) = self.cache.get(&address) {
            return value.clone();
        }
        if let Some(start) = self.stack.iter().position(|at| *at == address) {
            self.cycle.extend(self.stack[start..].iter().copied());
            return CellValue::Error(CellError::Cycle);
        }
        if self.stack.len() >= MAX_DEPTH {
            return CellValue::Error(CellError::Formula);
        }
        self.stack.push(address);
        let input = self
            .book
            .cell(self.sheet, address)
            .cloned()
            .unwrap_or(CellInput::Empty);
        let mut value = match input {
            CellInput::Empty => CellValue::Empty,
            CellInput::Text(text) => CellValue::Text(text),
            CellInput::Number(number) if number.is_finite() => CellValue::Number(number),
            CellInput::Number(_) => CellValue::Error(CellError::Num),
            CellInput::Formula(source) => match parse(&source) {
                Ok(expr) => self.expr(&expr),
                Err(error) => CellValue::Error(error),
            },
        };
        self.stack.pop();
        if self.cycle.contains(&address) {
            value = CellValue::Error(CellError::Cycle);
        }
        self.cache.insert(address, value.clone());
        value
    }

    fn expr(&mut self, expr: &Expr) -> CellValue {
        match expr {
            Expr::Number(number) => CellValue::Number(*number),
            Expr::Ref(address) => self.cell(*address),
            Expr::BadRef => CellValue::Error(CellError::Ref),
            Expr::Range(_, _) => CellValue::Error(CellError::Formula),
            Expr::UnaryMinus(inner) => match numeric(self.expr(inner)) {
                Ok(number) => finite(-number),
                Err(error) => CellValue::Error(error),
            },
            Expr::Binary(op, left, right) => {
                let a = numeric(self.expr(left));
                let b = numeric(self.expr(right));
                match (a, b) {
                    (Ok(a), Ok(b)) => match op {
                        '+' => finite(a + b),
                        '-' => finite(a - b),
                        '*' => finite(a * b),
                        '/' if b == 0.0 => CellValue::Error(CellError::DivZero),
                        '/' => finite(a / b),
                        _ => CellValue::Error(CellError::Formula),
                    },
                    (Err(error), _) | (_, Err(error)) => CellValue::Error(error),
                }
            }
            Expr::Compare(op, left, right) => {
                let a = numeric(self.expr(left));
                let b = numeric(self.expr(right));
                match (a, b) {
                    (Ok(a), Ok(b)) => CellValue::Number(
                        if match *op {
                            "=" => a == b,
                            "<>" => a != b,
                            "<" => a < b,
                            "<=" => a <= b,
                            ">" => a > b,
                            ">=" => a >= b,
                            _ => false,
                        } {
                            1.0
                        } else {
                            0.0
                        },
                    ),
                    (Err(error), _) | (_, Err(error)) => CellValue::Error(error),
                }
            }
            Expr::Call(name, args) => self.call(name, args),
        }
    }

    fn call(&mut self, name: &str, args: &[Expr]) -> CellValue {
        if name == "IF" {
            if args.len() != 3 {
                return CellValue::Error(CellError::Formula);
            }
            return match numeric(self.expr(&args[0])) {
                Ok(test) => self.expr(&args[if test != 0.0 { 1 } else { 2 }]),
                Err(error) => CellValue::Error(error),
            };
        }
        if !["SUM", "AVERAGE", "MIN", "MAX", "COUNT"].contains(&name) || args.is_empty() {
            return CellValue::Error(CellError::Formula);
        }
        let mut numbers = Vec::new();
        for arg in args {
            let values = match arg {
                Expr::Range(start, end) => match self.range(*start, *end) {
                    Ok(values) => values,
                    Err(error) => return CellValue::Error(error),
                },
                _ => vec![self.expr(arg)],
            };
            for value in values {
                match value {
                    CellValue::Number(number) => numbers.push(number),
                    CellValue::Error(error) => return CellValue::Error(error),
                    CellValue::Text(_) | CellValue::Empty => {}
                }
            }
        }
        let result = match name {
            "SUM" => numbers.iter().sum::<f64>(),
            "COUNT" => numbers.len() as f64,
            "AVERAGE" if numbers.is_empty() => return CellValue::Error(CellError::DivZero),
            "AVERAGE" => numbers.iter().sum::<f64>() / numbers.len() as f64,
            "MIN" => numbers.into_iter().reduce(f64::min).unwrap_or(0.0),
            "MAX" => numbers.into_iter().reduce(f64::max).unwrap_or(0.0),
            _ => return CellValue::Error(CellError::Formula),
        };
        finite(result)
    }

    fn range(&mut self, start: CellAddr, end: CellAddr) -> Result<Vec<CellValue>, CellError> {
        if start.row > end.row || start.col > end.col {
            return Err(CellError::Ref);
        }
        let count = (u64::from(end.row - start.row) + 1) * (u64::from(end.col - start.col) + 1);
        if count > MAX_RANGE as u64 {
            return Err(CellError::Ref);
        }
        let mut values = Vec::with_capacity(count as usize);
        for row in start.row..=end.row {
            for col in start.col..=end.col {
                values.push(self.cell(CellAddr { row, col }));
            }
        }
        Ok(values)
    }
}

fn numeric(value: CellValue) -> Result<f64, CellError> {
    match value {
        CellValue::Number(number) => Ok(number),
        CellValue::Empty => Ok(0.0),
        CellValue::Text(_) => Err(CellError::Formula),
        CellValue::Error(error) => Err(error),
    }
}

fn finite(number: f64) -> CellValue {
    if number.is_finite() {
        CellValue::Number(number)
    } else {
        CellValue::Error(CellError::Num)
    }
}
