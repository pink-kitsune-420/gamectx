//! Safe formulas that a game or its model can define once and the game evaluates exactly.
//!
//! Models are poor at arithmetic over data and good at deciding *what* to calculate. A
//! formula lets the model say it once (`danger(hp, dist) = max(0, 10 - dist) * (1 - hp/100)`)
//! and the game compute it exactly every turn, for example as a table column that `where`
//! and `sort_by` can use.
//!
//! ```
//! use gamectx::data::formula::Formulas;
//! use gamectx::Value;
//!
//! let mut f = Formulas::new();
//! f.define("danger", &["hp", "dist"], "max(0, 10 - dist) * (1 - hp / 100)", "Threat from a hound.")?;
//! assert_eq!(f.call("danger", &[Value::from(50), Value::from(4)])?, Value::from(3.0));
//! assert_eq!(f.calc("round(danger(20, 2) + 1.25, 1)")?, Value::from(7.7));
//! # Ok::<(), gamectx::Error>(())
//! ```
//!
//! Formulas are not code. The language has numbers and true/false; `+ - * / %`; comparisons
//! `< <= > >= == !=`; `and`, `or`, `not`; `if(condition, then, else)`; and `min`, `max`,
//! `abs`, `clamp(x, low, high)`, `round(x)` or `round(x, places)`, `floor`, `ceil`, `sqrt`.
//! A formula may call formulas defined before it, so recursion is impossible. Types, names
//! and argument counts are checked when a formula is defined. Evaluation touches nothing
//! outside its arguments and stops after a fixed number of steps.

