use crate::Extent2D;
use crate::shader::{CompileError, FragmentProgram};
use std::collections::BTreeMap;

const MAX_FORMULA_BYTES: usize = 128;

#[derive(Clone, Copy, Debug)]
enum Term {
    Constant(f64),
    Uniform(usize),
    Width,
    Height,
    Add,
    Subtract,
    Multiply,
    Divide,
    Negate,
}

#[derive(Debug)]
pub(in crate::shader) struct Dimension(Box<[Term]>);

#[derive(Clone, Copy)]
pub(in crate::shader) enum ScalarInput {
    Constant(f64),
    Uniform(usize),
}

impl Dimension {
    pub(in crate::shader) fn parse(formula: &str, inputs: &BTreeMap<&str, ScalarInput>) -> Result<Self, CompileError> {
        let error = || CompileError::Invalid(format!("Invalid ISF pass dimension '{formula}'"));
        if formula.len() > MAX_FORMULA_BYTES || !formula.is_ascii() {
            return Err(error());
        }
        let mut parser = Parser {
            remaining: formula,
            terms: Vec::new(),
            inputs,
        };
        parser.expression(0).ok_or_else(error)?;
        if !parser.remaining.trim().is_empty() {
            return Err(error());
        }
        Ok(Self(parser.terms.into_boxed_slice()))
    }

    pub(super) fn evaluate(&self, canvas: Extent2D, program: &FragmentProgram) -> Result<u32, CompileError> {
        let mut stack = [0.0; MAX_FORMULA_BYTES];
        let mut depth = 0;
        for term in &self.0 {
            let value = match *term {
                Term::Constant(value) => value,
                Term::Uniform(slot) => program.uniform_scalar(slot).expect("validated scalar shader input"),
                Term::Width => f64::from(canvas.width()),
                Term::Height => f64::from(canvas.height()),
                Term::Negate => {
                    stack[depth - 1] = -stack[depth - 1];
                    continue;
                }
                op => {
                    let right = stack[depth - 1];
                    let left = &mut stack[depth - 2];
                    *left = match op {
                        Term::Add => *left + right,
                        Term::Subtract => *left - right,
                        Term::Multiply => *left * right,
                        Term::Divide => *left / right,
                        _ => unreachable!("only binary operators reach this arm"),
                    };
                    depth -= 1;
                    continue;
                }
            };
            stack[depth] = value;
            depth += 1;
        }
        let value = stack[0];
        if !value.is_finite() || value <= 0.0 || value > f64::from(u32::MAX) {
            return Err(CompileError::Invalid(
                "ISF pass dimension is outside the positive pixel range".into(),
            ));
        }
        Ok(value.floor().max(1.0) as u32)
    }
}

struct Parser<'a> {
    remaining: &'a str,
    terms: Vec<Term>,
    inputs: &'a BTreeMap<&'a str, ScalarInput>,
}

impl<'a> Parser<'a> {
    fn expression(&mut self, minimum: u8) -> Option<()> {
        self.atom()?;
        loop {
            let (term, precedence) = match self.peek() {
                Some(b'+') => (Term::Add, 1),
                Some(b'-') => (Term::Subtract, 1),
                Some(b'*') => (Term::Multiply, 2),
                Some(b'/') => (Term::Divide, 2),
                _ => return Some(()),
            };
            if precedence < minimum {
                return Some(());
            }
            self.advance();
            self.expression(precedence + 1)?;
            self.terms.push(term);
        }
    }

    fn atom(&mut self) -> Option<()> {
        let term = match self.peek()? {
            b'(' => {
                self.advance();
                self.expression(0)?;
                if self.peek()? != b')' {
                    return None;
                }
                self.advance();
                return Some(());
            }
            b'-' => {
                self.advance();
                self.atom()?;
                Term::Negate
            }
            b'+' => {
                self.advance();
                return self.atom();
            }
            b'$' => {
                self.advance();
                match self.take(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
                    "WIDTH" => Term::Width,
                    "HEIGHT" => Term::Height,
                    name => match *self.inputs.get(name)? {
                        ScalarInput::Constant(value) => Term::Constant(value),
                        ScalarInput::Uniform(slot) => Term::Uniform(slot),
                    },
                }
            }
            b'0'..=b'9' | b'.' => {
                let sign_after_exponent = |input: &str| input.ends_with(['e', 'E']);
                let start = self.remaining;
                let mut number = self.take(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E'));
                if sign_after_exponent(number) && matches!(self.remaining.as_bytes().first(), Some(b'+' | b'-')) {
                    self.advance();
                    self.take(|byte| byte.is_ascii_digit());
                    number = &start[..start.len() - self.remaining.len()];
                }
                Term::Constant(number.parse::<f64>().ok().filter(|value| value.is_finite())?)
            }
            _ => return None,
        };
        self.terms.push(term);
        Some(())
    }

    fn peek(&mut self) -> Option<u8> {
        self.remaining = self.remaining.trim_start();
        self.remaining.as_bytes().first().copied()
    }

    fn advance(&mut self) {
        self.remaining = &self.remaining[1..];
    }

    fn take(&mut self, predicate: impl Fn(u8) -> bool) -> &'a str {
        let end = self.remaining.bytes().take_while(|&byte| predicate(byte)).count();
        let (token, remaining) = self.remaining.split_at(end);
        self.remaining = remaining;
        token
    }
}
