//! Model-written queries: a small SQL `SELECT` that the game runs exactly over its own tables.
//!
//! Models filter numbers and add them up badly, whatever format the data is in. They write
//! SQL well. A [`tool`] lets the model ask for the rows or totals it needs, and [`run`]
//! computes them exactly from the game's tables:
//!
//! ```
//! use gamectx::{json, query};
//!
//! let items = json::parse(r#"[{"id":"salt_pouch","kind":"reagent","count":14,"weight":0.5},
//!     {"id":"moss_cake","kind":"food","count":2,"weight":0.25},
//!     {"id":"ember_cake","kind":"food","count":6,"weight":3.75}]"#, 1 << 20)?;
//! let tables = [("inventory", items.as_array().unwrap())];
//!
//! let heavy = query::run("SELECT id FROM inventory WHERE count >= 6 ORDER BY weight DESC", &tables)?;
//! assert_eq!(heavy.to_json(), r#"["ember_cake","salt_pouch"]"#);
//! let food = query::run("SELECT SUM(weight * count), COUNT(*) FROM inventory WHERE kind = 'food'", &tables)?;
//! assert_eq!(food.to_json(), "[23,2]");
//! # Ok::<(), gamectx::Error>(())
//! ```
//!
//! Supported: `SELECT` columns or arithmetic on them (`+ - * / %`, `ROUND(x)`,
//! `ROUND(x, places)`, `ABS(x)`), or aggregates `COUNT(*)`, `COUNT(x)`, `SUM(x)`, `AVG(x)`,
//! `MIN(x)`, `MAX(x)` combined with arithmetic (`MAX(trust) - MIN(trust)`); `FROM` one table;
//! `WHERE` conditions joined by `AND`, each `column op value` with `= != <> < <= > >=` or
//! `column IN (values)`; `ORDER BY column [ASC|DESC]`; `LIMIT n`; an optional final `;`.
//! `AS name` after a selected value is accepted and ignored, since results are plain values.
//! Unlike standard SQL, `ORDER BY` and `LIMIT` apply before aggregates, so
//! `SELECT SUM(weight) FROM inventory ORDER BY weight DESC LIMIT 3` adds up the three
//! heaviest items. Keywords and function names ignore case; table and column names do not.
//! Strings are quoted with `'` or `"`, and a bare word on the right of a condition is a
//! string too (`WHERE kind = food`).
//!
//! `WHERE` compares exactly like the [`view`] `where` step, and `ORDER BY` sorts
//! like `sort_by` (stable, nulls last). Selected columns keep their original values; computed
//! numbers are written like formula results (whole numbers without a fraction). Division by
//! zero, non-finite results and arithmetic on missing or non-number cells give `null`;
//! aggregates skip `null`s. Arithmetic uses 64-bit floats; use `ROUND` to trim tails.
//! One selected value gives a list (or a single number for aggregates), several give a list
//! of lists (or a list of numbers).
//!
//! The query is untrusted model output: its length, size and nesting are limited
//! ([`Limits`]), every column it names must exist in every row, and errors say what is
//! supported, so the model can fix its query when the game sends the error back.

use crate::json::{Error, Result, Value};
use crate::view::{self, Op, View};

mod schema;
pub use schema::{describe, describe_with};

/// Limits on a query's text and shape.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Characters of query text.
    pub chars: usize,
    /// Values after `SELECT`.
    pub items: usize,
    /// Conditions after `WHERE`.
    pub conditions: usize,
    /// Values inside one `IN (...)`.
    pub in_values: usize,
    /// Expression nodes in the whole query.
    pub nodes: usize,
    /// Nesting of parentheses, functions and minus signs.
    pub depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { chars: 500, items: 16, conditions: 16, in_values: 64, nodes: 200, depth: 32 }
    }
}

const HINTS: [(&str, &str); 6] = [
    ("or", "OR is not supported; use IN (...) or one query per case"),
    ("group", "GROUP BY is not supported; use WHERE and one query per group"),
    ("having", "HAVING is not supported; use WHERE"),
    ("join", "JOIN is not supported; query one table at a time"),
    ("union", "UNION is not supported; use one query per table"),
    ("select", "subqueries are not supported; ORDER BY and LIMIT apply before aggregates"),
];

fn hint(word: &str) -> Option<Error> {
    let lower = word.to_ascii_lowercase();
    HINTS.iter().find(|(w, _)| *w == lower).map(|(_, h)| Error::new(*h))
}

fn subquery() -> Error {
    hint("select").expect("select hint")
}

// ------------------------------------------------------------------ tokens

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    /// A number and its text, which conditions compare as written.
    Num(f64, String),
    Id(String),
    Str(String),
    Sym(&'static str),
}