use crate::data::json::{Error, Result, Value};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub formulas: usize,
    pub body_chars: usize,
    pub nodes: usize,
    pub params: usize,
    pub steps: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { formulas: 64, body_chars: 300, nodes: 200, params: 8, steps: 10_000 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Type {
    Num,
    Bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum V {
    Num(f64),
    Bool(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Builtin {
    Min,
    Max,
    Abs,
    Clamp,
    Round,
    Floor,
    Ceil,
    Sqrt,
    If,
}

impl Builtin {
    fn named(name: &str) -> Option<Builtin> {
        Some(match name {
            "min" => Builtin::Min,
            "max" => Builtin::Max,
            "abs" => Builtin::Abs,
            "clamp" => Builtin::Clamp,
            "round" => Builtin::Round,
            "floor" => Builtin::Floor,
            "ceil" => Builtin::Ceil,
            "sqrt" => Builtin::Sqrt,
            "if" => Builtin::If,
            _ => return None,
        })
    }
}

const RESERVED: [&str; 12] = ["min", "max", "abs", "clamp", "round", "floor", "ceil", "sqrt", "if", "and", "or", "not"];

#[derive(Clone, Debug)]
enum Expr {
    Num(f64),
    Bool(bool),
    Var(usize),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(&'static str, Box<Expr>, Box<Expr>),
    Builtin(Builtin, Vec<Expr>),
    User(usize, Vec<Expr>),
}

#[derive(Clone, Debug)]
struct Def {
    name: String,
    params: Vec<String>,
    body: String,
    description: String,
    expr: Expr,
    ty: Type,
}

/// A set of formulas, each able to call the ones defined before it.
#[derive(Clone, Debug)]
pub struct Formulas {
    defs: Vec<Def>,
    limits: Limits,
}

impl Default for Formulas {
    fn default() -> Self {
        Formulas::new()
    }
}

// ------------------------------------------------------------------ tokens

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(&'static str),
    LParen,
    RParen,
    Comma,
}

const OPS: [&str; 13] = ["<=", ">=", "==", "!=", "<", ">", "+", "-", "*", "/", "%", "(", ")"];

fn tokenize(text: &str) -> Result<Vec<Tok>> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                i += 1;
                if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                    i += 1;
                }
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let text: String = chars[start..i].iter().collect();
            let n: f64 = text.parse().map_err(|_| Error::new(format!("bad number \"{text}\"")))?;
            if !n.is_finite() {
                return Err(Error::new(format!("number \"{text}\" is too large")));
            }
            out.push(Tok::Num(n));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
        } else if c == ',' {
            out.push(Tok::Comma);
            i += 1;
        } else {
            let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
            let op =
                OPS.iter().find(|op| two.starts_with(**op)).ok_or_else(|| Error::new(format!("unexpected \"{c}\"")))?;
            i += op.chars().count();
            out.push(match *op {
                "(" => Tok::LParen,
                ")" => Tok::RParen,
                op => Tok::Op(op),
            });
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ parser and type checker

struct Parser<'a> {
    toks: Vec<Tok>,
    pos: usize,
    params: &'a [String],
    defs: &'a [Def],
    nodes: usize,
    max_nodes: usize,
    depth: usize,
}

const MAX_DEPTH: usize = 32;

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn node(&mut self) -> Result<()> {
        self.nodes += 1;
        if self.nodes > self.max_nodes {
            return Err(Error::new(format!("formula is too long (more than {} parts)", self.max_nodes)));
        }
        Ok(())
    }

    fn expect(&mut self, tok: Tok, what: &str) -> Result<()> {
        if self.next() == Some(tok) {
            Ok(())
        } else {
            Err(Error::new(format!("expected {what}")))
        }
    }

    /// Precedence climbing: or < and < not < comparison < + - < * / % < unary minus.
    fn expr(&mut self, min: u8) -> Result<(Expr, Type)> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Error::new("formula is nested too deeply"));
        }
        let (mut left, mut lt) = self.prefix()?;
        loop {
            let (op, prec): (&'static str, u8) = match self.peek() {
                Some(Tok::Ident(w)) if w == "or" => ("or", 1),
                Some(Tok::Ident(w)) if w == "and" => ("and", 2),
                Some(Tok::Op(op)) if ["<", "<=", ">", ">=", "==", "!="].contains(op) => (op, 4),
                Some(Tok::Op(op)) if ["+", "-"].contains(op) => (op, 5),
                Some(Tok::Op(op)) if ["*", "/", "%"].contains(op) => (op, 6),
                _ => break,
            };
            if prec < min {
                break;
            }
            self.pos += 1;
            self.node()?;
            // Comparisons do not chain: `a < b < c` is an error, not a surprise.
            let (right, rt) = self.expr(if prec == 4 { 5 } else { prec + 1 })?;
            let ty = match op {
                "or" | "and" if lt == Type::Bool && rt == Type::Bool => Type::Bool,
                "or" | "and" => return Err(Error::new(format!("\"{op}\" needs true/false on both sides"))),
                "==" | "!=" if lt == rt => Type::Bool,
                "==" | "!=" => return Err(Error::new(format!("\"{op}\" compares a number with true/false"))),
                "<" | "<=" | ">" | ">=" if lt == Type::Num && rt == Type::Num => Type::Bool,
                _ if lt == Type::Num && rt == Type::Num && !["<", "<=", ">", ">="].contains(&op) => Type::Num,
                _ => return Err(Error::new(format!("\"{op}\" needs numbers on both sides"))),
            };
            if prec == 4 && matches!(self.peek(), Some(Tok::Op(o)) if ["<", "<=", ">", ">=", "==", "!="].contains(o)) {
                return Err(Error::new("comparisons cannot be chained; use \"and\""));
            }
            left = Expr::Bin(op, Box::new(left), Box::new(right));
            lt = ty;
        }
        self.depth -= 1;
        Ok((left, lt))
    }

    fn prefix(&mut self) -> Result<(Expr, Type)> {
        self.node()?;
        match self.next() {
            Some(Tok::Num(n)) => Ok((Expr::Num(n), Type::Num)),
            Some(Tok::Op("-")) => {
                let (e, t) = self.expr(7)?;
                if t != Type::Num {
                    return Err(Error::new("\"-\" needs a number"));
                }
                Ok((Expr::Neg(Box::new(e)), Type::Num))
            }
            Some(Tok::LParen) => {
                let inner = self.expr(0)?;
                self.expect(Tok::RParen, "\")\"")?;
                Ok(inner)
            }
            Some(Tok::Ident(w)) if w == "not" => {
                let (e, t) = self.expr(3)?;
                if t != Type::Bool {
                    return Err(Error::new("\"not\" needs true/false"));
                }
                Ok((Expr::Not(Box::new(e)), Type::Bool))
            }
            Some(Tok::Ident(w)) if w == "true" || w == "false" => Ok((Expr::Bool(w == "true"), Type::Bool)),
            Some(Tok::Ident(name)) => {
                if self.peek() == Some(&Tok::LParen) {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if self.peek() != Some(&Tok::RParen) {
                        loop {
                            args.push(self.expr(0)?);
                            match self.next() {
                                Some(Tok::Comma) => continue,
                                Some(Tok::RParen) => break,
                                _ => return Err(Error::new(format!("expected \",\" or \")\" in {name}(...)"))),
                            }
                        }
                    } else {
                        self.pos += 1;
                    }
                    self.call(&name, args)
                } else if let Some(i) = self.params.iter().position(|p| *p == name) {
                    Ok((Expr::Var(i), Type::Num))
                } else {
                    Err(Error::new(format!("unknown name \"{name}\"")))
                }
            }
            Some(t) => Err(Error::new(format!("unexpected {t:?}"))),
            None => Err(Error::new("formula ended too early")),
        }
    }

    fn call(&mut self, name: &str, args: Vec<(Expr, Type)>) -> Result<(Expr, Type)> {
        let types: Vec<Type> = args.iter().map(|(_, t)| *t).collect();
        let exprs: Vec<Expr> = args.into_iter().map(|(e, _)| e).collect();
        let nums = |n: std::ops::RangeInclusive<usize>| -> Result<()> {
            if !n.contains(&types.len()) || types.iter().any(|t| *t != Type::Num) {
                let count =
                    if n.start() == n.end() { n.start().to_string() } else { format!("{} to {}", n.start(), n.end()) };
                return Err(Error::new(format!("{name}() takes {count} numbers")));
            }
            Ok(())
        };
        if let Some(b) = Builtin::named(name) {
            let ty = match b {
                Builtin::Min | Builtin::Max => nums(1..=16).map(|_| Type::Num)?,
                Builtin::Abs | Builtin::Floor | Builtin::Ceil | Builtin::Sqrt => nums(1..=1).map(|_| Type::Num)?,
                Builtin::Round => nums(1..=2).map(|_| Type::Num)?,
                Builtin::Clamp => nums(3..=3).map(|_| Type::Num)?,
                Builtin::If => {
                    if types.len() != 3 || types[0] != Type::Bool || types[1] != types[2] {
                        return Err(Error::new("if() takes a true/false condition and two results of the same kind"));
                    }
                    types[1]
                }
            };
            return Ok((Expr::Builtin(b, exprs), ty));
        }
        let (i, def) = self
            .defs
            .iter()
            .enumerate()
            .find(|(_, d)| d.name == name)
            .ok_or_else(|| Error::new(format!("unknown formula \"{name}\"")))?;
        if types.len() != def.params.len() || types.iter().any(|t| *t != Type::Num) {
            return Err(Error::new(format!("{name}() takes {} numbers", def.params.len())));
        }
        Ok((Expr::User(i, exprs), def.ty))
    }
}

