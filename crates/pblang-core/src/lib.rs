// pub mod lexer; // REMOVED: Fix #10 — lexer.rs was dead code (never used)
pub mod parser;

use chumsky::{prelude::Simple, Parser};

use crate::parser::parser;

use std::collections::HashMap;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl From<Range<usize>> for Span {
    fn from(r: Range<usize>) -> Self {
        Span {
            start: r.start,
            end: r.end,
        }
    }
}

impl Span {
    pub fn join(a: Span, b: Span) -> Span {
        Span {
            start: a.start.min(b.start),
            end: a.end.max(b.end),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolyglotBlock {
    pub lang: String,
    pub name: Option<String>,
    /// Raw `(...)` argument payload (without the surrounding parens).
    pub args: Option<String>,
    /// Parsed key/value attributes from `args` (best-effort).
    pub attrs: HashMap<String, String>,
    pub code: String,
    pub span: Span, // span of the whole `@lang ... { ... }` region
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeExpr {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Any,
    Number,
    /// Arbitrary-precision integer (`bigint`).
    BigInt,
    /// Exact arbitrary-precision rational (`rational`).
    Rational,
    /// Arbitrary-precision base-10 decimal (`decimal`).
    Decimal,
    Bool,
    String,
    Nil,
    Array(Box<TypeExpr>),
    /// map<K, V> — key and value type parameters.
    Map(Box<TypeExpr>, Box<TypeExpr>),
    /// T? — optional / nullable type (T | nil).
    Option(Box<TypeExpr>),
    /// result<T, E> — success (T) or error (E).
    Result(Box<TypeExpr>, Box<TypeExpr>),
    /// Generic type parameter name (e.g. T, K, V) — placeholder for monomorphisation.
    Generic(String),
    /// T! — non-nullable variant; compiler enforces no nil assignment.
    NonNull(Box<TypeExpr>),
}

/// A function parameter definition (name, optional type annotation, optional default value).
/// - name: the parameter name
/// - ty: optional type annotation (e.g. `: number`)
/// - default: optional default expression (e.g. `= 42`), used when arg is not provided
/// - is_vararg: if true, this is the `...name` rest parameter (must be last)
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Option<TypeExpr>,
    pub default: Option<Expr>,
    pub is_vararg: bool,
}

impl Param {
    pub fn simple(name: String, ty: Option<TypeExpr>) -> Self {
        Param {
            name,
            ty,
            default: None,
            is_vararg: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Number(f64),
    Str(String),
    Bool(bool),
    Nil,
    Var(String),
    Call(Box<Expr>, Vec<Expr>),
    GetAttr(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Array(Vec<Expr>),
    Map(Vec<(String, Expr)>), // map{"k": v, foo: v} (keys are strings/idents)
    Try(Box<Expr>, Box<Expr>), // try expr catch expr
    FnLit(Option<String>, Vec<Param>, Option<TypeExpr>, Vec<Stmt>),
    BlockExpr(Vec<Stmt>, Option<Box<Expr>>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    NotEq(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Ge(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Le(Box<Expr>, Box<Expr>),
    Logical {
        left: Box<Expr>,
        op: LogicalOp,
        right: Box<Expr>,
    },
    Match(Box<Expr>, Vec<(MatchArm, Expr)>, Option<Box<Expr>>), // scrutinee, arms (pattern, body), else

    // --- Safety / Rust-inspired operators ---
    /// `expr?.field` — nil-safe attribute access; returns nil if expr is nil.
    SafeGetAttr(Box<Expr>, String),
    /// `expr?[idx]` — nil-safe index; returns nil if expr is nil.
    SafeIndex(Box<Expr>, Box<Expr>),
    /// `expr?` — Result unwrap: if expr is `{ok:false,...}` throw runtime error, else unwrap `.value`.
    ResultUnwrap(Box<Expr>),
    /// `!expr` — logical NOT.
    Not(Box<Expr>),
    /// `a ?? b` — null coalescing: if a is nil, evaluate b, otherwise a.
    NullCoalesce(Box<Expr>, Box<Expr>),
    /// `f"Hello {name}"` — f-string interpolation: Vec of (literal, optional expr).
    /// Each part is either a literal string segment or an interpolated expression.
    FString(Vec<FStringPart>),
    /// `a ** b` — exponentiation (a^b), sugar for num_pow(a, b).
    Pow(Box<Expr>, Box<Expr>),
    /// `a % b` — modulo, sugar for num_mod(a, b).
    Mod(Box<Expr>, Box<Expr>),
    /// `if cond { then } else { else }` — expression-level conditional.
    IfExpr(Box<Expr>, Box<Expr>, Box<Expr>),
}

/// Part of an f-string interpolation.
#[derive(Debug, Clone, PartialEq)]
pub enum FStringPart {
    /// A literal string segment (not interpolated).
    Literal(String),
    /// An interpolated expression `{expr}`.
    Interp(Box<Expr>),
}

/// Pattern in a match arm: literal value, bind to name, or wildcard.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchArm {
    Literal(Expr),
    Bind(String),
    Wildcard,
    BindGuard(String, Expr), // bind name + guard condition (e.g. `case n if n < 0: ...`)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LogicalOp {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    Print(Expr),
    PrintF(usize, Expr),
    Import(String),
    Export(Vec<String>),
    Let(String, Option<TypeExpr>, Expr),
    /// `const name = expr;` — immutable binding; compiler prevents reassignment.
    Const(String, Option<TypeExpr>, Expr),
    Assign(String, Expr),
    SetIndex(Expr, Expr, Expr),  // target[index] = value
    SetAttr(Expr, String, Expr), // target.attr = value   (sugar for map/object set)
    Expr(Expr),                  // expression statement (evaluate + discard)
    If(Expr, Vec<Stmt>, Option<Vec<Stmt>>),
    While(Expr, Vec<Stmt>),
    For(String, Expr, Vec<Stmt>),
    /// `for x in expr { body }` — iterate over array elements or map keys
    ForIn(String, Expr, Vec<Stmt>),
    /// `for k, v in expr { body }` — iterate over map key-value pairs
    ForInKV(String, String, Expr, Vec<Stmt>),
    /// `loop { body }` — infinite loop; requires `break` to exit
    Loop(Vec<Stmt>),
    /// `break;` — exit the nearest enclosing loop
    Break,
    /// `continue;` — jump to the next iteration of the nearest enclosing loop
    Continue,
    /// `throw expr;` — raise a runtime error with the given value/message
    Throw(Expr),
    Fn(String, Vec<Param>, Option<TypeExpr>, Vec<Stmt>),
    Return(Option<Expr>),
    Defer(Expr), // run expr when leaving current scope (function return)
    Block(Vec<Stmt>),
    /// Flat sequence of statements at the SAME scope level (no new scope).
    /// Used by destructuring desugaring: let [a,b]=arr expands to multiple
    /// `let` statements that must be visible in the enclosing scope.
    Seq(Vec<Stmt>),
}

/// Mask `//` and `/* */` comments in `src` with spaces (length-preserving).
///
/// FIX #17: This function and `mask_and_extract_polyglot_blocks` share the same
/// comment-detection logic. The duplicate was retained for now because each
/// function operates on a different buffer (`src` bytes vs. a mutable Vec<u8>),
/// but they should eventually be unified into a shared byte-level helper.
///
/// Both correctly handle:
///  - Nested block comments `/* /* … */ */`
///  - Single-quoted and double-quoted strings (escape sequences aware)
///  - Newline preservation (so line-number spans remain stable)
///
/// Replace every comment byte with a space while leaving string literals,
/// newlines, and all other byte offsets untouched. Used by the CLI and the
/// parser front-end so spans computed against the masked source still line up
/// with the original text. Handles `//` line comments and nested `/* */`
/// block comments.
pub fn strip_comments_mask(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = bytes.to_vec(); // same length as input (byte offsets preserved)
    let mut i = 0usize;
    let mut in_double_string = false;
    let mut in_single_string = false;
    let mut block_depth: i32 = 0;

    while i < out.len() {
        let b = out[i];

        // Inside string: only care about escapes and matching close quote.
        if in_double_string || in_single_string {
            if b == b'\\' {
                i = i.saturating_add(2);
                continue;
            }
            if in_double_string && b == b'"' {
                in_double_string = false;
            } else if in_single_string && b == b'\'' {
                in_single_string = false;
            }
            i += 1;
            continue;
        }

        // Inside block comment (nested): mask everything except newlines
        if block_depth > 0 {
            if i + 1 < out.len() && out[i] == b'/' && out[i + 1] == b'*' {
                out[i] = b' ';
                out[i + 1] = b' ';
                block_depth += 1;
                i += 2;
                continue;
            }
            if i + 1 < out.len() && out[i] == b'*' && out[i + 1] == b'/' {
                out[i] = b' ';
                out[i + 1] = b' ';
                block_depth -= 1;
                i += 2;
                continue;
            }
            if out[i] != b'\n' {
                out[i] = b' ';
            }
            i += 1;
            continue;
        }

        // Not in string/comment
        if b == b'"' {
            in_double_string = true;
            i += 1;
            continue;
        }
        if b == b'\'' {
            in_single_string = true;
            i += 1;
            continue;
        }

        // line comment //
        if i + 1 < out.len() && out[i] == b'/' && out[i + 1] == b'/' {
            out[i] = b' ';
            out[i + 1] = b' ';
            i += 2;
            while i < out.len() && out[i] != b'\n' {
                out[i] = b' ';
                i += 1;
            }
            continue;
        }

        // block comment /*
        if i + 1 < out.len() && out[i] == b'/' && out[i + 1] == b'*' {
            out[i] = b' ';
            out[i + 1] = b' ';
            block_depth += 1;
            i += 2;
            continue;
        }

        i += 1;
    }

    String::from_utf8(out.clone()).unwrap_or_else(|_| String::from_utf8_lossy(&out).to_string())
}

fn mask_and_extract_polyglot_blocks(src: &str) -> (String, Vec<PolyglotBlock>) {
    let bytes = src.as_bytes();
    let mut out = bytes.to_vec(); // same length as input (byte offsets preserved)
    let mut blocks: Vec<PolyglotBlock> = vec![];

    fn is_ident_start(b: u8) -> bool {
        (b as char).is_ascii_alphabetic() || b == b'_'
    }

    fn is_ident_cont(b: u8) -> bool {
        (b as char).is_ascii_alphanumeric() || b == b'_'
    }

    fn skip_ws(out: &[u8], mut idx: usize) -> usize {
        while idx < out.len() && matches!(out[idx], b' ' | b'\t' | b'\r' | b'\n') {
            idx += 1;
        }
        idx
    }

    fn unquote_attr_value(s: &str) -> String {
        let ss = s.trim();
        if ss.len() >= 2 && ss.starts_with('"') && ss.ends_with('"') {
            let inner = &ss[1..ss.len() - 1];
            let mut out = String::with_capacity(inner.len());
            let mut chars = inner.chars();
            while let Some(ch) = chars.next() {
                if ch == '\\' {
                    match chars.next() {
                        Some('n') => out.push('\n'),
                        Some('r') => out.push('\r'),
                        Some('t') => out.push('\t'),
                        Some('\\') => out.push('\\'),
                        Some('"') => out.push('"'),
                        Some(other) => out.push(other),
                        None => break,
                    }
                } else {
                    out.push(ch);
                }
            }
            out
        } else {
            ss.to_string()
        }
    }

    fn parse_polyglot_attrs(args: &str) -> HashMap<String, String> {
        // Best-effort parser for `k=v, flag, foo="bar"` (supports quotes and basic nesting).
        let mut out = HashMap::<String, String>::new();
        let mut cur = String::new();

        let mut in_dq = false;
        let mut prev_bs = false;
        let mut depth_paren: i32 = 0;
        let mut depth_brack: i32 = 0;
        let mut depth_brace: i32 = 0;

        let flush = |s: &mut String, out: &mut HashMap<String, String>| {
            let item = s.trim();
            if item.is_empty() {
                s.clear();
                return;
            }
            if let Some((k, v)) = item.split_once('=') {
                out.insert(k.trim().to_string(), unquote_attr_value(v));
            } else {
                out.insert(item.to_string(), "true".to_string());
            }
            s.clear();
        };

        for ch in args.chars() {
            if in_dq {
                cur.push(ch);
                if ch == '"' && !prev_bs {
                    in_dq = false;
                }
                prev_bs = ch == '\\' && !prev_bs;
                continue;
            }

            match ch {
                '"' => {
                    in_dq = true;
                    prev_bs = false;
                    cur.push(ch);
                }
                '(' => {
                    depth_paren += 1;
                    cur.push(ch);
                }
                ')' => {
                    depth_paren -= 1;
                    cur.push(ch);
                }
                '[' => {
                    depth_brack += 1;
                    cur.push(ch);
                }
                ']' => {
                    depth_brack -= 1;
                    cur.push(ch);
                }
                '{' => {
                    depth_brace += 1;
                    cur.push(ch);
                }
                '}' => {
                    depth_brace -= 1;
                    cur.push(ch);
                }
                ',' if depth_paren == 0 && depth_brack == 0 && depth_brace == 0 => {
                    flush(&mut cur, &mut out);
                }
                _ => cur.push(ch),
            }
        }
        flush(&mut cur, &mut out);
        out
    }

    let mut i = 0usize;
    let mut in_string = false;
    let mut prev_was_backslash = false;
    let mut block_comment_depth: i32 = 0;

    while i < out.len() {
        let b = out[i];
        let next = if i + 1 < out.len() {
            Some(out[i + 1])
        } else {
            None
        };

        if in_string {
            if b == b'"' && !prev_was_backslash {
                in_string = false;
            }
            prev_was_backslash = b == b'\\' && !prev_was_backslash;
            i += 1;
            continue;
        }

        if block_comment_depth > 0 {
            if next == Some(b'*') && b == b'/' {
                block_comment_depth += 1;
                i += 2;
                continue;
            }
            if next == Some(b'/') && b == b'*' {
                block_comment_depth -= 1;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        // start string
        if b == b'"' {
            in_string = true;
            prev_was_backslash = false;
            i += 1;
            continue;
        }

        // line comment //
        if b == b'/' && next == Some(b'/') {
            i += 2;
            while i < out.len() && out[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        // block comment /* ... */ (nested)
        if b == b'/' && next == Some(b'*') {
            block_comment_depth = 1;
            i += 2;
            continue;
        }

        // polyglot start: @lang [name] [(k=v, ...)] { ... }
        if b == b'@' {
            let mut j = i + 1;
            if j < out.len() && is_ident_start(out[j]) {
                j += 1;
                while j < out.len() && is_ident_cont(out[j]) {
                    j += 1;
                }

                let lang = &src[i + 1..j];

                let mut h = skip_ws(&out, j);

                // optional block name
                let mut name: Option<String> = None;
                if h < out.len() && is_ident_start(out[h]) {
                    let start = h;
                    h += 1;
                    while h < out.len() && is_ident_cont(out[h]) {
                        h += 1;
                    }
                    name = Some(src[start..h].to_string());
                    h = skip_ws(&out, h);
                }

                // optional args (...) before the opening '{'
                let mut args: Option<String> = None;
                let mut attrs: HashMap<String, String> = HashMap::new();
                if h < out.len() && out[h] == b'(' {
                    let args_start = h + 1;
                    let mut p = h + 1;
                    let mut depth: i32 = 1;
                    let mut in_dq = false;
                    let mut prev_bs = false;
                    while p < out.len() {
                        let bb = out[p];
                        if in_dq {
                            if bb == b'"' && !prev_bs {
                                in_dq = false;
                            }
                            prev_bs = bb == b'\\' && !prev_bs;
                            p += 1;
                            continue;
                        }
                        if bb == b'"' {
                            in_dq = true;
                            prev_bs = false;
                            p += 1;
                            continue;
                        }
                        if bb == b'(' {
                            depth += 1;
                            p += 1;
                            continue;
                        }
                        if bb == b')' {
                            depth -= 1;
                            if depth == 0 {
                                let args_str = src[args_start..p].to_string();
                                attrs = parse_polyglot_attrs(&args_str);
                                args = Some(args_str);
                                h = p + 1;
                                break;
                            }
                            p += 1;
                            continue;
                        }
                        p += 1;
                    }
                    h = skip_ws(&out, h);
                }

                // finally: opening '{'
                let k = skip_ws(&out, h);
                if k < out.len() && out[k] == b'{' {
                    let block_start = i;
                    let mut p = k + 1; // after '{'

                    let mut depth: i32 = 1;
                    let mut in_dq = false;
                    let mut in_sq = false;
                    let mut prev_bs = false;
                    let mut bc_depth: i32 = 0;

                    while p < out.len() {
                        let bb = out[p];
                        let nn = if p + 1 < out.len() {
                            Some(out[p + 1])
                        } else {
                            None
                        };

                        if in_dq {
                            if bb == b'"' && !prev_bs {
                                in_dq = false;
                            }
                            prev_bs = bb == b'\\' && !prev_bs;
                            p += 1;
                            continue;
                        }
                        if in_sq {
                            if bb == b'\'' && !prev_bs {
                                in_sq = false;
                            }
                            prev_bs = bb == b'\\' && !prev_bs;
                            p += 1;
                            continue;
                        }

                        if bc_depth > 0 {
                            if bb == b'/' && nn == Some(b'*') {
                                bc_depth += 1;
                                p += 2;
                                continue;
                            }
                            if bb == b'*' && nn == Some(b'/') {
                                bc_depth -= 1;
                                p += 2;
                                continue;
                            }
                            p += 1;
                            continue;
                        }

                        // comments inside the block (C/Rust-style)
                        if bb == b'/' && nn == Some(b'/') {
                            p += 2;
                            while p < out.len() && out[p] != b'\n' {
                                p += 1;
                            }
                            continue;
                        }
                        if bb == b'/' && nn == Some(b'*') {
                            bc_depth = 1;
                            p += 2;
                            continue;
                        }

                        if bb == b'"' {
                            in_dq = true;
                            prev_bs = false;
                            p += 1;
                            continue;
                        }
                        if bb == b'\'' {
                            in_sq = true;
                            prev_bs = false;
                            p += 1;
                            continue;
                        }

                        if bb == b'{' {
                            depth += 1;
                            p += 1;
                            continue;
                        }
                        if bb == b'}' {
                            depth -= 1;
                            if depth == 0 {
                                let block_end_inclusive = p; // at '}'
                                let code = src[k + 1..block_end_inclusive].to_string();
                                blocks.push(PolyglotBlock {
                                    lang: lang.to_string(),
                                    name: name.clone(),
                                    args: args.clone(),
                                    attrs: attrs.clone(),
                                    code,
                                    span: Span {
                                        start: block_start,
                                        end: block_end_inclusive + 1,
                                    },
                                });

                                // mask region to spaces (preserve newlines)
                                for byte in out[block_start..=block_end_inclusive].iter_mut() {
                                    if *byte != b'\n' {
                                        *byte = b' ';
                                    }
                                }

                                i = block_end_inclusive + 1;
                                break;
                            }
                            p += 1;
                            continue;
                        }

                        p += 1;
                    }

                    // If we found no closing brace, fall through and treat as normal text.
                    if i != block_start {
                        continue;
                    }
                }
            }
        }

        i += 1;
    }

    (
        String::from_utf8(out.clone())
            .unwrap_or_else(|_| String::from_utf8_lossy(&out).to_string()),
        blocks,
    )
}

/// Opaque parse-error type carried in `ParseWithPolyglotResult`.
///
/// Chumsky 1.0 made `Simple` borrow from the input (`Simple<'src, char>`),
/// so we can no longer expose the raw chumsky error in a `'static` return
/// type. We replace it with a small owned struct that captures the bits
/// callers actually use — currently the byte-offset span, the offending
/// character (if any), and a display message. (`Simple` in chumsky 1.0 no
/// longer exposes `expected`/`reason`; richer diagnostics need `Rich`.)
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: std::ops::Range<usize>,
    pub found: Option<char>,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

/// Type alias for the result of `parse_with_polyglot` to tame the complex type signature.
pub type ParseWithPolyglotResult = Result<(Vec<Stmt>, Vec<PolyglotBlock>), Vec<ParseError>>;

fn preprocess_for_parse(input: &str) -> (String, Vec<PolyglotBlock>) {
    // Keep byte offsets stable by masking comments/polyglot blocks with spaces.
    let (masked, blocks) = mask_and_extract_polyglot_blocks(input);
    let masked = strip_comments_mask(&masked);
    (masked, blocks)
}

/// Maximum bracket/paren/brace nesting depth the parser will accept. Source that
/// nests deeper is rejected up front with a clean `ParseError` instead of
/// overflowing the recursive descent parser's native stack — a `(((…)))` /
/// `[[[…]]]` / `{{{…}}}` "stack bomb" from an untrusted `.pb`/`.sapp` file would
/// otherwise abort the whole process uncatchably. 512 is far above any realistic
/// hand-written or generated nesting.
pub const MAX_NESTING_DEPTH: usize = 512;

/// Scan comment-masked source for the deepest bracket nesting, skipping string
/// literals. Returns a `ParseError` at the first offending offset if the depth
/// exceeds [`MAX_NESTING_DEPTH`]. O(n), iterative (no recursion of its own).
fn check_nesting_depth(masked: &str) -> Option<ParseError> {
    let mut depth: usize = 0;
    let mut in_dq = false; // inside a "double-quoted" string
    let mut in_sq = false; // inside a 'single-quoted' string
    let mut escaped = false;
    for (i, c) in masked.char_indices() {
        if in_dq || in_sq {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if (in_dq && c == '"') || (in_sq && c == '\'') {
                in_dq = false;
                in_sq = false;
            }
            continue;
        }
        match c {
            '"' => in_dq = true,
            '\'' => in_sq = true,
            '(' | '[' | '{' => {
                depth += 1;
                if depth > MAX_NESTING_DEPTH {
                    return Some(ParseError {
                        message: format!(
                            "nesting too deep (exceeds {} levels) at byte {}: refusing to parse to avoid a stack overflow",
                            MAX_NESTING_DEPTH, i
                        ),
                        span: i..i,
                        found: Some(c),
                    });
                }
            }
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn errs_to_owned<'src>(errs: Vec<Simple<'src, char>>) -> Vec<ParseError> {
    errs.into_iter()
        .map(|e| {
            let span = (*e.span()).into_range();
            let found = e.found().copied();
            let message = match found {
                Some(c) => format!("unexpected character {:?} at byte {}", c, span.start),
                None => format!("unexpected end of input at byte {}", span.start),
            };
            ParseError {
                message,
                span,
                found,
            }
        })
        .collect()
}

pub fn parse_with_polyglot_strict(input: &str) -> ParseWithPolyglotResult {
    let (masked, blocks) = preprocess_for_parse(input);
    if let Some(err) = check_nesting_depth(&masked) {
        return Err(vec![err]);
    }
    let result: Result<Vec<Stmt>, Vec<ParseError>> = {
        let parsed = parser().parse(masked.as_str());
        let (out, errs) = parsed.into_output_errors();
        if errs.is_empty() {
            match out {
                Some(stmts) => Ok(stmts),
                None => Err(Vec::new()),
            }
        } else {
            Err(errs_to_owned(errs))
        }
    };
    match result {
        Ok(stmts) => Ok((stmts, blocks)),
        Err(errs) => Err(errs),
    }
}

pub fn parse_with_polyglot(input: &str) -> ParseWithPolyglotResult {
    let (masked, blocks) = preprocess_for_parse(input);
    if let Some(err) = check_nesting_depth(&masked) {
        return Err(vec![err]);
    }

    // SP1-gamma fix: prefer strict parser().parse() over parse_recovery().
    // parse_recovery() consumes unexpected characters (like guard separators `~`/`#`)
    // as error-recovery tokens, preventing valid guard expressions from parsing.
    // The strict parser stops cleanly at separators and returns the correct AST.
    // We only fall back to recovery via `into_output_errors` (chumsky 1.0 equivalent)
    // when strict parsing completely fails, to keep REPL partial-AST behaviour.
    //
    // To keep the borrow checker happy under chumsky 1.0's borrowing `Simple<'src>`
    // we consume the ParseResult inside a nested scope so the borrow ends before
    // we return `masked`-independent owned errors.
    let outcome: Result<Vec<Stmt>, Vec<ParseError>> = {
        let parsed = parser().parse(masked.as_str());
        let (strict_out, strict_errs) = parsed.into_output_errors();
        if strict_errs.is_empty() {
            match strict_out {
                Some(stmts) => Ok(stmts),
                None => Err(Vec::new()),
            }
        } else {
            // Strict parse failed. Try recovery to get a partial AST for IDE/REPL.
            let parsed2 = parser().parse(masked.as_str());
            let (output, _recovery_errs) = parsed2.into_output_errors();
            match output {
                Some(stmts) => Ok(stmts),
                None => Err(errs_to_owned(strict_errs)),
            }
        }
    };
    match outcome {
        Ok(stmts) => Ok((stmts, blocks)),
        Err(errs) => Err(errs),
    }
}

pub fn extract_polyglot_blocks(input: &str) -> Vec<PolyglotBlock> {
    let (_masked, blocks) = mask_and_extract_polyglot_blocks(input);
    blocks
}

pub fn parse(input: &str) -> Result<Vec<Stmt>, Vec<ParseError>> {
    // NOTE: We mask comments with spaces (preserving length) so:
    // - spans remain stable byte offsets
    // - both REPL input and imported module files can contain `//` or `/* */`
    parse_with_polyglot(input).map(|(stmts, _)| stmts)
}

pub fn parse_strict(input: &str) -> Result<Vec<Stmt>, Vec<ParseError>> {
    parse_with_polyglot_strict(input).map(|(stmts, _)| stmts)
}

#[cfg(test)]
mod depth_guard_tests {
    use super::*;

    #[test]
    fn rejects_deeply_nested_parens_without_overflow() {
        // A `(((…1…)))` bomb that would otherwise overflow the recursive parser.
        let n = MAX_NESTING_DEPTH + 50;
        let src = format!("let x = {}1{}", "(".repeat(n), ")".repeat(n));
        let err = parse_strict(&src).expect_err("deep nesting must be rejected");
        assert!(
            err.iter().any(|e| e.message.contains("nesting too deep")),
            "expected a nesting-depth error, got: {:?}",
            err
        );
    }

    #[test]
    fn rejects_deeply_nested_brackets_and_braces() {
        let n = MAX_NESTING_DEPTH + 10;
        for (open, close) in [('[', ']'), ('{', '}')] {
            let src = format!(
                "let x = {}{}",
                open.to_string().repeat(n),
                close.to_string().repeat(n)
            );
            assert!(
                parse_strict(&src)
                    .err()
                    .map(|es| es.iter().any(|e| e.message.contains("nesting too deep")))
                    .unwrap_or(false),
                "depth bomb with {}{} should be rejected",
                open,
                close
            );
        }
    }

    #[test]
    fn accepts_reasonable_nesting() {
        // Realistic nesting (well under the limit) must still parse.
        let n = 30;
        let src = format!("let x = {}1{}", "(".repeat(n), ")".repeat(n));
        assert!(parse_strict(&src).is_ok(), "shallow nesting must parse");
    }

    #[test]
    fn brackets_inside_strings_do_not_count() {
        // A string literal full of '(' must not trip the depth guard.
        let src = format!("let s = \"{}\"", "(".repeat(MAX_NESTING_DEPTH + 100));
        assert!(
            parse_strict(&src).is_ok(),
            "brackets inside a string literal must not count toward nesting depth"
        );
    }
}