const SYMBOLS: [&str; 16] = ["!=", "<>", "<=", ">=", "-", "+", "*", "/", "%", "(", ")", ",", "=", "<", ">", ";"];

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

fn tokenize(text: &str, limits: &Limits) -> Result<Vec<Tok>> {
    if text.chars().take(limits.chars.saturating_add(1)).count() > limits.chars {
        return Err(Error::new(format!("query is longer than {} characters", limits.chars)));
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        while i < chars.len() && is_space(chars[i]) {
            i += 1;
        }
        if i == chars.len() {
            return Ok(out);
        }
        let c = chars[i];
        let start = i;
        if c.is_ascii_digit() {
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if chars.get(i) == Some(&'.') && chars.get(i + 1).is_some_and(char::is_ascii_digit) {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let text: String = chars[start..i].iter().collect();
            let n: f64 = text.parse().map_err(|_| Error::new(format!("bad number \"{text}\"")))?;
            out.push(Tok::Num(n, text));
        } else if c.is_ascii_alphabetic() || c == '_' {
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Tok::Id(chars[start..i].iter().collect()));
        } else if c == '\'' || c == '"' {
            let end =
                chars[i + 1..].iter().position(|&d| d == c).ok_or_else(|| Error::new(format!("unexpected '{c}'")))?;
            out.push(Tok::Str(chars[i + 1..i + 1 + end].iter().collect()));
            i += end + 2;
        } else {
            let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
            let sym =
                SYMBOLS.iter().find(|s| two.starts_with(**s)).ok_or_else(|| Error::new(format!("unexpected '{c}'")))?;
            i += sym.len();
            out.push(Tok::Sym(if *sym == "<>" { "!=" } else { sym }));
        }
    }
}

// ------------------------------------------------------------------ parsing

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Agg {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

#[derive(Clone, Debug)]
enum Expr {
    Num(f64),
    Col(String),
    Neg(Box<Expr>),
    Bin(&'static str, Box<Expr>, Box<Expr>),
    /// `None` is `COUNT(*)`.
    Agg(Agg, Option<Box<Expr>>),
    Abs(Box<Expr>),
    Round(Box<Expr>, i32),
}

impl Expr {
    fn has_agg(&self) -> bool {
        match self {
            Expr::Agg(..) => true,
            Expr::Num(_) | Expr::Col(_) => false,
            Expr::Neg(e) | Expr::Abs(e) | Expr::Round(e, _) => e.has_agg(),
            Expr::Bin(_, a, b) => a.has_agg() || b.has_agg(),
        }
    }

    /// The first column used outside any aggregate.
    fn loose_column(&self) -> Option<&str> {
        match self {
            Expr::Col(c) => Some(c),
            Expr::Num(_) | Expr::Agg(..) => None,
            Expr::Neg(e) | Expr::Abs(e) | Expr::Round(e, _) => e.loose_column(),
            Expr::Bin(_, a, b) => a.loose_column().or_else(|| b.loose_column()),
        }
    }

    fn columns<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            Expr::Num(_) | Expr::Agg(_, None) => {}
            Expr::Col(c) => out.push(c),
            Expr::Neg(e) | Expr::Abs(e) | Expr::Round(e, _) | Expr::Agg(_, Some(e)) => e.columns(out),
            Expr::Bin(_, a, b) => {
                a.columns(out);
                b.columns(out);
            }
        }
    }
}