fn name_ok(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.len() <= 32
        && !RESERVED.contains(&name)
        && name != "true"
        && name != "false"
}

// ------------------------------------------------------------------ evaluation

struct Eval<'a> {
    defs: &'a [Def],
    steps: usize,
    max_steps: usize,
}

impl Eval<'_> {
    fn num(&mut self, e: &Expr, args: &[f64]) -> Result<f64> {
        match self.eval(e, args)? {
            V::Num(n) => Ok(n),
            V::Bool(_) => Err(Error::new("internal type error")),
        }
    }

    fn eval(&mut self, e: &Expr, args: &[f64]) -> Result<V> {
        self.steps += 1;
        if self.steps > self.max_steps {
            return Err(Error::new(format!("formula took more than {} steps", self.max_steps)));
        }
        let v = match e {
            Expr::Num(n) => V::Num(*n),
            Expr::Bool(b) => V::Bool(*b),
            Expr::Var(i) => V::Num(args[*i]),
            Expr::Neg(x) => V::Num(-self.num(x, args)?),
            Expr::Not(x) => V::Bool(self.eval(x, args)? == V::Bool(false)),
            Expr::Bin(op, a, b) => match *op {
                "and" => V::Bool(self.eval(a, args)? == V::Bool(true) && self.eval(b, args)? == V::Bool(true)),
                "or" => V::Bool(self.eval(a, args)? == V::Bool(true) || self.eval(b, args)? == V::Bool(true)),
                "==" => V::Bool(self.eval(a, args)? == self.eval(b, args)?),
                "!=" => V::Bool(self.eval(a, args)? != self.eval(b, args)?),
                op => {
                    let (x, y) = (self.num(a, args)?, self.num(b, args)?);
                    match op {
                        "<" => V::Bool(x < y),
                        "<=" => V::Bool(x <= y),
                        ">" => V::Bool(x > y),
                        ">=" => V::Bool(x >= y),
                        "+" => V::Num(x + y),
                        "-" => V::Num(x - y),
                        "*" => V::Num(x * y),
                        "/" if y == 0.0 => return Err(Error::new("division by zero")),
                        "/" => V::Num(x / y),
                        "%" if y == 0.0 => return Err(Error::new("remainder by zero")),
                        _ => V::Num(x % y),
                    }
                }
            },
            Expr::Builtin(b, xs) => match b {
                Builtin::If => {
                    if self.eval(&xs[0], args)? == V::Bool(true) {
                        self.eval(&xs[1], args)?
                    } else {
                        self.eval(&xs[2], args)?
                    }
                }
                _ => {
                    let mut n = Vec::with_capacity(xs.len());
                    for x in xs {
                        n.push(self.num(x, args)?);
                    }
                    V::Num(match b {
                        Builtin::Min => n.iter().copied().fold(f64::INFINITY, f64::min),
                        Builtin::Max => n.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                        Builtin::Abs => n[0].abs(),
                        Builtin::Floor => n[0].floor(),
                        Builtin::Ceil => n[0].ceil(),
                        Builtin::Sqrt if n[0] < 0.0 => return Err(Error::new("sqrt of a negative number")),
                        Builtin::Sqrt => n[0].sqrt(),
                        Builtin::Clamp if n[1] > n[2] => return Err(Error::new("clamp low is above high")),
                        Builtin::Clamp => n[0].max(n[1]).min(n[2]),
                        Builtin::Round => {
                            let places = n.get(1).copied().unwrap_or(0.0);
                            if !(0.0..=12.0).contains(&places) || places.fract() != 0.0 {
                                return Err(Error::new("round places must be a whole number from 0 to 12"));
                            }
                            let scale = 10f64.powi(places as i32);
                            (n[0] * scale).round() / scale
                        }
                        Builtin::If => unreachable!(),
                    })
                }
            },
            Expr::User(i, xs) => {
                let mut n = Vec::with_capacity(xs.len());
                for x in xs {
                    n.push(self.num(x, args)?);
                }
                let def = &self.defs[*i];
                self.eval(&def.expr, &n)?
            }
        };
        if let V::Num(n) = v {
            if !n.is_finite() {
                return Err(Error::new("result is not a finite number"));
            }
        }
        Ok(v)
    }
}

