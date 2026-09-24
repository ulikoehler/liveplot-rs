//! Formula expressions for math traces: lexer, parser, AST and evaluator.
//!
//! Formulas are free-text math expressions evaluated per-sample on the union of
//! the referenced traces' timestamps. Trace references are written in braces
//! (`{trace name}`) so names may contain spaces or other special characters.
//! Bare words are reserved for the builtins `t` (sample timestamp), `pi`, `e`
//! and the function names listed in [`Func`].
//!
//! Supported syntax:
//! - Binary operators `+ - * / ^` and parentheses. `^` is right-associative and
//!   binds tighter than unary minus (`-x^2 == -(x^2)`).
//! - Implicit multiplication: `2{t}`, `{a}{b}`, `2(t+1)`, `(a)(b)`.
//! - Function calls `name(expr, ...)`: sqrt, root(x,n), exp, sin, cos, tan,
//!   asin, acos, atan, atan2(y,x), sinh, cosh, tanh, asinh, acosh, atanh,
//!   abs, ln, log10, log2, log(x) [=log10] and log(x,b) [arbitrary base].
//!
//! Evaluation produces `f64` values; NaN/Inf propagate and are filtered out by
//! the caller, which turns domain errors (e.g. `sqrt(-1)`, `x/0`) into gaps in
//! the output trace — consistent with `MathKind::Divide`.

use crate::data::traces::TraceRef;

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

/// Supported function names with fixed or variable arity.
///
/// `Root(x, n)` is the n-th root of x. `Log` accepts one argument (base-10) or
/// two arguments (`log(x, b)` = arbitrary base `b`). `Atan2(y, x)` follows the
/// conventional argument order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    Sqrt,
    Root,
    Exp,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Sinh,
    Cosh,
    Tanh,
    Asinh,
    Acosh,
    Atanh,
    Abs,
    Ln,
    Log10,
    Log2,
    Log,
    /// `min(a, b, …)` — per-sample minimum over all arguments.
    Min,
    /// `max(a, b, …)` — per-sample maximum over all arguments.
    Max,
    /// `minh(a, b, …)` — running minimum over the evaluation history
    /// (stateful; the accumulator lives on the `MathTrace`).
    MinH,
    /// `maxh(a, b, …)` — running maximum over the evaluation history.
    MaxH,
}