struct Parser<'a> {
    toks: Vec<Tok>,
    i: usize,
    nodes: usize,
    depth: usize,
    in_agg: bool,
    limits: &'a Limits,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }

    fn keyword(&mut self, word: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Id(w)) if w.eq_ignore_ascii_case(word)) {
            self.i += 1;
            return true;
        }
        false
    }

    fn sym(&mut self, s: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Sym(t)) if *t == s) {
            self.i += 1;
            return true;
        }
        false
    }

    fn expect_keyword(&mut self, word: &str) -> Result<()> {
        if self.keyword(word) {
            Ok(())
        } else {
            Err(Error::new(format!("expected {}", word.to_ascii_uppercase())))
        }
    }

    fn unexpected(&self) -> Error {
        match self.peek() {
            None => Error::new("unexpected end of query"),
            Some(Tok::Id(w)) => hint(w).unwrap_or_else(|| Error::new(format!("unexpected \"{w}\""))),
            Some(Tok::Num(_, t)) => Error::new(format!("unexpected \"{t}\"")),
            Some(Tok::Str(s)) => Error::new(format!("unexpected '{s}'")),
            Some(Tok::Sym(s)) => Error::new(format!("unexpected \"{s}\"")),
        }
    }

    fn ident(&mut self) -> Result<String> {
        match self.peek() {
            Some(Tok::Sym("(")) => Err(subquery()),
            Some(Tok::Id(w)) => {
                let w = w.clone();
                self.i += 1;
                Ok(w)
            }
            _ => Err(Error::new("expected a name")),
        }
    }

    /// Skips `AS name` after a selected value; results have no column names.
    fn alias(&mut self) -> Result<()> {
        if self.keyword("as") {
            self.ident()?;
        }
        Ok(())
    }

    fn node(&mut self, e: Expr) -> Result<Expr> {
        self.nodes += 1;
        if self.nodes > self.limits.nodes {
            return Err(Error::new("query is too complex"));
        }
        Ok(e)
    }

    fn nested(&mut self) -> Result<Expr> {
        self.depth += 1;
        if self.depth > self.limits.depth {
            return Err(Error::new("query nests too deeply"));
        }
        let e = self.expr()?;
        self.depth -= 1;
        Ok(e)
    }

    fn expr(&mut self) -> Result<Expr> {
        let mut left = self.term()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Sym(s @ ("+" | "-"))) => *s,
                _ => return Ok(left),
            };
            self.i += 1;
            let right = self.term()?;
            left = self.node(Expr::Bin(op, Box::new(left), Box::new(right)))?;
        }
    }

    fn term(&mut self) -> Result<Expr> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Sym(s @ ("*" | "/" | "%"))) => *s,
                _ => return Ok(left),
            };
            self.i += 1;
            let right = self.unary()?;
            left = self.node(Expr::Bin(op, Box::new(left), Box::new(right)))?;
        }
    }

    fn unary(&mut self) -> Result<Expr> {
        if !self.sym("-") {
            return self.atom();
        }
        self.depth += 1;
        if self.depth > self.limits.depth {
            return Err(Error::new("query nests too deeply"));
        }
        let inner = self.unary()?;
        let e = self.node(Expr::Neg(Box::new(inner)))?;
        self.depth -= 1;
        Ok(e)
    }

    fn atom(&mut self) -> Result<Expr> {
        match self.peek().cloned() {
            Some(Tok::Num(n, _)) => {
                self.i += 1;
                self.node(Expr::Num(n))
            }
            Some(Tok::Sym("(")) => {
                self.i += 1;
                if matches!(self.peek(), Some(Tok::Id(w)) if w.eq_ignore_ascii_case("select")) {
                    return Err(subquery());
                }
                let e = self.nested()?;
                if !self.sym(")") {
                    return Err(Error::new("expected )"));
                }
                Ok(e)
            }
            Some(Tok::Sym("*")) => Err(Error::new("SELECT * is not supported; name the columns")),
            Some(Tok::Id(name)) => {
                self.i += 1;
                if !self.sym("(") {
                    return self.node(Expr::Col(name));
                }
                self.function(&name)
            }
            _ => Err(self.unexpected()),
        }
    }

    fn function(&mut self, name: &str) -> Result<Expr> {
        let lower = name.to_ascii_lowercase();
        let agg = match lower.as_str() {
            "count" => Some(Agg::Count),
            "sum" => Some(Agg::Sum),
            "avg" => Some(Agg::Avg),
            "min" => Some(Agg::Min),
            "max" => Some(Agg::Max),
            _ => None,
        };
        if let Some(agg) = agg {
            if self.in_agg {
                return Err(Error::new("aggregates cannot be nested"));
            }
            let inner = if agg == Agg::Count && self.sym("*") {
                None
            } else {
                self.in_agg = true;
                let e = self.nested()?;
                self.in_agg = false;
                Some(Box::new(e))
            };
            if !self.sym(")") {
                return Err(Error::new(format!("{} takes one argument", lower.to_ascii_uppercase())));
            }
            return self.node(Expr::Agg(agg, inner));
        }
        match lower.as_str() {
            "abs" => {
                let arg = self.nested()?;
                if !self.sym(")") {
                    return Err(Error::new("ABS takes one argument"));
                }
                self.node(Expr::Abs(Box::new(arg)))
            }
            "round" => {
                let arg = self.nested()?;
                let mut places = 0;
                if self.sym(",") {
                    match self.peek() {
                        Some(Tok::Num(n, _)) if n.fract() == 0.0 && *n <= 12.0 => places = *n as i32,
                        _ => return Err(Error::new("ROUND places must be a whole number from 0 to 12")),
                    }
                    self.i += 1;
                }
                if !self.sym(")") {
                    return Err(Error::new("ROUND takes a value and optional places"));
                }
                self.node(Expr::Round(Box::new(arg), places))
            }
            _ => Err(Error::new(format!("unknown function {name}; use COUNT, SUM, AVG, MIN, MAX, ROUND or ABS"))),
        }
    }

    /// A value on the right of a condition: a number, a string, or a bare word as a string.
    fn literal(&mut self) -> Result<Value> {
        let negative = self.sym("-");
        let value = match self.peek() {
            Some(Tok::Num(_, text)) => Value::Number(if negative { format!("-{text}") } else { text.clone() }),
            _ if negative => return Err(Error::new("expected a number after -")),
            Some(Tok::Id(w)) if w.eq_ignore_ascii_case("select") => return Err(subquery()),
            Some(Tok::Sym("(")) if matches!(self.toks.get(self.i + 1), Some(Tok::Id(w)) if w.eq_ignore_ascii_case("select")) => {
                return Err(subquery())
            }
            Some(Tok::Str(s) | Tok::Id(s)) => Value::str(s.as_str()),
            _ => return Err(Error::new("expected a value")),
        };
        self.i += 1;
        Ok(value)
    }

    fn condition(&mut self) -> Result<(String, Op, Value)> {
        let column = self.ident()?;
        if self.keyword("in") {
            if !self.sym("(") {
                return Err(Error::new("IN needs a list in parentheses"));
            }
            let mut options = Vec::new();
            loop {
                options.push(self.literal()?);
                if options.len() > self.limits.in_values {
                    return Err(Error::new(format!("IN takes at most {} values", self.limits.in_values)));
                }
                if !self.sym(",") {
                    break;
                }
            }
            if !self.sym(")") {
                return Err(Error::new("expected )"));
            }
            return Ok((column, Op::In, Value::Array(options)));
        }
        let op = match self.peek() {
            Some(Tok::Sym(s @ ("=" | "!=" | "<" | "<=" | ">" | ">="))) => Op::parse(s)?,
            _ => return Err(Error::new(format!("expected a comparison after {column}"))),
        };
        self.i += 1;
        Ok((column, op, self.literal()?))
    }
}