fn to_value(v: V) -> Value {
    match v {
        V::Num(n) if n.fract() == 0.0 && n.abs() < 1e15 => Value::from(n as i64),
        V::Num(n) => Value::from(n),
        V::Bool(b) => Value::Bool(b),
    }
}

// ------------------------------------------------------------------ public API

impl Formulas {
    pub fn new() -> Formulas {
        Formulas::with_limits(Limits::default())
    }

    pub fn with_limits(limits: Limits) -> Formulas {
        Formulas { defs: Vec::new(), limits }
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        self.defs.iter().map(|d| d.name.as_str()).collect()
    }

    fn parse(&self, body: &str, params: &[String]) -> Result<(Expr, Type)> {
        if body.chars().count() > self.limits.body_chars || body.contains(['\n', '\r']) {
            return Err(Error::new(format!(
                "formula must be one line of at most {} characters",
                self.limits.body_chars
            )));
        }
        let toks = tokenize(body)?;
        let mut p = Parser { toks, pos: 0, params, defs: &self.defs, nodes: 0, max_nodes: self.limits.nodes, depth: 0 };
        let out = p.expr(0)?;
        if p.pos != p.toks.len() {
            return Err(Error::new("unexpected text after the formula"));
        }
        Ok(out)
    }

    /// Defines a formula. Its body may use its parameters (numbers), literals, operators,
    /// built-in functions and formulas already defined. Redefining a name is refused;
    /// [`Formulas::forget`] it first.
    pub fn define(&mut self, name: &str, params: &[&str], body: &str, description: &str) -> Result<()> {
        let err = |e: Error| Error::new(format!("formula \"{name}\": {e}"));
        if !name_ok(name) {
            return Err(err(Error::new("names use letters, digits and _, start with a letter, are not built-ins")));
        }
        if self.defs.iter().any(|d| d.name == name) {
            return Err(err(Error::new("already defined")));
        }
        if self.defs.len() >= self.limits.formulas {
            return Err(err(Error::new(format!("at most {} formulas", self.limits.formulas))));
        }
        if params.len() > self.limits.params {
            return Err(err(Error::new(format!("at most {} parameters", self.limits.params))));
        }
        let mut owned: Vec<String> = Vec::new();
        for p in params {
            if !name_ok(p) || owned.iter().any(|o| o == p) || self.defs.iter().any(|d| d.name == *p) {
                return Err(err(Error::new(format!("bad or repeated parameter \"{p}\""))));
            }
            owned.push(p.to_string());
        }
        let description = description.trim();
        if description.chars().count() > 120 || description.contains(['\n', '\r']) {
            return Err(err(Error::new("description must be one line of at most 120 characters")));
        }
        let (expr, ty) = self.parse(body, &owned).map_err(err)?;
        self.defs.push(Def {
            name: name.to_owned(),
            params: owned,
            body: body.trim().to_owned(),
            description: description.to_owned(),
            expr,
            ty,
        });
        Ok(())
    }