impl Func {
    /// All supported functions, for UI listing and documentation.
    pub const ALL: &'static [Func] = &[
        Func::Sqrt,
        Func::Root,
        Func::Exp,
        Func::Sin,
        Func::Cos,
        Func::Tan,
        Func::Asin,
        Func::Acos,
        Func::Atan,
        Func::Atan2,
        Func::Sinh,
        Func::Cosh,
        Func::Tanh,
        Func::Asinh,
        Func::Acosh,
        Func::Atanh,
        Func::Abs,
        Func::Ln,
        Func::Log10,
        Func::Log2,
        Func::Log,
        Func::Min,
        Func::Max,
        Func::MinH,
        Func::MaxH,
    ];

    /// Look up a function by name.
    pub fn from_name(name: &str) -> Option<Func> {
        Some(match name {
            "sqrt" => Func::Sqrt,
            "root" => Func::Root,
            "exp" => Func::Exp,
            "sin" => Func::Sin,
            "cos" => Func::Cos,
            "tan" => Func::Tan,
            "asin" => Func::Asin,
            "acos" => Func::Acos,
            "atan" => Func::Atan,
            "atan2" => Func::Atan2,
            "sinh" => Func::Sinh,
            "cosh" => Func::Cosh,
            "tanh" => Func::Tanh,
            "asinh" => Func::Asinh,
            "acosh" => Func::Acosh,
            "atanh" => Func::Atanh,
            "abs" => Func::Abs,
            "ln" => Func::Ln,
            "log10" => Func::Log10,
            "log2" => Func::Log2,
            "log" => Func::Log,
            "min" => Func::Min,
            "max" => Func::Max,
            "minh" => Func::MinH,
            "maxh" => Func::MaxH,
            _ => return None,
        })
    }

    /// Canonical name used for display and chip insertion.
    pub fn name(self) -> &'static str {
        match self {
            Func::Sqrt => "sqrt",
            Func::Root => "root",
            Func::Exp => "exp",
            Func::Sin => "sin",
            Func::Cos => "cos",
            Func::Tan => "tan",
            Func::Asin => "asin",
            Func::Acos => "acos",
            Func::Atan => "atan",
            Func::Atan2 => "atan2",
            Func::Sinh => "sinh",
            Func::Cosh => "cosh",
            Func::Tanh => "tanh",
            Func::Asinh => "asinh",
            Func::Acosh => "acosh",
            Func::Atanh => "atanh",
            Func::Abs => "abs",
            Func::Ln => "ln",
            Func::Log10 => "log10",
            Func::Log2 => "log2",
            Func::Log => "log",
            Func::Min => "min",
            Func::Max => "max",
            Func::MinH => "minh",
            Func::MaxH => "maxh",
        }
    }

    /// Whether `n` is an accepted argument count for this function.
    fn arity_ok(self, n: usize) -> bool {
        match self {
            Func::Root | Func::Atan2 => n == 2,
            Func::Log => n == 1 || n == 2,
            Func::Min | Func::Max | Func::MinH | Func::MaxH => n >= 1,
            _ => n == 1,
        }
    }

    /// Human-readable accepted-argument-count description for error messages.
    fn arity_desc(self) -> &'static str {
        match self {
            Func::Root | Func::Atan2 => "2",
            Func::Log => "1 or 2",
            Func::Min | Func::Max | Func::MinH | Func::MaxH => "1 or more",
            _ => "1",
        }
    }

    fn eval(self, args: &[f64]) -> f64 {
        match self {
            Func::Sqrt => args[0].sqrt(),
            Func::Root => {
                // n-th root; supports negative x for odd integer n.
                let (x, n) = (args[0], args[1]);
                if x < 0.0 && n.fract() == 0.0 && (n as i64) % 2 != 0 {
                    -(-x).powf(1.0 / n)
                } else {
                    x.powf(1.0 / n)
                }
            }
            Func::Exp => args[0].exp(),
            Func::Sin => args[0].sin(),
            Func::Cos => args[0].cos(),
            Func::Tan => args[0].tan(),
            Func::Asin => args[0].asin(),
            Func::Acos => args[0].acos(),
            Func::Atan => args[0].atan(),
            Func::Atan2 => args[0].atan2(args[1]),
            Func::Sinh => args[0].sinh(),
            Func::Cosh => args[0].cosh(),
            Func::Tanh => args[0].tanh(),
            Func::Asinh => args[0].asinh(),
            Func::Acosh => args[0].acosh(),
            Func::Atanh => args[0].atanh(),
            Func::Abs => args[0].abs(),
            Func::Ln => args[0].ln(),
            Func::Log10 => args[0].log10(),
            Func::Log2 => args[0].log2(),
            Func::Log => {
                if args.len() == 2 {
                    args[0].log(args[1])
                } else {
                    args[0].log10()
                }
            }
            // NaN-propagating extremes: a missing input makes the whole call
            // NaN so the caller can skip the point (same as binary ops).
            // `MinH`/`MaxH` get the same stateless fold here — the real
            // running accumulation happens in `Expr::eval_rec` where the
            // `HistState` accumulators live.
            Func::Min | Func::MinH => extremum(args, f64::INFINITY, f64::min),
            Func::Max | Func::MaxH => extremum(args, f64::NEG_INFINITY, f64::max),
        }
    }
}

/// Extreme of `args` with NaN propagation: any NaN argument yields NaN.
fn extremum(args: &[f64], init: f64, f: fn(f64, f64) -> f64) -> f64 {
    if args.iter().any(|v| v.is_nan()) {
        f64::NAN
    } else {
        args.iter().copied().fold(init, f)
    }
}