/// A parsed query, ready to run against one table.
#[derive(Clone, Debug)]
pub struct Query {
    items: Vec<Expr>,
    table: String,
    view: View,
}

impl Query {
    /// Parses a query with the default [`Limits`].
    pub fn parse(text: &str) -> Result<Query> {
        Query::parse_with(text, &Limits::default())
    }

    /// Parses a query, refusing anything over `limits`.
    pub fn parse_with(text: &str, limits: &Limits) -> Result<Query> {
        let mut p = Parser { toks: tokenize(text, limits)?, i: 0, nodes: 0, depth: 0, in_agg: false, limits };
        p.expect_keyword("select")?;
        let mut items = Vec::new();
        loop {
            items.push(p.expr()?);
            if items.len() > limits.items {
                return Err(Error::new(format!("at most {} selected values", limits.items)));
            }
            p.alias()?;
            if !p.sym(",") {
                break;
            }
        }
        p.expect_keyword("from")?;
        let table = p.ident()?;
        let mut view = View::default();
        if p.keyword("where") {
            loop {
                view.filters.push(p.condition()?);
                if view.filters.len() > limits.conditions {
                    return Err(Error::new(format!("at most {} conditions", limits.conditions)));
                }
                if !p.keyword("and") {
                    break;
                }
            }
        }
        if p.keyword("order") {
            p.expect_keyword("by")?;
            let column = p.ident()?;
            let descending = p.keyword("desc");
            if !descending {
                p.keyword("asc");
            }
            view.sort_by = Some((column, descending));
        }
        if p.keyword("limit") {
            match p.peek() {
                Some(Tok::Num(n, _)) if n.fract() == 0.0 && *n < 4_294_967_296.0 => view.limit = Some(*n as usize),
                _ => return Err(Error::new("LIMIT needs a whole number")),
            }
            p.i += 1;
        }
        p.sym(";");
        if p.i != p.toks.len() {
            return Err(p.unexpected());
        }
        if items.iter().any(Expr::has_agg) {
            if let Some(column) = items.iter().find_map(Expr::loose_column) {
                return Err(Error::new(format!(
                    "column {column} must be inside an aggregate such as SUM(...), or select it in a separate query"
                )));
            }
        }
        Ok(Query { items, table, view })
    }