    /// Removes a formula. Refused while a later formula uses it.
    pub fn forget(&mut self, name: &str) -> Result<()> {
        let i = self
            .defs
            .iter()
            .position(|d| d.name == name)
            .ok_or_else(|| Error::new(format!("no formula \"{name}\"")))?;
        if let Some(user) = self.defs[i + 1..].iter().find(|d| uses(&d.expr, i)) {
            return Err(Error::new(format!("formula \"{}\" uses \"{name}\"", user.name)));
        }
        self.defs.remove(i);
        for d in &mut self.defs[i..] {
            shift(&mut d.expr, i);
        }
        Ok(())
    }

    /// Calls a formula with number arguments.
    pub fn call(&self, name: &str, args: &[Value]) -> Result<Value> {
        let i = self
            .defs
            .iter()
            .position(|d| d.name == name)
            .ok_or_else(|| Error::new(format!("no formula \"{name}\"")))?;
        let def = &self.defs[i];
        if args.len() != def.params.len() {
            return Err(Error::new(format!("{name}() takes {} numbers", def.params.len())));
        }
        let mut nums = Vec::with_capacity(args.len());
        for (p, a) in def.params.iter().zip(args) {
            match a {
                Value::Number(t) => nums.push(t.parse::<f64>().map_err(|_| Error::new("unreadable number"))?),
                _ => return Err(Error::new(format!("{name}(): \"{p}\" must be a number"))),
            }
        }
        let mut ev = Eval { defs: &self.defs, steps: 0, max_steps: self.limits.steps };
        ev.eval(&def.expr, &nums).map(to_value)
    }

    /// Evaluates a one-off expression with no variables, such as a model's `calc` request:
    /// `"round(danger(20, 2) + 1.25, 1)"`.
    pub fn calc(&self, expression: &str) -> Result<Value> {
        self.eval_with(expression, &[])
    }