/// Running accumulators for `minh`/`maxh` call sites.
///
/// One `f64` slot per history-function call site in pre-order AST traversal
/// (the slot index is assigned deterministically during evaluation). Stored on
/// `MathTrace` so incremental `compute_math_trace` calls continue the running
/// extreme instead of restarting it. `NaN` means "no valid sample yet".
#[derive(Debug, Clone, Default)]
pub struct HistState {
    /// Running extreme per `minh`/`maxh` call site, in pre-order traversal.
    pub slots: Vec<f64>,
}

/// Parsed expression AST.
///
/// `Group` records explicit parentheses from the source text; it is a no-op for
/// evaluation but the preview renderer uses it to decide between inline slashes
/// and stacked fractions.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Numeric literal.
    Num(f64),
    /// The sample timestamp `t`.
    Time,
    /// Named constant: `pi`, `e`.
    Const(&'static str, f64),
    /// Reference to a trace `{name}`.
    Trace(TraceRef),
    /// Unary minus.
    Neg(Box<Expr>),
    /// Binary operation.
    Bin(BinOp, Box<Expr>, Box<Expr>),
    /// Function call.
    Call(Func, Vec<Expr>),
    /// Explicitly parenthesized sub-expression.
    Group(Box<Expr>),
}

/// Parse error with a character offset into the source string (for UI hints).
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// Character offset where the error occurred.
    pub pos: usize,
    /// Human-readable description.
    pub msg: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "position {}: {}", self.pos, self.msg)
    }
}

impl std::error::Error for ParseError {}

/// Parse a formula string into an [`Expr`] AST.
pub fn parse(src: &str) -> Result<Expr, ParseError> {
    let mut p = Parser {
        chars: src.chars().collect(),
        pos: 0,
    };
    let e = p.parse_expr()?;
    p.skip_ws();
    if p.pos < p.chars.len() {
        return Err(ParseError {
            pos: p.pos,
            msg: format!("unexpected character `{}`", p.chars[p.pos]),
        });
    }
    Ok(e)
}

impl Expr {
    /// Evaluate the expression at time `t`.
    ///
    /// `get` resolves a trace reference to its value at `t` (interpolation is
    /// the caller's job). A missing trace value yields `NaN`, which propagates
    /// through the whole expression so callers can simply skip non-finite
    /// results.
    pub fn eval(&self, t: f64, get: &mut dyn FnMut(&TraceRef) -> Option<f64>) -> f64 {
        self.eval_ctx(t, get, &mut HistState::default())
    }

    /// Evaluate with persistent history state.
    ///
    /// `state` carries the `minh`/`maxh` accumulators across calls — the
    /// `MathTrace` keeps one and passes it in on every grid point and every
    /// incremental compute, so the running extreme spans the whole history.
    /// Accumulation is idempotent: re-evaluating the same inputs (snapshot
    /// pass, full recompute) leaves the accumulators unchanged.
    pub fn eval_ctx(
        &self,
        t: f64,
        get: &mut dyn FnMut(&TraceRef) -> Option<f64>,
        state: &mut HistState,
    ) -> f64 {
        let mut slot = 0usize;
        self.eval_rec(t, get, state, &mut slot)
    }