    /// The table this query reads.
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Runs the query over the rows of its table.
    pub fn run(&self, rows: &[Value]) -> Result<Value> {
        if rows.iter().any(|row| row.as_object().is_none()) {
            return Err(Error::new("table rows must be objects"));
        }
        let mut used: Vec<&str> = Vec::new();
        for item in &self.items {
            item.columns(&mut used);
        }
        used.extend(self.view.filters.iter().map(|(c, _, _)| c.as_str()));
        used.extend(self.view.sort_by.iter().map(|(c, _)| c.as_str()));
        for column in used {
            if rows.iter().any(|row| row.get(column).is_none()) {
                let names: Vec<&str> = rows
                    .first()
                    .and_then(Value::as_object)
                    .map(|o| o.iter().map(|(k, _)| k.as_str()).collect())
                    .unwrap_or_default();
                return Err(Error::new(format!(
                    "no column \"{column}\" in table \"{}\"; columns: {}",
                    self.table,
                    names.join(", ")
                )));
            }
        }
        let rows = view::apply(rows, &self.view)?;
        let pick = |values: Vec<Value>| {
            if values.len() == 1 {
                values.into_iter().next().unwrap()
            } else {
                Value::Array(values)
            }
        };
        if self.items.iter().any(Expr::has_agg) {
            return Ok(pick(self.items.iter().map(|e| number(eval(e, None, &rows))).collect()));
        }
        let out = rows
            .iter()
            .map(|row| {
                pick(
                    self.items
                        .iter()
                        .map(|e| match e {
                            Expr::Col(c) => row.get(c).cloned().unwrap_or(Value::Null),
                            _ => number(eval(e, Some(row), &[])),
                        })
                        .collect(),
                )
            })
            .collect();
        Ok(Value::Array(out))
    }
}

fn cell(row: &Value, column: &str) -> Option<f64> {
    match row.get(column) {
        Some(Value::Number(t)) => t.parse().ok(),
        _ => None,
    }
}

/// Evaluates per row (`row` set) or over all `rows` (aggregates). `None` is null.
fn eval(e: &Expr, row: Option<&Value>, rows: &[Value]) -> Option<f64> {
    let value = match e {
        Expr::Num(n) => Some(*n),
        Expr::Col(c) => row.and_then(|r| cell(r, c)),
        Expr::Neg(e) => eval(e, row, rows).map(|v| -v),
        Expr::Bin(op, a, b) => {
            let (a, b) = (eval(a, row, rows)?, eval(b, row, rows)?);
            match *op {
                "+" => Some(a + b),
                "-" => Some(a - b),
                "*" => Some(a * b),
                _ if b == 0.0 => None,
                "/" => Some(a / b),
                _ => Some(a % b),
            }
        }
        Expr::Abs(e) => eval(e, row, rows).map(f64::abs),
        Expr::Round(e, places) => {
            let scale = 10f64.powi(*places);
            eval(e, row, rows).map(|v| (v * scale).round() / scale)
        }
        Expr::Agg(_, None) => Some(rows.len() as f64),
        Expr::Agg(Agg::Count, Some(inner)) => Some(match &**inner {
            Expr::Col(c) => rows.iter().filter(|r| !matches!(r.get(c), Some(Value::Null) | None)).count(),
            _ => rows.iter().filter(|r| eval(inner, Some(r), &[]).is_some()).count(),
        } as f64),
        Expr::Agg(agg, Some(inner)) => {
            let values: Vec<f64> = rows.iter().filter_map(|r| eval(inner, Some(r), &[])).collect();
            let sum = || values.iter().fold(0.0, |total, v| total + v);
            match agg {
                Agg::Sum => Some(sum()),
                _ if values.is_empty() => None,
                Agg::Avg => Some(sum() / values.len() as f64),
                Agg::Min => Some(values[1..].iter().fold(values[0], |best, &v| if v < best { v } else { best })),
                _ => Some(values[1..].iter().fold(values[0], |best, &v| if v > best { v } else { best })),
            }
        }
    };
    value.filter(|v| v.is_finite())
}

/// A computed number as JSON: whole numbers without a fraction, and `null` for missing or
/// non-finite results, as formula results are written.
fn number(v: Option<f64>) -> Value {
    match v {
        Some(n) if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 => Value::from(n as i64),
        Some(n) => Value::from(n),
        None => Value::Null,
    }
}

/// Parses and runs a model's query over the named tables. Errors name the tables or
/// columns that exist, so they can be sent back to the model as they are.
pub fn run(text: &str, tables: &[(&str, &[Value])]) -> Result<Value> {
    let query = Query::parse(text)?;
    let rows = tables.iter().find(|(name, _)| *name == query.table).map(|(_, rows)| *rows).ok_or_else(|| {
        let names: Vec<&str> = tables.iter().map(|(n, _)| *n).collect();
        Error::new(format!("no table \"{}\"; tables: {}", query.table, names.join(", ")))
    })?;
    query.run(rows)
}

/// What the model is told about queries: the instruction measured in `eval/query_eval.py`.
pub const DESCRIPTION: &str = "One SELECT over one table of the game data, which the game runs exactly. \
Supported: SELECT columns, COUNT(*), SUM(x), AVG(x), MIN(x), MAX(x), where x may be arithmetic on columns; \
FROM table; WHERE conditions joined by AND (= != < <= > >= IN); ORDER BY column [DESC]; LIMIT n, applied \
before aggregates.";