    /// Evaluates an expression whose names refer to the given number variables, such as a
    /// table row's columns.
    pub fn eval_with(&self, expression: &str, vars: &[(&str, f64)]) -> Result<Value> {
        let names: Vec<String> = vars.iter().map(|(n, _)| n.to_string()).collect();
        let values: Vec<f64> = vars.iter().map(|(_, v)| *v).collect();
        let (expr, _) = self.parse(expression, &names)?;
        let mut ev = Eval { defs: &self.defs, steps: 0, max_steps: self.limits.steps };
        ev.eval(&expr, &values).map(to_value)
    }

    /// Compiles an expression over named number variables once, for evaluating many rows.
    pub(crate) fn compile(&self, expression: &str, names: &[String]) -> Result<Compiled> {
        let (expr, _) = self.parse(expression, names)?;
        let mut used = Vec::new();
        vars(&expr, &mut used);
        Ok(Compiled { expr, used })
    }

    /// Runs a compiled expression. `values` follow the names given to `compile`; `None`
    /// marks a value that is missing or not a number, which makes the result `None` only
    /// when the expression uses it.
    pub(crate) fn run(&self, compiled: &Compiled, values: &[Option<f64>]) -> Result<Option<Value>> {
        if compiled.used.iter().any(|&i| values[i].is_none()) {
            return Ok(None);
        }
        let nums: Vec<f64> = values.iter().map(|v| v.unwrap_or(0.0)).collect();
        let mut ev = Eval { defs: &self.defs, steps: 0, max_steps: self.limits.steps };
        ev.eval(&compiled.expr, &nums).map(|v| Some(to_value(v)))
    }