    fn eval_rec(
        &self,
        t: f64,
        get: &mut dyn FnMut(&TraceRef) -> Option<f64>,
        state: &mut HistState,
        slot: &mut usize,
    ) -> f64 {
        match self {
            Expr::Num(v) => *v,
            Expr::Time => t,
            Expr::Const(_, v) => *v,
            Expr::Trace(name) => get(name).unwrap_or(f64::NAN),
            Expr::Neg(inner) => -inner.eval_rec(t, get, state, slot),
            Expr::Bin(op, l, r) => {
                let (a, b) = (
                    l.eval_rec(t, get, state, slot),
                    r.eval_rec(t, get, state, slot),
                );
                match op {
                    BinOp::Add => a + b,
                    BinOp::Sub => a - b,
                    BinOp::Mul => a * b,
                    BinOp::Div => a / b,
                    BinOp::Pow => a.powf(b),
                }
            }
            Expr::Call(f, args) => {
                if matches!(f, Func::MinH | Func::MaxH) {
                    // Claim the accumulator slot before evaluating args so the
                    // mapping is a stable pre-order traversal index — nested
                    // `minh`/`maxh` in the args get later slots.
                    let idx = *slot;
                    *slot += 1;
                    if state.slots.len() <= idx {
                        state.slots.resize(idx + 1, f64::NAN);
                    }
                    let vals: Vec<f64> = args
                        .iter()
                        .map(|a| a.eval_rec(t, get, state, slot))
                        .collect();
                    let cur = f.eval(&vals);
                    if cur.is_nan() {
                        // Missing input → skip the point, keep the extreme.
                        return f64::NAN;
                    }
                    let acc = &mut state.slots[idx];
                    *acc = if acc.is_nan() {
                        cur
                    } else if *f == Func::MinH {
                        acc.min(cur)
                    } else {
                        acc.max(cur)
                    };
                    *acc
                } else {
                    let vals: Vec<f64> = args
                        .iter()
                        .map(|a| a.eval_rec(t, get, state, slot))
                        .collect();
                    f.eval(&vals)
                }
            }
            Expr::Group(inner) => inner.eval_rec(t, get, state, slot),
        }
    }

    /// Whether the expression contains a history function (`minh`/`maxh`).
    pub fn uses_history(&self) -> bool {
        match self {
            Expr::Call(f, args) => {
                matches!(f, Func::MinH | Func::MaxH) || args.iter().any(Expr::uses_history)
            }
            Expr::Neg(inner) | Expr::Group(inner) => inner.uses_history(),
            Expr::Bin(_, l, r) => l.uses_history() || r.uses_history(),
            _ => false,
        }
    }

    /// Names of all traces referenced via `{name}`, deduplicated and sorted for
    /// deterministic output. Builtins (`t`, `pi`, `e`) are never included.
    pub fn referenced_traces(&self) -> Vec<TraceRef> {
        let mut out: Vec<TraceRef> = Vec::new();
        self.collect_traces(&mut out);
        out.sort();
        out.dedup();
        out
    }

    fn collect_traces(&self, out: &mut Vec<TraceRef>) {
        match self {
            Expr::Trace(name) => out.push(name.clone()),
            Expr::Neg(inner) | Expr::Group(inner) => inner.collect_traces(out),
            Expr::Bin(_, l, r) => {
                l.collect_traces(out);
                r.collect_traces(out);
            }
            Expr::Call(_, args) => {
                for a in args {
                    a.collect_traces(out);
                }
            }
            Expr::Num(_) | Expr::Time | Expr::Const(_, _) => {}
        }
    }
}