/// The `query` tool for the model, strict so it packs as a signature:
/// `query(sql): Ask the game for exact rows or totals...`.
pub fn tool() -> Value {
    crate::obj! {"type" => "function", "function" => crate::obj! {
    "name" => "query",
    "description" => "Ask the game for exact rows or totals from its tables instead of reading or adding them up yourself.",
    "strict" => true,
    "parameters" => crate::obj! {"type" => "object",
        "properties" => crate::obj! {"sql" => crate::obj! {
            "type" => "string", "maxLength" => Limits::default().chars, "description" => DESCRIPTION}},
        "required" => vec!["sql"], "additionalProperties" => false}}}
}

/// Guidance returned after the first invalid query; measured with the schema-only eval.
pub const RETRY_HINT: &str = "Use only the tables and columns below. No subqueries; one SELECT only. \
For the largest row use ORDER BY column DESC LIMIT 1. \
ORDER BY and LIMIT apply before SUM/AVG/MIN/MAX, including top-N sums.";

/// One query and at most one correction. A successful execution ends the session too:
/// semantic mistakes in valid SQL must not trigger an automatic retry.
///
/// Start a new session for each independent question. Keep the same table snapshot for
/// both attempts. Returned values are protocol envelopes: `{"result": ...}` or
/// `{"error": ..., "retry": bool}`. The first error includes table schemas and a hint;
/// no further requests are accepted after success or the second failure. The host runs
/// its own model/API loop and sends these envelopes back as untrusted tool data.
///
/// ```
/// use gamectx::{obj, query};
/// let rows = vec![obj! {"unit_weight" => 0.25, "count" => 2}];
/// let tables = [("inventory", rows.as_slice())];
/// let mut session = query::Session::new().result_bytes(512);
/// let feedback = session.apply_call(&obj! {"sql" => "SELECT COUNT(*) FROM food"}, &tables)?;
/// assert_eq!(feedback.get("retry").and_then(gamectx::Value::as_bool), Some(true));
/// let result = session.apply_call(&obj! {"sql" => "SELECT SUM(unit_weight * count) FROM inventory"}, &tables)?;
/// assert_eq!(result.to_json(), r#"{"result":0.5}"#);
/// assert!(session.is_finished());
/// # Ok::<(), gamectx::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct Session {
    attempts: usize,
    finished: bool,
    result_bytes: usize,
}