    /// The defined formulas as text lines for a packet, so the model knows what exists:
    /// `danger(hp, dist) = max(0, 10 - dist) * (1 - hp / 100); Threat from a hound.`
    pub fn describe(&self) -> String {
        self.defs
            .iter()
            .map(|d| {
                let desc = if d.description.is_empty() { String::new() } else { format!("; {}", d.description) };
                format!("{}({}) = {}{desc}", d.name, d.params.join(", "), d.body)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Tools for the model: `define_formula` (save a formula for later turns) and `calc`
    /// (evaluate an expression exactly). Strict, so they pack as signatures.
    pub fn tools(&self) -> Value {
        let s = |desc: &str, max: usize| crate::obj! {"type" => "string", "maxLength" => max, "description" => desc};
        let tool = |name: &str, desc: &str, props: Value, required: Vec<&str>| {
            crate::obj! {"type" => "function", "function" => crate::obj! {
            "name" => name, "description" => desc, "strict" => true,
            "parameters" => crate::obj! {"type" => "object", "properties" => props,
                "required" => required, "additionalProperties" => false}}}
        };
        Value::Array(vec![
            tool(
                "define_formula",
                "Save a formula the game will compute exactly from now on. Numbers only; no loops.",
                crate::obj! {
                    "name" => s("Letters, digits and _.", 32),
                    "params" => s("Comma-separated parameter names.", 120),
                    "body" => s("Uses + - * / % < > == and or not if() min() max() abs() clamp() round() and earlier formulas.", self.limits.body_chars),
                    "description" => s("What it computes, one line.", 120),
                },
                vec!["name", "params", "body", "description"],
            ),
            tool(
                "calc",
                "Compute an expression exactly; the game replies with the result.",
                crate::obj! {"expression" => s("For example round(danger(20, 2) * 1.5, 1).", self.limits.body_chars)},
                vec!["expression"],
            ),
        ])
    }

    /// Validates a model's `define_formula` call and defines the formula.
    pub fn apply_define(&mut self, args: &Value) -> Result<()> {
        let mut keys: Vec<&str> =
            args.as_object().map(|o| o.iter().map(|(k, _)| k.as_str()).collect()).unwrap_or_default();
        keys.sort_unstable();
        if keys != ["body", "description", "name", "params"] {
            return Err(Error::new("define_formula needs exactly name, params, body and description"));
        }
        let text =
            |k: &str| args.get(k).and_then(Value::as_str).ok_or_else(|| Error::new(format!("{k} must be a string")));
        let params: Vec<&str> = text("params")?.split(',').map(str::trim).filter(|p| !p.is_empty()).collect();
        self.define(text("name")?, &params, text("body")?, text("description")?)
    }

    /// Saves formulas as JSON: `[{"name","params","body","description"}]`, in definition order.
    pub fn to_value(&self) -> Value {
        Value::Array(
            self.defs
                .iter()
                .map(|d| {
                    crate::obj! {"name" => d.name.as_str(), "params" => Value::strings(&d.params),
                    "body" => d.body.as_str(), "description" => d.description.as_str()}
                })
                .collect(),
        )
    }

    /// Loads formulas saved by [`Formulas::to_value`], checking each one again.
    pub fn from_value(value: &Value, limits: Limits) -> Result<Formulas> {
        let list = value.as_array().ok_or_else(|| Error::new("saved formulas must be an array"))?;
        let mut f = Formulas::with_limits(limits);
        for item in list {
            let text = |k: &str| {
                item.get(k).and_then(Value::as_str).ok_or_else(|| Error::new(format!("formula {k} must be a string")))
            };
            let params: Vec<&str> = item
                .get("params")
                .and_then(Value::as_array)
                .and_then(|a| a.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
                .ok_or_else(|| Error::new("formula params must be strings"))?;
            f.define(text("name")?, &params, text("body")?, text("description")?)?;
        }
        Ok(f)
    }
}

/// An expression compiled against fixed variable names; see [`Formulas::compile`].
#[derive(Clone, Debug)]
pub(crate) struct Compiled {
    expr: Expr,
    used: Vec<usize>,
}

fn vars(e: &Expr, out: &mut Vec<usize>) {
    match e {
        Expr::Var(i) if !out.contains(i) => out.push(*i),
        Expr::User(_, xs) | Expr::Builtin(_, xs) => xs.iter().for_each(|x| vars(x, out)),
        Expr::Neg(x) | Expr::Not(x) => vars(x, out),
        Expr::Bin(_, a, b) => {
            vars(a, out);
            vars(b, out);
        }
        _ => {}
    }
}

fn uses(e: &Expr, i: usize) -> bool {
    match e {
        Expr::User(j, xs) => *j == i || xs.iter().any(|x| uses(x, i)),
        Expr::Builtin(_, xs) => xs.iter().any(|x| uses(x, i)),
        Expr::Neg(x) | Expr::Not(x) => uses(x, i),
        Expr::Bin(_, a, b) => uses(a, i) || uses(b, i),
        _ => false,
    }
}

/// After removing definition `removed`, later references move down by one.
fn shift(e: &mut Expr, removed: usize) {
    match e {
        Expr::User(j, xs) => {
            if *j > removed {
                *j -= 1;
            }
            xs.iter_mut().for_each(|x| shift(x, removed));
        }
        Expr::Builtin(_, xs) => xs.iter_mut().for_each(|x| shift(x, removed)),
        Expr::Neg(x) | Expr::Not(x) => shift(x, removed),
        Expr::Bin(_, a, b) => {
            shift(a, removed);
            shift(b, removed);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::json::parse;

    fn f() -> Formulas {
        let mut f = Formulas::new();
        f.define("danger", &["hp", "dist"], "max(0, 10 - dist) * (1 - hp / 100)", "Threat from a hound.").unwrap();
        f.define("risky", &["hp", "dist"], "danger(hp, dist) > 3 and not (hp > 90)", "").unwrap();
        f
    }

    #[test]
    fn evaluates_with_precedence_and_types() {
        let f = f();
        assert_eq!(f.calc("1 + 2 * 3 - 4 / 2").unwrap(), Value::from(5));
        assert_eq!(f.calc("-2 * -3 % 4").unwrap(), Value::from(2));
        assert_eq!(f.calc("(1 + 2) * 3").unwrap(), Value::from(9));
        assert_eq!(f.calc("danger(50, 4)").unwrap(), Value::from(3));
        assert_eq!(f.calc("risky(10, 2)").unwrap(), Value::Bool(true));
        assert_eq!(f.calc("if(1 < 2 or false, clamp(15, 0, 10), 0)").unwrap(), Value::from(10));
        assert_eq!(f.calc("round(2.345, 2) + round(0.5)").unwrap(), Value::from(3.35));
        assert_eq!(
            f.calc("min(3, 1, 2) + max(1, 5) + abs(-1) + floor(1.9) + ceil(1.1) + sqrt(9)").unwrap(),
            Value::from(13)
        );
        assert_eq!(f.eval_with("hp * 2 + dist", &[("hp", 3.0), ("dist", 0.5)]).unwrap(), Value::from(6.5));
        assert_eq!(f.call("danger", &[Value::from(100), Value::from(0)]).unwrap(), Value::from(0));
    }

    #[test]
    fn refuses_unsafe_or_wrong_formulas_at_definition() {
        let mut f = f();
        let bad = |f: &mut Formulas, body: &str| f.define("x", &["a"], body, "").is_err();
        assert!(bad(&mut f, "a + true"), "type error");
        assert!(bad(&mut f, "b + 1"), "unknown name");
        assert!(bad(&mut f, "x(a)"), "recursion is impossible: x is not defined yet");
        assert!(bad(&mut f, "danger(a)"), "argument count");
        assert!(bad(&mut f, "1 < a < 3"), "chained comparison");
        assert!(bad(&mut f, "if(a, 1, 2)"), "condition must be true/false");
        assert!(bad(&mut f, "a +"), "incomplete");
        assert!(bad(&mut f, "a; rm -rf /"), "not code");
        assert!(bad(&mut f, &"(".repeat(40)), "nesting");
        assert!(bad(&mut f, &vec!["a"; 150].join(" + ")), "too many parts");
        assert!(f.define("min", &[], "1", "").is_err(), "built-in names are reserved");
        assert!(f.define("danger", &[], "1", "").is_err(), "no redefinition");
        assert!(f.define("y", &["a", "a"], "a", "").is_err(), "repeated parameter");
    }

    #[test]
    fn runtime_errors_and_step_limit() {
        let f = f();
        assert!(f.calc("1 / 0").is_err());
        assert!(f.calc("5 % 0").is_err());
        assert!(f.calc("sqrt(-1)").is_err());
        assert!(f.calc("round(1, 13)").is_err());
        assert!(f.calc("1e308 * 10").is_err(), "non-finite");
        let mut chain = Formulas::new();
        chain.define("f0", &["x"], "x + 1", "").unwrap();
        for i in 1..20 {
            chain.define(&format!("f{i}"), &["x"], &format!("f{}(x) + f{}(x)", i - 1, i - 1), "").unwrap();
        }
        assert!(chain.calc("f19(1)").unwrap_err().to_string().contains("steps"), "2^19 calls stop at the step limit");
        assert!(chain.calc("f5(1)").is_ok());
    }

    #[test]
    fn model_tools_forget_and_round_trip() {
        let mut f = f();
        let call = parse(r#"{"name":"heal_cost","params":"hp, max_hp","body":"ceil((max_hp - hp) / 10)","description":"Coins to heal."}"#, 1024).unwrap();
        f.apply_define(&call).unwrap();
        assert_eq!(f.calc("heal_cost(42, 100)").unwrap(), Value::from(6));
        assert!(f.apply_define(&parse(r#"{"name":"z","params":"","body":"1"}"#, 64).unwrap()).is_err());
        assert!(crate::packet::signatures::pack(f.tools().as_array().unwrap()).is_ok());
        assert_eq!(
            f.describe(),
            "danger(hp, dist) = max(0, 10 - dist) * (1 - hp / 100); Threat from a hound.\nrisky(hp, dist) = danger(hp, dist) > 3 and not (hp > 90)\nheal_cost(hp, max_hp) = ceil((max_hp - hp) / 10); Coins to heal."
        );
        let saved = f.to_value();
        let loaded = Formulas::from_value(&parse(&saved.to_json(), 1 << 16).unwrap(), Limits::default()).unwrap();
        assert_eq!(loaded.describe(), f.describe());
        assert!(f.forget("danger").is_err(), "risky uses danger");
        f.forget("risky").unwrap();
        assert_eq!(f.calc("heal_cost(90, 100)").unwrap(), Value::from(1), "later references still resolve");
        f.forget("danger").unwrap();
        assert_eq!(f.calc("heal_cost(0, 100)").unwrap(), Value::from(10));
    }
}