// =============================================================================
// Lexer + recursive-descent parser
// =============================================================================

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    TraceName(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError {
            pos: self.pos.min(self.chars.len()),
            msg: msg.into(),
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// Lex the next token; `Err` on malformed input, `Ok(None)` at end.
    fn next_tok(&mut self) -> Result<Option<Tok>, ParseError> {
        self.skip_ws();
        let Some(c) = self.peek() else {
            return Ok(None);
        };
        let start = self.pos;
        let tok = match c {
            '+' => {
                self.pos += 1;
                Tok::Plus
            }
            '-' => {
                self.pos += 1;
                Tok::Minus
            }
            '*' => {
                self.pos += 1;
                Tok::Star
            }
            '/' => {
                self.pos += 1;
                Tok::Slash
            }
            '^' => {
                self.pos += 1;
                Tok::Caret
            }
            '(' => {
                self.pos += 1;
                Tok::LParen
            }
            ')' => {
                self.pos += 1;
                Tok::RParen
            }
            ',' => {
                self.pos += 1;
                Tok::Comma
            }
            '{' => {
                self.pos += 1;
                let name_start = self.pos;
                while let Some(c) = self.peek() {
                    if c == '}' {
                        break;
                    }
                    self.pos += 1;
                }
                if self.peek().is_none() {
                    return Err(ParseError {
                        pos: start,
                        msg: "unclosed `{` — expected `}`".to_string(),
                    });
                }
                let name: String = self.chars[name_start..self.pos].iter().collect();
                self.pos += 1; // consume '}'
                let name = name.trim().to_string();
                if name.is_empty() {
                    return Err(ParseError {
                        pos: start,
                        msg: "empty trace reference `{}`".to_string(),
                    });
                }
                return Ok(Some(Tok::TraceName(name)));
            }
            c if c.is_ascii_digit() || c == '.' => {
                let mut end = self.pos;
                while end < self.chars.len()
                    && (self.chars[end].is_ascii_digit() || self.chars[end] == '.')
                {
                    end += 1;
                }
                // Optional exponent: [eE][+-]?digits — backtrack if malformed.
                if end < self.chars.len() && matches!(self.chars[end], 'e' | 'E') {
                    let mut e2 = end + 1;
                    if e2 < self.chars.len() && matches!(self.chars[e2], '+' | '-') {
                        e2 += 1;
                    }
                    let digits_start = e2;
                    while e2 < self.chars.len() && self.chars[e2].is_ascii_digit() {
                        e2 += 1;
                    }
                    if e2 > digits_start {
                        end = e2;
                    }
                }
                let text: String = self.chars[self.pos..end].iter().collect();
                match text.parse::<f64>() {
                    Ok(v) => {
                        self.pos = end;
                        Tok::Num(v)
                    }
                    Err(_) => {
                        return Err(ParseError {
                            pos: start,
                            msg: format!("invalid number `{text}`"),
                        })
                    }
                }
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut end = self.pos;
                while end < self.chars.len()
                    && (self.chars[end].is_ascii_alphanumeric() || self.chars[end] == '_')
                {
                    end += 1;
                }
                let name: String = self.chars[self.pos..end].iter().collect();
                self.pos = end;
                Tok::Ident(name)
            }
            _ => {
                return Err(ParseError {
                    pos: start,
                    msg: format!("unexpected character `{c}`"),
                })
            }
        };
        Ok(Some(tok))
    }

    /// Peek at the next token without consuming it.
    fn peek_tok(&mut self) -> Result<Option<Tok>, ParseError> {
        let save = self.pos;
        let t = self.next_tok()?;
        self.pos = save;
        Ok(t)
    }

    fn expect_tok(&mut self, want: Tok, what: &str) -> Result<(), ParseError> {
        match self.next_tok()? {
            Some(t) if t == want => Ok(()),
            Some(t) => Err(self.err(format!("expected {what}, found `{t:?}`"))),
            None => Err(self.err(format!("expected {what}, found end of input"))),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_add()
    }

    fn parse_add(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_mul()?;
        loop {
            match self.peek_tok()? {
                Some(Tok::Plus) => {
                    self.next_tok()?;
                    let rhs = self.parse_mul()?;
                    lhs = Expr::Bin(BinOp::Add, Box::new(lhs), Box::new(rhs));
                }
                Some(Tok::Minus) => {
                    self.next_tok()?;
                    let rhs = self.parse_mul()?;
                    lhs = Expr::Bin(BinOp::Sub, Box::new(lhs), Box::new(rhs));
                }
                _ => return Ok(lhs),
            }
        }
    }

    fn parse_mul(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_unary()?;
        loop {
            match self.peek_tok()? {
                Some(Tok::Star) => {
                    self.next_tok()?;
                    let rhs = self.parse_unary()?;
                    lhs = Expr::Bin(BinOp::Mul, Box::new(lhs), Box::new(rhs));
                }
                Some(Tok::Slash) => {
                    self.next_tok()?;
                    let rhs = self.parse_unary()?;
                    lhs = Expr::Bin(BinOp::Div, Box::new(lhs), Box::new(rhs));
                }
                // Implicit multiplication: a primary directly follows.
                Some(Tok::Num(_))
                | Some(Tok::Ident(_))
                | Some(Tok::TraceName(_))
                | Some(Tok::LParen) => {
                    let rhs = self.parse_unary()?;
                    lhs = Expr::Bin(BinOp::Mul, Box::new(lhs), Box::new(rhs));
                }
                _ => return Ok(lhs),
            }
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.peek_tok()?, Some(Tok::Minus)) {
            self.next_tok()?;
            let inner = self.parse_unary()?;
            return Ok(Expr::Neg(Box::new(inner)));
        }
        // Leading `+` is a no-op.
        if matches!(self.peek_tok()?, Some(Tok::Plus)) {
            self.next_tok()?;
            return self.parse_unary();
        }
        self.parse_pow()
    }

    fn parse_pow(&mut self) -> Result<Expr, ParseError> {
        let base = self.parse_primary()?;
        if matches!(self.peek_tok()?, Some(Tok::Caret)) {
            self.next_tok()?;
            // Right-associative; unary minus allowed in the exponent (x^-2).
            let exp = self.parse_unary()?;
            return Ok(Expr::Bin(BinOp::Pow, Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let start = self.pos;
        match self.next_tok()? {
            Some(Tok::Num(v)) => Ok(Expr::Num(v)),
            Some(Tok::TraceName(name)) => Ok(Expr::Trace(TraceRef::new(name))),
            Some(Tok::LParen) => {
                let inner = self.parse_expr()?;
                self.expect_tok(Tok::RParen, "`)`")?;
                Ok(Expr::Group(Box::new(inner)))
            }
            Some(Tok::Ident(name)) => match name.as_str() {
                "t" => Ok(Expr::Time),
                "pi" => Ok(Expr::Const("pi", std::f64::consts::PI)),
                "e" => Ok(Expr::Const("e", std::f64::consts::E)),
                _ => {
                    if let Some(f) = Func::from_name(&name) {
                        // Function call requires parentheses.
                        match self.peek_tok()? {
                            Some(Tok::LParen) => {
                                self.next_tok()?;
                                self.parse_call_args(f, start)
                            }
                            _ => Err(ParseError {
                                pos: start,
                                msg: format!("expected `(` after function name `{name}`"),
                            }),
                        }
                    } else {
                        Err(ParseError {
                            pos: start,
                            msg: format!(
                                "unknown identifier `{name}` — wrap trace names in braces: `{{{name}}}`"
                            ),
                        })
                    }
                }
            },
            Some(t) => Err(ParseError {
                pos: start,
                msg: format!("unexpected token `{t:?}`"),
            }),
            None => Err(ParseError {
                pos: start,
                msg: "unexpected end of input".to_string(),
            }),
        }
    }

    fn parse_call_args(&mut self, f: Func, start: usize) -> Result<Expr, ParseError> {
        let mut args = Vec::new();
        if matches!(self.peek_tok()?, Some(Tok::RParen)) {
            self.next_tok()?;
        } else {
            loop {
                args.push(self.parse_expr()?);
                match self.next_tok()? {
                    Some(Tok::Comma) => continue,
                    Some(Tok::RParen) => break,
                    Some(t) => return Err(self.err(format!("expected `,` or `)`, found `{t:?}`"))),
                    None => return Err(self.err("expected `)`, found end of input")),
                }
            }
        }
        if !f.arity_ok(args.len()) {
            return Err(ParseError {
                pos: start,
                msg: format!(
                    "`{}` expects {} argument(s), got {}",
                    f.name(),
                    f.arity_desc(),
                    args.len()
                ),
            });
        }
        Ok(Expr::Call(f, args))
    }
}