impl Default for Session {
    fn default() -> Self {
        Self { attempts: 0, finished: false, result_bytes: 4096 }
    }
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Limit the compact result's UTF-8 bytes (default 4096), excluding its envelope.
    /// Oversize results receive the same one-correction allowance. Execution still scales
    /// with the host's table size; this is an output limit, not a table/work limit.
    pub fn result_bytes(mut self, bytes: usize) -> Self {
        self.result_bytes = bytes;
        self
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Validates the query tool's argument object (exactly one string field, `sql`).
    /// Malformed arguments consume an attempt and receive the same correction feedback.
    pub fn apply_call(&mut self, args: &Value, tables: &[(&str, &[Value])]) -> Result<Value> {
        let sql = args
            .as_object()
            .filter(|entries| entries.len() == 1)
            .and_then(|_| args.get("sql"))
            .and_then(Value::as_str)
            .unwrap_or("");
        self.submit(sql, tables)
    }

    /// Executes a model's SQL, returning a result or bounded correction feedback.
    /// Host table-description errors close the session and return `Err`. Query failures
    /// are envelopes; `Err` also refuses any submission after the session has finished.
    pub fn submit(&mut self, sql: &str, tables: &[(&str, &[Value])]) -> Result<Value> {
        if self.finished {
            return Err(Error::new("query session is finished"));
        }
        self.attempts += 1;
        let result = run(sql, tables).and_then(|value| {
            if value.to_json().len() > self.result_bytes {
                Err(Error::new(format!(
                    "query result exceeds {} bytes; select fewer columns or add LIMIT",
                    self.result_bytes
                )))
            } else {
                Ok(value)
            }
        });
        match result {
            Ok(value) => {
                self.finished = true;
                Ok(crate::obj! {"result" => value})
            }
            Err(error) => {
                let text = error.to_string();
                let message = if text.starts_with("no table ") {
                    "Unknown table. Use a table name listed below."
                } else if text.starts_with("no column ") {
                    "Unknown column. Use columns from the selected table."
                } else if text.starts_with("query result exceeds ") {
                    &text
                } else {
                    "Query must use the supported SELECT subset."
                };
                let retry = self.attempts < 2;
                self.finished = !retry;
                let mut reply =
                    vec![("error".to_owned(), Value::str(message)), ("retry".to_owned(), Value::Bool(retry))];
                if retry {
                    let schema = describe(tables).inspect_err(|_| {
                        self.finished = true;
                    })?;
                    reply.push(("tables".to_owned(), schema));
                    reply.push(("hint".to_owned(), Value::str(RETRY_HINT)));
                }
                Ok(Value::Object(reply))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    fn items() -> Value {
        parse(
            r#"[{"id":"a","kind":"food","count":3,"weight":0.5,"note":null},
                {"id":"b","kind":"tool","count":18,"weight":2.25,"note":"x"},
                {"id":"c","kind":"food","count":15,"weight":1.0,"note":null}]"#,
            1 << 16,
        )
        .unwrap()
    }

    fn q(text: &str) -> Result<String> {
        let items = items();
        run(text, &[("items", items.as_array().unwrap())]).map(|v| v.to_json())
    }

    #[test]
    fn selects_filters_sorts_and_limits() {
        assert_eq!(q("select id from items where count >= 15").unwrap(), r#"["b","c"]"#);
        assert_eq!(
            q("SELECT id, weight FROM items WHERE kind = food ORDER BY weight DESC").unwrap(),
            r#"[["c",1.0],["a",0.5]]"#
        );
        assert_eq!(q("SELECT id FROM items WHERE kind IN ('tool', \"food\") AND count < 10;").unwrap(), r#"["a"]"#);
        assert_eq!(q("SELECT id FROM items ORDER BY id DESC LIMIT 1").unwrap(), r#"["c"]"#);
        assert_eq!(q("SELECT id FROM items WHERE count <> 3 AND weight > -1").unwrap(), r#"["b","c"]"#);
        assert_eq!(q("SELECT id FROM items WHERE count > 99").unwrap(), "[]");
        assert_eq!(q("SELECT id FROM items ORDER BY note").unwrap(), r#"["b","a","c"]"#);
        assert_eq!(q("SELECT MAX(count) - MIN(count) AS spread, COUNT(*) AS n FROM items").unwrap(), "[15,3]");
    }

    #[test]
    fn aggregates_apply_after_limit() {
        assert_eq!(q("SELECT SUM(weight) FROM items ORDER BY weight DESC LIMIT 2").unwrap(), "3.25");
        assert_eq!(q("SELECT MAX(count) - MIN(count), AVG(count), COUNT(*) FROM items").unwrap(), "[15,12,3]");
        assert_eq!(q("SELECT COUNT(note), COUNT(note * 2), SUM(count * weight) FROM items").unwrap(), "[1,0,57]");
        assert_eq!(
            q("SELECT SUM(count), AVG(count), MIN(count) FROM items WHERE count > 99").unwrap(),
            "[0,null,null]"
        );
    }

    #[test]
    fn arithmetic_per_row() {
        assert_eq!(
            q("SELECT ROUND(weight * 1.5, 1), count % 4, -count / 0 FROM items LIMIT 1").unwrap(),
            "[[0.8,3,null]]"
        );
        assert_eq!(q("SELECT ABS(1 - count) FROM items").unwrap(), "[2,17,14]");
    }

    #[test]
    fn errors_say_what_exists() {
        let err = |text: &str| q(text).unwrap_err().to_string();
        assert!(err("SELECT id FROM food").contains("tables: items"));
        assert!(err("SELECT name FROM items").contains("columns: id, kind, count, weight, note"));
        assert!(err("SELECT * FROM items").contains("name the columns"));
        assert!(err("SELECT id FROM items WHERE kind = food OR count > 1").contains("IN"));
        assert!(err("SELECT id, SUM(count) FROM items").contains("inside an aggregate"));
        assert!(err("SELECT SUM(weight) FROM items WHERE id IN (SELECT id FROM items)").contains("subqueries"));
        assert!(err("SELECT id FROM items WHERE count = (SELECT MAX(count) FROM items)").contains("subqueries"));
        assert!(err("SELECT kind, COUNT(*) FROM items GROUP BY kind").contains("GROUP BY"));
        assert!(err("SELECT COUNT(*) FROM items GROUP BY kind").contains("GROUP BY"));
        assert!(err("SELECT SUM(MAX(count)) FROM items").contains("nested"));
        assert!(err("SELECT id FROM items LIMIT 2.5").contains("LIMIT"));
        assert!(err("SELECT id FROM items WHERE id = 'open").contains("unexpected"));
    }

    #[test]
    fn limits_are_enforced() {
        let long = format!("SELECT id FROM items WHERE id = '{}'", "x".repeat(500));
        assert!(q(&long).unwrap_err().to_string().contains("longer"));
        let deep = format!("SELECT {}1{} FROM items", "(".repeat(40), ")".repeat(40));
        assert!(q(&deep).unwrap_err().to_string().contains("deeply"));
        let negs = format!("SELECT {}1 FROM items", "-".repeat(40));
        assert!(q(&negs).unwrap_err().to_string().contains("deeply"));
        let wide = format!("SELECT {} FROM items", vec!["1+1"; 70].join("+"));
        assert!(q(&wide).unwrap_err().to_string().contains("complex"));
        for limits in [
            Limits { chars: 0, ..Limits::default() },
            Limits { items: 0, ..Limits::default() },
            Limits { nodes: 0, ..Limits::default() },
        ] {
            assert!(Query::parse_with("SELECT 1 FROM items", &limits).is_err());
        }
        let limits = Limits { in_values: 0, ..Limits::default() };
        assert!(Query::parse_with("SELECT id FROM items WHERE id IN ('a')", &limits).is_err());
        let limits = Limits { conditions: 0, ..Limits::default() };
        assert!(Query::parse_with("SELECT id FROM items WHERE id = 'a'", &limits).is_err());
        let limits = Limits { depth: 0, ..Limits::default() };
        assert!(Query::parse_with("SELECT ABS(1) FROM items", &limits).is_err());
    }

    #[test]
    fn invalid_rows_and_nonfinite_arithmetic() {
        assert!(run("SELECT 1 FROM items", &[("items", &[Value::Null])]).is_err());
        let rows = parse(r#"[{"x":1e308},{"x":-1e308},{"x":1e999}]"#, 1024).unwrap();
        let tables = [("items", rows.as_array().unwrap())];
        assert_eq!(run("SELECT x * 10 FROM items", &tables).unwrap().to_json(), "[null,null,null]");
        assert_eq!(run("SELECT COUNT(x * 10), MIN(x * 10) FROM items", &tables).unwrap().to_json(), "[0,null]");
        assert_eq!(run("SELECT SUM(x), AVG(x) FROM items", &tables).unwrap().to_json(), "[0,0]");
        assert_eq!(q("SELECT ROUND(-0.5), ROUND(0.5), 1 / 0 FROM items LIMIT 1").unwrap(), "[[-1,1,null]]");
    }

    #[test]
    fn tool_packs_as_a_signature() {
        let text = crate::signatures::pack(&[tool()]).unwrap();
        assert!(text.contains("\nquery(sql): Ask the game"), "{text}");
    }

    #[test]
    fn sessions_stop_after_success_or_second_failure() {
        let items = items();
        let tables = [("items", items.as_array().unwrap())];
        let mut session = Session::new();
        let first = session.submit("SELECT id FROM food", &tables).unwrap();
        assert_eq!(first.get("retry"), Some(&Value::Bool(true)));
        assert!(first.get("tables").unwrap().get("items").is_some());
        let corrected = session.submit("SELECT COUNT(*) FROM items", &tables).unwrap();
        assert_eq!(corrected.to_json(), r#"{"result":3}"#);
        assert!(session.is_finished());
        assert!(session.submit("SELECT 1 FROM items", &tables).is_err());
        let mut session = Session::new();
        for retry in [true, false] {
            let reply = session.apply_call(&crate::obj! {"sql" => 3}, &tables).unwrap();
            assert_eq!(reply.get("retry"), Some(&Value::Bool(retry)));
        }
        assert!(session.is_finished());
        let mut session = Session::new();
        assert_eq!(session.submit("SELECT SUM(count) FROM items", &tables).unwrap().to_json(), r#"{"result":36}"#);
        assert!(session.is_finished(), "valid SQL ends the session regardless of intended meaning");
    }

    #[test]
    fn session_budgets_result_bytes_and_validates_arguments() {
        let items = items();
        let tables = [("items", items.as_array().unwrap())];
        let mut session = Session::new().result_bytes(5);
        let first = session.submit("SELECT id FROM items", &tables).unwrap();
        assert_eq!(first.get("retry"), Some(&Value::Bool(true)));
        assert!(first.get("error").unwrap().as_str().unwrap().contains("LIMIT"));
        assert_eq!(session.submit("SELECT id FROM items LIMIT 1", &tables).unwrap().to_json(), r#"{"result":["a"]}"#);
        let mut session = Session::new();
        let call = crate::obj! {"sql" => "SELECT COUNT(*) FROM items", "extra" => true};
        assert_eq!(session.apply_call(&call, &tables).unwrap().get("retry"), Some(&Value::Bool(true)));
        assert_eq!(
            session.apply_call(&crate::obj! {"sql" => "SELECT 1 FROM items LIMIT 1"}, &tables).unwrap().to_json(),
            r#"{"result":[1]}"#
        );
        let mut session = Session::new();
        assert!(session.submit("bad", &[("bad name", &[])]).is_err());
        assert!(session.is_finished(), "bad host schemas must close the session");
    }
}
