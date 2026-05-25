use chumsky::input::MapExtra;
use chumsky::prelude::*;

use crate::{
    Expr, ExprKind, FStringPart, LogicalOp, MatchArm, Param, Span, Stmt, StmtKind, TypeExpr,
    TypeKind,
};

fn mk_expr(kind: ExprKind, span: std::ops::Range<usize>) -> Expr {
    Expr {
        kind,
        span: span.into(),
    }
}

fn mk_stmt(kind: StmtKind, span: std::ops::Range<usize>) -> Stmt {
    Stmt {
        kind,
        span: span.into(),
    }
}

fn mk_type(kind: TypeKind, span: std::ops::Range<usize>) -> TypeExpr {
    TypeExpr {
        kind,
        span: span.into(),
    }
}

// Helpers for use with `.map_with(...)` — they pull the span from MapExtra,
// fixing the input/extra types so the closure's inference doesn't blow up.
fn mk_expr_w<'src>(
    kind: ExprKind,
    e: &mut MapExtra<'src, '_, &'src str, extra::Err<Simple<'src, char>>>,
) -> Expr {
    mk_expr(kind, e.span().into_range())
}

fn mk_stmt_w<'src>(
    kind: StmtKind,
    e: &mut MapExtra<'src, '_, &'src str, extra::Err<Simple<'src, char>>>,
) -> Stmt {
    mk_stmt(kind, e.span().into_range())
}

fn mk_type_w<'src>(
    kind: TypeKind,
    e: &mut MapExtra<'src, '_, &'src str, extra::Err<Simple<'src, char>>>,
) -> TypeExpr {
    mk_type(kind, e.span().into_range())
}

fn span_of<'src>(
    e: &mut MapExtra<'src, '_, &'src str, extra::Err<Simple<'src, char>>>,
) -> std::ops::Range<usize> {
    e.span().into_range()
}

#[derive(Clone, Copy, Debug)]
enum CmpOp {
    Eq,
    NotEq,
    Gt,
    Ge,
    Lt,
    Le,
}

#[derive(Clone, Copy, Debug)]
enum AddOp {
    Add,
    Sub,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
enum MulOp {
    Mul,
    Div,
}

pub fn parser<'src>(
) -> impl Parser<'src, &'src str, Vec<Stmt>, extra::Err<Simple<'src, char>>> {
    let ws0 = one_of(" \t\r\n").repeated().ignored();
    let ws1 = one_of(" \t\r\n").repeated().at_least(1).ignored();

    let number = text::int(10)
        .map(|s: &str| s.to_string())
        .then(
            just('.')
                .ignore_then(text::digits(10).to_slice().map(|s: &str| s.to_string()))
                .or_not(),
        )
        .map(|(int_part, frac_part): (String, Option<String>)| {
            let s = match frac_part {
                Some(frac) => format!("{int_part}.{frac}"),
                None => int_part,
            };
            ExprKind::Number(s.parse::<f64>().unwrap_or(f64::NAN))
        })
        .map_with(mk_expr_w)
        .padded();

    // String content: either non-escape char or escape sequences
    // Supports: \n \t \r \" \\ \0 \xNN (hex byte) \uXXXX (unicode)
    let hex_digit = any::<&'src str, extra::Err<Simple<'src, char>>>().filter(|c: &char| c.is_ascii_hexdigit());
    let escape = just('\\').then(choice((
        just('n').to('\n'),
        just('t').to('\t'),
        just('r').to('\r'),
        just('"').to('"'),
        just('\\').to('\\'),
        just('0').to('\0'),
        just('{').to('{'), // allow escaping { in f-strings
        just('}').to('}'),
        // \xNN — hex byte escape
        just('x').ignore_then(hex_digit.then(hex_digit).map(|(a, b)| {
            let s = format!("{}{}", a, b);
            char::from(u8::from_str_radix(&s, 16).unwrap_or(0))
        })),
        // \uXXXX — unicode escape (4 hex digits)
        just('u').ignore_then(
            hex_digit
                .then(hex_digit)
                .then(hex_digit)
                .then(hex_digit)
                .map(|(((a, b), c), d)| {
                    let s = format!("{}{}{}{}", a, b, c, d);
                    let cp = u32::from_str_radix(&s, 16).unwrap_or(0);
                    char::from_u32(cp).unwrap_or('\u{FFFD}')
                }),
        ),
    )));
    let string_char = choice((
        none_of(
            "\"\\\
",
        )
        .map(|c| c),
        escape.map(|(_, c)| c),
    ));
    let string = just('"')
        .ignore_then(string_char.repeated().collect::<Vec<char>>())
        .then_ignore(just('"'))
        .map(|chars: Vec<char>| ExprKind::Str(chars.into_iter().collect()))
        .map_with(mk_expr_w)
        .padded();

    let escape_raw = just('\\').then(choice((
        just('n').to('\n'),
        just('t').to('\t'),
        just('r').to('\r'),
        just('"').to('"'),
        just('\\').to('\\'),
    )));
    let string_char_raw = choice((none_of("\"\\").map(|c| c), escape_raw.map(|(_, c)| c)));
    let string_raw = just('"')
        .ignore_then(string_char_raw.repeated().collect::<Vec<char>>())
        .then_ignore(just('"'))
        .map(|chars: Vec<char>| chars.into_iter().collect::<String>())
        .padded();

    let bool_lit = choice((
        just("true").to(ExprKind::Bool(true)),
        just("false").to(ExprKind::Bool(false)),
    ))
    .map_with(mk_expr_w)
    .padded();

    let nil_lit = just("nil")
        .to(ExprKind::Nil)
        .map_with(mk_expr_w)
        .padded();

    let ident = text::ident().map(|s: &str| s.to_string()).padded();

    // Mutual recursion: expression blocks need statements, and statements need expressions.
    let mut expr = Recursive::declare();
    let mut stmt = Recursive::declare();
    // Restricted parser for `match`-arm guard expressions.
    //
    // SP1-gamma: the full `expr` recursive parser exposes a chumsky-0.8
    // `cmp_op.padded().repeated()` partial-consume failure when extended
    // past `n < 0` (the guard ends and the outer arm expects the body).
    // We define `guard_expr` as `cmp_expr` (the level just below logical /
    // ternary / postfix) so the parser stops cleanly before the chain that
    // triggers the cascade. Logical `and`/`or` inside guards is supported
    // by wrapping the guard in parentheses.
    let mut guard_expr = Recursive::declare();

    // -------------------- statements --------------------
    let inner_stmt0 = stmt.clone().then_ignore(just(';').padded().or_not());

    let block_stmt = just('{')
        .ignore_then(ws0)
        .ignore_then(
            inner_stmt0
                .then_ignore(ws0)
                .repeated()
                .collect::<Vec<Stmt>>(),
        )
        .then_ignore(just('}'))
        .map(StmtKind::Block)
        .map_with(mk_stmt_w)
        .padded();

    // -------------------- types (for gradual typing) --------------------
    let mut ty = Recursive::declare();

    let ty_atom = choice((
        just("any").to(TypeKind::Any),
        just("number").to(TypeKind::Number),
        just("bool").to(TypeKind::Bool),
        just("string").to(TypeKind::String),
        just("nil").to(TypeKind::Nil),
    ))
    .map_with(mk_type_w)
    .padded();

    let ty_array = just("array")
        .ignore_then(just('<').padded())
        .ignore_then(ty.clone())
        .then_ignore(just('>').padded())
        .map(|inner| TypeKind::Array(Box::new(inner)))
        .map_with(mk_type_w)
        .padded();

    let ty_map = just("map")
        .ignore_then(just('<').padded())
        .ignore_then(ty.clone())
        .then_ignore(just(',').padded())
        .then(ty.clone())
        .then_ignore(just('>').padded())
        .map(|(k, v)| TypeKind::Map(Box::new(k), Box::new(v)))
        .map_with(mk_type_w)
        .padded();

    let ty_result = just("result")
        .ignore_then(just('<').padded())
        .ignore_then(ty.clone())
        .then_ignore(just(',').padded())
        .then(ty.clone())
        .then_ignore(just('>').padded())
        .map(|(ok, err)| TypeKind::Result(Box::new(ok), Box::new(err)))
        .map_with(mk_type_w)
        .padded();

    // Generic type parameter: an uppercase-starting identifier (e.g. T, K, V)
    let ty_generic = text::ident().map(|s: &str| s.to_string())
        .padded()
        .try_map(|name: String, span| {
            if name
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
                && name != "Any"
                && name != "Number"
                && name != "Bool"
                && name != "String"
                && name != "Nil"
            {
                Ok(TypeKind::Generic(name))
            } else {
                Err(<Simple<char> as chumsky::label::LabelError<&'src str, _>>::expected_found(
                    Vec::<chumsky::util::MaybeRef<'_, char>>::new(),
                    None,
                    span,
                ))
            }
        })
        .map_with(mk_type_w)
        .padded();

    // The recursive type definition includes the `?` postfix for Option types
    // so that T? works at every nesting level (e.g. array<number?> is valid).
    // The inner parser produces a TypeExpr; then we check for an optional trailing `?`
    // to wrap it in TypeKind::Option.
    let ty_inner = choice((ty_array, ty_map, ty_result, ty_atom, ty_generic));
    ty.define(
        ty_inner
            .then(just('?').or_not())
            .map(|(inner, opt): (TypeExpr, Option<_>)| match opt {
                Some(_) => TypeKind::Option(Box::new(inner)),
                None => inner.kind,
            })
            .map_with(mk_type_w),
    );

    let type_ann = just(':').padded().ignore_then(ty.clone()).or_not();

    // vararg param: ...name (must be last in param list)
    let vararg_param = just("...")
        .ignore_then(text::ident().map(|s: &str| s.to_string()).padded())
        .map(|name| Param {
            name,
            ty: None,
            default: None,
            is_vararg: true,
        });

    // param with optional type annotation AND optional default value.
    // IMPORTANT: Default expressions are limited to LITERAL values (number/string/bool/nil)
    // to avoid chumsky type-inference cascade from the recursive Expr type.
    // For complex defaults, use: fn foo(x, y = nil) { if y == nil { y = compute(); } }
    //
    // Supported:
    //   fn foo(x, y = 42, msg = "hello", debug = false, data = nil)
    let param_literal_default = choice((
        // number literal
        text::int(10)
            .map(|s: &str| s.to_string())
            .then(
                just('.')
                    .ignore_then(text::digits(10).to_slice().map(|s: &str| s.to_string()))
                    .or_not(),
            )
            .padded()
            .map(|(int_part, frac_part): (String, Option<String>)| {
                let s = match frac_part {
                    Some(frac) => format!("{int_part}.{frac}"),
                    None => int_part,
                };
                let kind = ExprKind::Number(s.parse::<f64>().unwrap_or(f64::NAN));
                Expr {
                    kind,
                    span: Span { start: 0, end: 0 },
                }
            }),
        // string literal
        just('"')
            .ignore_then(string_char.repeated().collect::<Vec<char>>())
            .then_ignore(just('"'))
            .padded()
            .map(|chars: Vec<char>| Expr {
                kind: ExprKind::Str(chars.into_iter().collect()),
                span: Span { start: 0, end: 0 },
            }),
        // bool literal
        choice((
            just("true").to(ExprKind::Bool(true)),
            just("false").to(ExprKind::Bool(false)),
        ))
        .padded()
        .map(|k| Expr {
            kind: k,
            span: Span { start: 0, end: 0 },
        }),
        // nil literal
        just("nil").to(ExprKind::Nil).padded().map(|k| Expr {
            kind: k,
            span: Span { start: 0, end: 0 },
        }),
    ));

    let param_with_default = text::ident().map(|s: &str| s.to_string())
        .padded()
        .then(type_ann.clone())
        .then(
            just('=')
                .padded()
                .ignore_then(param_literal_default)
                .or_not(),
        )
        .map(|((name, ty), default)| Param {
            name,
            ty,
            default,
            is_vararg: false,
        });

    let param = choice((vararg_param, param_with_default));

    let params = param
        .separated_by(just(',').padded())
        .allow_trailing()
        .collect::<Vec<Param>>()
        .delimited_by(just('('), just(')'))
        .padded();

    let ret_ann = just("->").padded().ignore_then(ty.clone()).or_not();

    let fn_stmt = just("fn")
        .then_ignore(ws1)
        .ignore_then(text::ident().map(|s: &str| s.to_string()).padded())
        .then(params.clone())
        .then(ret_ann.clone())
        .then(block_stmt.clone().map(|s| match s.kind {
            StmtKind::Block(body) => body,
            _ => unreachable!("block parser must produce StmtKind::Block"),
        }))
        .map(|(((name, params), ret), body)| StmtKind::Fn(name, params, ret, body))
        .map_with(mk_stmt_w);

    let printf_stmt = just("printf")
        .then_ignore(ws1)
        .ignore_then(text::int(10).from_str::<i64>().unwrapped())
        .then_ignore(ws1)
        .then(expr.clone())
        .map(|(p, e)| StmtKind::PrintF(p as usize, e))
        .map_with(mk_stmt_w);

    let print_stmt = just("print")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .map(StmtKind::Print)
        .map_with(mk_stmt_w);

    let return_stmt = just("return")
        .ignore_then(expr.clone().or_not())
        .map(StmtKind::Return)
        .map_with(mk_stmt_w)
        .padded();

    // break; — exit nearest loop
    let break_stmt = just("break")
        .map(|_| StmtKind::Break)
        .map_with(mk_stmt_w)
        .padded();

    // continue; — next iteration of nearest loop
    let continue_stmt = just("continue")
        .map(|_| StmtKind::Continue)
        .map_with(mk_stmt_w)
        .padded();

    // throw expr; — raise a runtime error
    let throw_stmt = just("throw")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .map(StmtKind::Throw)
        .map_with(mk_stmt_w)
        .padded();

    // loop { body } — infinite loop (use break to exit)
    let loop_stmt = just("loop")
        .ignore_then(ws0)
        .ignore_then(block_stmt.clone().map(|s| match s.kind {
            StmtKind::Block(body) => body,
            _ => unreachable!(),
        }))
        .map(StmtKind::Loop)
        .map_with(mk_stmt_w);

    let defer_stmt = just("defer")
        .then_ignore(ws1)
        .then(expr.clone())
        .map(|(_, e)| StmtKind::Defer(e))
        .map_with(mk_stmt_w)
        .padded();

    let import_stmt = just("import")
        .then_ignore(ws1)
        .ignore_then(string_raw)
        .map(StmtKind::Import)
        .map_with(mk_stmt_w);

    let export_stmt = just("export")
        .then_ignore(ws1)
        .ignore_then(
            text::ident().map(|s: &str| s.to_string())
                .padded()
                .separated_by(just(',').padded())
                .allow_trailing()
                .collect::<Vec<String>>(),
        )
        .map(StmtKind::Export)
        .map_with(mk_stmt_w);

    // Array destructuring: let [a, b, c] = expr
    //   desugars to: let __d_N = expr; let a = __d_N[0]; let b = __d_N[1]; ...
    //   with optional rest: let [a, b, ...rest] = expr
    //     desugars to: ... let rest = [__d_N[2], __d_N[3], ...]
    let destruct_ident = text::ident().map(|s: &str| s.to_string()).padded();
    let destruct_rest = just("...")
        .ignore_then(text::ident().map(|s: &str| s.to_string()).padded())
        .map(|name| (name, true)); // (name, is_rest)
    let destruct_name_or_rest = choice((destruct_rest, destruct_ident.map(|n| (n, false))));

    let array_destruct_stmt = just("let")
        .then_ignore(ws1)
        .ignore_then(
            destruct_name_or_rest
                .separated_by(just(',').padded())
                .allow_trailing()
                .collect::<Vec<(String, bool)>>()
                .delimited_by(just('[').padded(), just(']').padded()),
        )
        .then_ignore(just('=').padded())
        .then(expr.clone())
        .map_with(|(names, rhs), __e| { let sp = __e.span().into_range();
            let sp = Span::from(sp);
            let tmp = format!("__destruct_{}", sp.start);
            let tmp_expr = Expr {
                kind: ExprKind::Var(tmp.clone()),
                span: sp,
            };

            let mut stmts = vec![Stmt {
                kind: StmtKind::Let(tmp.clone(), None, rhs),
                span: sp,
            }];

            let mut regular_count = 0usize;
            for (name, is_rest) in &names {
                if *is_rest {
                    // rest: collect remaining into array using array_drop-like call
                    // Desugar: let rest = range(regular_count, len(__d)).map(|i| __d[i])
                    // Simpler: build via array_take/array_drop builtins
                    // Use: let rest = { let __t = []; for __ri len(__d) - N { __t[__ri] = __d[N + __ri]; }; __t }
                    let len_call = Expr {
                        kind: ExprKind::Call(
                            Box::new(Expr {
                                kind: ExprKind::Var("len".into()),
                                span: sp,
                            }),
                            vec![tmp_expr.clone()],
                        ),
                        span: sp,
                    };
                    let rest_tmp = format!("__rest_{}", sp.start);
                    let ri_var = format!("__ri_{}", sp.start);
                    let n_const = Expr {
                        kind: ExprKind::Number(regular_count as f64),
                        span: sp,
                    };
                    let rest_len = Expr {
                        kind: ExprKind::Sub(Box::new(len_call), Box::new(n_const.clone())),
                        span: sp,
                    };
                    let rest_init = Stmt {
                        kind: StmtKind::Let(
                            rest_tmp.clone(),
                            None,
                            Expr {
                                kind: ExprKind::Array(vec![]),
                                span: sp,
                            },
                        ),
                        span: sp,
                    };
                    let ri_expr = Expr {
                        kind: ExprKind::Var(ri_var.clone()),
                        span: sp,
                    };
                    let index_expr = Expr {
                        kind: ExprKind::Index(
                            Box::new(tmp_expr.clone()),
                            Box::new(Expr {
                                kind: ExprKind::Add(Box::new(ri_expr.clone()), Box::new(n_const)),
                                span: sp,
                            }),
                        ),
                        span: sp,
                    };
                    let rest_assign = Stmt {
                        kind: StmtKind::SetIndex(
                            Expr {
                                kind: ExprKind::Var(rest_tmp.clone()),
                                span: sp,
                            },
                            ri_expr.clone(),
                            index_expr,
                        ),
                        span: sp,
                    };
                    let for_body = vec![rest_assign];
                    let for_loop = Stmt {
                        kind: StmtKind::For(ri_var, rest_len, for_body),
                        span: sp,
                    };
                    stmts.push(rest_init);
                    stmts.push(for_loop);
                    stmts.push(Stmt {
                        kind: StmtKind::Let(
                            name.clone(),
                            None,
                            Expr {
                                kind: ExprKind::Var(rest_tmp),
                                span: sp,
                            },
                        ),
                        span: sp,
                    });
                } else {
                    let idx = Expr {
                        kind: ExprKind::Number(regular_count as f64),
                        span: sp,
                    };
                    let rhs = Expr {
                        kind: ExprKind::Index(Box::new(tmp_expr.clone()), Box::new(idx)),
                        span: sp,
                    };
                    stmts.push(Stmt {
                        kind: StmtKind::Let(name.clone(), None, rhs),
                        span: sp,
                    });
                    regular_count += 1;
                }
            }
            Stmt {
                kind: StmtKind::Seq(stmts),
                span: sp,
            }
        })
        .boxed();

    // Map destructuring: let {x, y} = expr  OR  let {key: local_name} = expr
    //   desugars to: let __d_N = expr; let x = __d_N.x; let y = __d_N.y;
    //   With rename: let {key: alias} = expr  →  let alias = __d_N.key;
    let map_destruct_field = choice((
        // key: local_name
        text::ident().map(|s: &str| s.to_string())
            .padded()
            .then_ignore(just(':').padded())
            .then(text::ident().map(|s: &str| s.to_string()).padded())
            .map(|(key, alias): (String, String)| (key, alias)),
        // key  (same name for key and local)
        text::ident().map(|s: &str| s.to_string())
            .padded()
            .map(|name: String| (name.clone(), name)),
    ));

    let map_destruct_stmt = just("let")
        .then_ignore(ws1)
        .ignore_then(
            map_destruct_field
                .separated_by(just(',').padded())
                .allow_trailing()
                .collect::<Vec<(String, String)>>()
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .then_ignore(just('=').padded())
        .then(expr.clone())
        .map_with(|(fields, rhs), __e| { let sp = __e.span().into_range();
            let sp = Span::from(sp);
            let tmp = format!("__mdestruct_{}", sp.start);
            let tmp_expr = Expr {
                kind: ExprKind::Var(tmp.clone()),
                span: sp,
            };

            let mut stmts = vec![Stmt {
                kind: StmtKind::Let(tmp.clone(), None, rhs),
                span: sp,
            }];

            for (key, local_name) in fields {
                let attr_expr = Expr {
                    kind: ExprKind::GetAttr(Box::new(tmp_expr.clone()), key),
                    span: sp,
                };
                stmts.push(Stmt {
                    kind: StmtKind::Let(local_name, None, attr_expr),
                    span: sp,
                });
            }
            Stmt {
                kind: StmtKind::Seq(stmts),
                span: sp,
            }
        })
        .boxed();

    // Plain let: let name[:type] = expr
    let let_stmt = just("let")
        .then_ignore(ws1)
        .ignore_then(text::ident().map(|s: &str| s.to_string()))
        .then(type_ann.clone())
        .then_ignore(just('=').padded())
        .then(expr.clone())
        .map(|((name, ty), expr)| StmtKind::Let(name, ty, expr))
        .map_with(mk_stmt_w);

    // const name = expr; — immutable binding (Rust-style)
    let const_stmt = just("const")
        .then_ignore(ws1)
        .ignore_then(text::ident().map(|s: &str| s.to_string()))
        .then(type_ann.clone())
        .then_ignore(just('=').padded())
        .then(expr.clone())
        .map(|((name, ty), init)| StmtKind::Const(name, ty, init))
        .map_with(mk_stmt_w);

    // assignment / mutation statement:
    //   x = expr
    //   x[i] = expr
    //   x.foo = expr
    //   (and nested lvalues like x.foo[i].bar = expr)
    #[derive(Clone, Debug)]
    enum LPostfix {
        Attr(String, Span),
        Index(Expr, Span),
    }

    let l_attr = just('.')
        .ignore_then(text::ident().map(|s: &str| s.to_string()).padded())
        .map_with(|name, e| LPostfix::Attr(name, Span::from(span_of(e))))
        .padded();

    let l_index = expr
        .clone()
        .delimited_by(just('['), just(']'))
        .map_with(|x, e| LPostfix::Index(x, Span::from(span_of(e))))
        .padded();

    let lvalue = text::ident().map(|s: &str| s.to_string())
        .padded()
        .map(ExprKind::Var)
        .map_with(mk_expr_w)
        .foldl(choice((l_attr, l_index)).repeated(), |left: Expr, pf: LPostfix| match pf {
            LPostfix::Attr(name, a_sp) => {
                let sp = Span {
                    start: left.span.start,
                    end: a_sp.end,
                };
                Expr {
                    kind: ExprKind::GetAttr(Box::new(left), name),
                    span: sp,
                }
            }
            LPostfix::Index(ix, i_sp) => {
                let sp = Span {
                    start: left.span.start,
                    end: i_sp.end,
                };
                Expr {
                    kind: ExprKind::Index(Box::new(left), Box::new(ix)),
                    span: sp,
                }
            }
        });

    // Augmented assignment operators: +=, -=, **=, *=, /=, %=
    #[derive(Clone, Copy, Debug)]
    enum AugOp {
        Add,
        Sub,
        Mul,
        Div,
        Mod,
        Pow,
    }

    let aug_op = choice((
        just("+=").to(AugOp::Add),
        just("-=").to(AugOp::Sub),
        just("**=").to(AugOp::Pow), // MUST come before *=
        just("*=").to(AugOp::Mul),
        just("/=").to(AugOp::Div),
        just("%=").to(AugOp::Mod),
    ))
    .padded();

    // aug_assign: lvalue op= expr  (desugars to lvalue = lvalue op expr)
    let aug_assign_stmt = lvalue
        .clone()
        .then(aug_op)
        .then(expr.clone())
        .map_with(|((lhs, op), rhs), __e| { let sp = __e.span().into_range();
            let sp = Span::from(sp);
            // Build: lhs_expr op rhs
            let lhs_expr = lhs.clone();
            let combined = match op {
                AugOp::Add => Expr {
                    kind: ExprKind::Add(Box::new(lhs_expr), Box::new(rhs)),
                    span: sp,
                },
                AugOp::Sub => Expr {
                    kind: ExprKind::Sub(Box::new(lhs_expr), Box::new(rhs)),
                    span: sp,
                },
                AugOp::Mul => Expr {
                    kind: ExprKind::Mul(Box::new(lhs_expr), Box::new(rhs)),
                    span: sp,
                },
                AugOp::Div => Expr {
                    kind: ExprKind::Div(Box::new(lhs_expr), Box::new(rhs)),
                    span: sp,
                },
                AugOp::Mod => Expr {
                    kind: ExprKind::Mod(Box::new(lhs_expr), Box::new(rhs)),
                    span: sp,
                },
                AugOp::Pow => Expr {
                    kind: ExprKind::Pow(Box::new(lhs_expr), Box::new(rhs)),
                    span: sp,
                },
            };
            let kind = match lhs.kind {
                ExprKind::Var(name) => StmtKind::Assign(name, combined),
                ExprKind::GetAttr(obj, name) => StmtKind::SetAttr(*obj, name, combined),
                ExprKind::Index(obj, ix) => StmtKind::SetIndex(*obj, *ix, combined),
                _ => unreachable!("lvalue parser only builds Var/GetAttr/Index"),
            };
            Stmt { kind, span: sp }
        });

    // Plain assignment: lvalue = expr
    // NOTE: aug_assign MUST come first in choice() to avoid consuming lvalue then failing on `+=`
    let set_stmt = lvalue
        .then_ignore(just('=').padded())
        .then(expr.clone())
        .map(|(lhs, rhs)| match lhs.kind {
            ExprKind::Var(name) => StmtKind::Assign(name, rhs),
            ExprKind::GetAttr(obj, name) => StmtKind::SetAttr(*obj, name, rhs),
            ExprKind::Index(obj, ix) => StmtKind::SetIndex(*obj, *ix, rhs),
            _ => unreachable!("lvalue parser only builds Var/GetAttr/Index"),
        })
        .map_with(mk_stmt_w);

    let expr_stmt = expr.clone().map(StmtKind::Expr).map_with(mk_stmt_w);

    // if/elif/else needs self-reference for `else if`
    let mut if_stmt = Recursive::declare();
    let block_body = block_stmt.clone().map(|s| match s.kind {
        StmtKind::Block(body) => body,
        _ => unreachable!("block parser must produce StmtKind::Block"),
    });

    let elif_clause = just("elif")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .then(block_body.clone());

    let else_clause = just("else")
        .then_ignore(ws1)
        .ignore_then(choice((
            block_body.clone(),
            if_stmt.clone().map(|s| vec![s]),
        )))
        .or_not();

    let if_stmt_impl = just("if")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .then(block_body.clone())
        .then(elif_clause.repeated().collect::<Vec<(Expr, Vec<Stmt>)>>())
        .then(else_clause)
        .map_with(|(((cond, then_body), elifs), else_body), __e| { let sp = __e.span().into_range();
            let sp = Span::from(sp);
            let mut else_body = else_body;

            // Build nested else-if chain from elifs.
            for (econd, ebody) in elifs.into_iter().rev() {
                let nested = Stmt {
                    kind: StmtKind::If(econd, ebody, else_body.take()),
                    span: sp,
                };
                else_body = Some(vec![nested]);
            }

            Stmt {
                kind: StmtKind::If(cond, then_body, else_body),
                span: sp,
            }
        });

    if_stmt.define(if_stmt_impl);

    let while_stmt = just("while")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .then(block_stmt.clone().map(|s| match s.kind {
            StmtKind::Block(body) => body,
            _ => unreachable!("block parser must produce StmtKind::Block"),
        }))
        .map(|(cond, body)| StmtKind::While(cond, body))
        .map_with(mk_stmt_w);

    // for k, v in expr { body } — iterates map key-value pairs
    let for_in_kv_stmt = just("for")
        .then_ignore(ws1)
        .ignore_then(text::ident().map(|s: &str| s.to_string()).padded())
        .then_ignore(just(',').padded())
        .then(text::ident().map(|s: &str| s.to_string()).padded())
        .then_ignore(ws0) // padded already consumed trailing ws
        .then_ignore(just("in"))
        .then_ignore(ws1)
        .then(expr.clone())
        .then(block_stmt.clone().map(|s| match s.kind {
            StmtKind::Block(body) => body,
            _ => unreachable!(),
        }))
        .map(|(((k, v), col), body)| StmtKind::ForInKV(k, v, col, body))
        .map_with(mk_stmt_w);

    // for x in expr { body } — iterates array elements or map keys
    let for_in_stmt = just("for")
        .then_ignore(ws1)
        .ignore_then(text::ident().map(|s: &str| s.to_string()))
        .then_ignore(ws1)
        .then_ignore(just("in"))
        .then_ignore(ws1)
        .then(expr.clone())
        .then(block_stmt.clone().map(|s| match s.kind {
            StmtKind::Block(body) => body,
            _ => unreachable!(),
        }))
        .map(|((name, col), body)| StmtKind::ForIn(name, col, body))
        .map_with(mk_stmt_w);

    // for i N { body } — original counter loop
    let for_stmt = just("for")
        .then_ignore(ws1)
        .ignore_then(text::ident().map(|s: &str| s.to_string()))
        .then_ignore(ws1)
        .then(expr.clone())
        .then(block_stmt.clone().map(|s| match s.kind {
            StmtKind::Block(body) => body,
            _ => unreachable!("block parser must produce StmtKind::Block"),
        }))
        .map(|((name, count), body)| StmtKind::For(name, count, body))
        .map_with(mk_stmt_w);

    // class statement (OOP sugar):
    // class Name {
    //   fn method(self, x) { ... }
    //   fn other() { ... }
    // }
    // Desugars to:
    // let Name = map{ method: fn method(self, x) { ... }, other: fn other() { ... } };
    let class_method = just("fn")
        .then_ignore(ws1)
        .ignore_then(text::ident().map(|s: &str| s.to_string()).padded())
        .then(params.clone())
        .then(ret_ann.clone())
        .then(block_body.clone())
        .map_with(|(((name, params), ret), body), __e| { let sp = __e.span().into_range();
            let sp = Span::from(sp);
            let f = Expr {
                kind: ExprKind::FnLit(Some(name.clone()), params, ret, body),
                span: sp,
            };
            (name, f)
        });

    let class_stmt = just("class")
        .then_ignore(ws1)
        .ignore_then(text::ident().map(|s: &str| s.to_string()).padded())
        .then_ignore(ws0)
        .then(
            class_method
                .then_ignore(just(';').padded().or_not())
                .repeated()
                .collect::<Vec<(String, Expr)>>()
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map_with(|(name, methods), __e| { let sp = __e.span().into_range();
            let sp = Span::from(sp);
            // Detect init arity BEFORE methods is moved into the map expression.
            // Used later to wire up __call so that `ClassName(args...)` works.
            let init_arity: Option<usize> = methods.iter().find_map(|(mname, mfn)| {
                if mname == "init" {
                    if let ExprKind::FnLit(_, params, _, _) = &mfn.kind {
                        // params[0] is `self`, so real arity = len - 1
                        return Some(params.len().saturating_sub(1));
                    }
                }
                None
            });
            let mexpr = Expr {
                kind: ExprKind::Map(methods),
                span: sp,
            };
            // Add OOP niceties without changing VM architecture:
            // - Class value is a map of methods
            // - Auto `new(...)` allocates an object map{ __class: Class } and calls init if present.
            let cname = name.clone();

            let let_class = Stmt {
                kind: StmtKind::Let(name.clone(), None, mexpr),
                span: sp,
            };

            // Name.new = fn(...args) { let o = map{ __class: Name }; try Name.init(o, ...args) catch nil; return o; }
            let o_name = "__obj".to_string();

            let o_init = Expr {
                kind: ExprKind::Map(vec![(
                    "__class".to_string(),
                    Expr {
                        kind: ExprKind::Var(cname.clone()),
                        span: sp,
                    },
                )]),
                span: sp,
            };

            // o = map{ __class: Name }
            let let_o = Stmt {
                kind: StmtKind::Let(o_name.clone(), None, o_init),
                span: sp,
            };

            // call: Name.init(o, args[0], args[1]...) but we don't have varargs;
            // provide fixed-arity new0/new1/new2/new3 via 4 overloads for now.
            // We'll generate Name.new0..new3 and Name.new as alias to new0.
            let mk_new = |n: usize| {
                let mut params: Vec<Param> = vec![];
                for i in 0..n {
                    params.push(Param::simple(format!("a{i}"), None));
                }

                let mut call_args = vec![Expr {
                    kind: ExprKind::Var(o_name.clone()),
                    span: sp,
                }];
                for i in 0..n {
                    call_args.push(Expr {
                        kind: ExprKind::Var(format!("a{i}")),
                        span: sp,
                    });
                }

                let init_callee = Expr {
                    kind: ExprKind::GetAttr(
                        Box::new(Expr {
                            kind: ExprKind::Var(cname.clone()),
                            span: sp,
                        }),
                        "init".to_string(),
                    ),
                    span: sp,
                };

                let init_call = Expr {
                    kind: ExprKind::Call(Box::new(init_callee), call_args),
                    span: sp,
                };

                let try_init = Stmt {
                    kind: StmtKind::Expr(Expr {
                        kind: ExprKind::Try(
                            Box::new(init_call),
                            Box::new(Expr {
                                kind: ExprKind::Nil,
                                span: sp,
                            }),
                        ),
                        span: sp,
                    }),
                    span: sp,
                };

                let ret_o = Stmt {
                    kind: StmtKind::Return(Some(Expr {
                        kind: ExprKind::Var(o_name.clone()),
                        span: sp,
                    })),
                    span: sp,
                };

                let body = vec![let_o.clone(), try_init, ret_o];

                Expr {
                    kind: ExprKind::FnLit(Some(format!("new{n}")), params, None, body),
                    span: sp,
                }
            };

            let new0 = mk_new(0);
            let new1 = mk_new(1);
            let new2 = mk_new(2);
            let new3 = mk_new(3);

            let set_new0 = Stmt {
                kind: StmtKind::SetAttr(
                    Expr {
                        kind: ExprKind::Var(cname.clone()),
                        span: sp,
                    },
                    "new0".to_string(),
                    new0,
                ),
                span: sp,
            };
            let set_new1 = Stmt {
                kind: StmtKind::SetAttr(
                    Expr {
                        kind: ExprKind::Var(cname.clone()),
                        span: sp,
                    },
                    "new1".to_string(),
                    new1,
                ),
                span: sp,
            };
            let set_new2 = Stmt {
                kind: StmtKind::SetAttr(
                    Expr {
                        kind: ExprKind::Var(cname.clone()),
                        span: sp,
                    },
                    "new2".to_string(),
                    new2,
                ),
                span: sp,
            };
            let set_new3 = Stmt {
                kind: StmtKind::SetAttr(
                    Expr {
                        kind: ExprKind::Var(cname.clone()),
                        span: sp,
                    },
                    "new3".to_string(),
                    new3,
                ),
                span: sp,
            };
            // FIX #11: .new must alias to the newN that matches the actual init arity,
            // not always new0. If there is no init method, new0 (zero-arg) is correct.
            let new_alias_target = init_arity
                .map(|a| format!("new{a}"))
                .unwrap_or_else(|| "new0".to_string());
            let set_new = Stmt {
                kind: StmtKind::SetAttr(
                    Expr {
                        kind: ExprKind::Var(cname.clone()),
                        span: sp,
                    },
                    "new".to_string(),
                    Expr {
                        kind: ExprKind::GetAttr(
                            Box::new(Expr {
                                kind: ExprKind::Var(cname.clone()),
                                span: sp,
                            }),
                            new_alias_target,
                        ),
                        span: sp,
                    },
                ),
                span: sp,
            };

            // If the class has an `init` method, wire __call to the matching newN so that
            // `ClassName(args...)` works (VM Map.__call dispatch handles it).
            let maybe_set_call = init_arity.map(|arity| Stmt {
                kind: StmtKind::SetAttr(
                    Expr {
                        kind: ExprKind::Var(cname.clone()),
                        span: sp,
                    },
                    "__call".to_string(),
                    Expr {
                        kind: ExprKind::GetAttr(
                            Box::new(Expr {
                                kind: ExprKind::Var(cname.clone()),
                                span: sp,
                            }),
                            format!("new{arity}"),
                        ),
                        span: sp,
                    },
                ),
                span: sp,
            });

            let mut block_stmts = vec![let_class, set_new0, set_new1, set_new2, set_new3, set_new];
            if let Some(set_call) = maybe_set_call {
                block_stmts.push(set_call);
            }

            Stmt {
                kind: StmtKind::Block(block_stmts),
                span: sp,
            }
        });

    // goto statement (structured, no VM architecture change):
    //
    // goto {
    //   label start { ...; goto loop; }
    //   label loop  { ...; end; }
    // }
    //
    // Implementation: desugars to an IIFE with:
    // - one function per label returning next label as string
    // - a pc loop dispatching by label string
    //
    // Notes:
    // - For shared state between labels, store it in a map/array declared outside (or mutate globals).
    //   (Each label runs in its own function; closures can capture outer vars, but label-local `let` is local.)
    let mut gstmt = Recursive::declare();

    let g_inner_stmt0 = gstmt.clone().then_ignore(just(';').padded().or_not());
    let gblock_stmt = just('{')
        .ignore_then(ws0)
        .ignore_then(
            g_inner_stmt0
                .then_ignore(ws0)
                .repeated()
                .collect::<Vec<Stmt>>(),
        )
        .then_ignore(just('}'))
        .map(StmtKind::Block)
        .map_with(mk_stmt_w)
        .padded();

    let gblock_body = gblock_stmt.clone().map(|s| match s.kind {
        StmtKind::Block(body) => body,
        _ => unreachable!("gblock parser must produce StmtKind::Block"),
    });

    let goto_jump_stmt = just("goto")
        .then_ignore(ws1)
        .ignore_then(text::ident().map(|s: &str| s.to_string()).padded())
        .map_with(|label, __e| { let sp = __e.span().into_range();
            let sp = Span::from(sp);
            let e = Expr {
                kind: ExprKind::Str(label),
                span: sp,
            };
            Stmt {
                kind: StmtKind::Return(Some(e)),
                span: sp,
            }
        });

    let end_stmt = just("end").map_with(|_, __e| {
        let sp = span_of(__e);
        let sp = Span::from(sp);
        let e = Expr {
            kind: ExprKind::Str("__done".to_string()),
            span: sp,
        };
        Stmt {
            kind: StmtKind::Return(Some(e)),
            span: sp,
        }
    });

    let g_elif_clause = just("elif")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .then(gblock_body.clone());

    let g_else_clause = just("else")
        .then_ignore(ws1)
        .ignore_then(choice((
            gblock_body.clone(),
            if_stmt.clone().map(|s| vec![s]),
        )))
        .or_not();

    let g_if_stmt = just("if")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .then(gblock_body.clone())
        .then(g_elif_clause.repeated().collect::<Vec<(Expr, Vec<Stmt>)>>())
        .then(g_else_clause)
        .map_with(|(((cond, then_body), elifs), else_body), __e| { let sp = __e.span().into_range();
            let sp = Span::from(sp);
            let mut else_body = else_body;
            for (econd, ebody) in elifs.into_iter().rev() {
                let nested = Stmt {
                    kind: StmtKind::If(econd, ebody, else_body.take()),
                    span: sp,
                };
                else_body = Some(vec![nested]);
            }
            Stmt {
                kind: StmtKind::If(cond, then_body, else_body),
                span: sp,
            }
        });

    let g_while_stmt = just("while")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .then(gblock_body.clone())
        .map(|(cond, body)| StmtKind::While(cond, body))
        .map_with(mk_stmt_w);

    // g_for_in_kv_stmt: for k, v in expr { body } inside goto context
    let g_for_in_kv_stmt = just("for")
        .then_ignore(ws1)
        .ignore_then(text::ident().map(|s: &str| s.to_string()).padded())
        .then_ignore(just(',').padded())
        .then(text::ident().map(|s: &str| s.to_string()).padded())
        .then_ignore(ws0) // padded already consumed trailing ws
        .then_ignore(just("in"))
        .then_ignore(ws1)
        .then(expr.clone())
        .then(gblock_body.clone())
        .map(|(((k, v), col), body)| StmtKind::ForInKV(k, v, col, body))
        .map_with(mk_stmt_w);

    // g_for_in_stmt: for x in expr { body } inside goto context
    let g_for_in_stmt = just("for")
        .then_ignore(ws1)
        .ignore_then(text::ident().map(|s: &str| s.to_string()))
        .then_ignore(ws1)
        .then_ignore(just("in"))
        .then_ignore(ws1)
        .then(expr.clone())
        .then(gblock_body.clone())
        .map(|((name, col), body)| StmtKind::ForIn(name, col, body))
        .map_with(mk_stmt_w);

    let g_for_stmt = just("for")
        .then_ignore(ws1)
        .ignore_then(text::ident().map(|s: &str| s.to_string()))
        .then_ignore(ws1)
        .then(expr.clone())
        .then(gblock_body.clone())
        .map(|((name, count), body)| StmtKind::For(name, count, body))
        .map_with(mk_stmt_w);

    // Allow nested blocks inside goto label bodies.
    let g_stmt_impl = choice((
        goto_jump_stmt,
        end_stmt,
        printf_stmt.clone(),
        print_stmt.clone(),
        let_stmt.clone(),
        return_stmt.clone(),
        g_if_stmt,
        g_while_stmt,
        g_for_in_kv_stmt,
        g_for_in_stmt,
        g_for_stmt,
        gblock_stmt.clone(),
        set_stmt.clone(),
        expr_stmt.clone(),
    ))
    .padded();

    gstmt.define(g_stmt_impl);

    let goto_label = just("label")
        .then_ignore(ws1)
        .ignore_then(text::ident().map(|s: &str| s.to_string()).padded())
        .then(gblock_body.clone());

    let goto_block_stmt = just("goto")
        .then_ignore(ws0)
        .ignore_then(
            goto_label
                .repeated()
                .at_least(1)
                .collect::<Vec<(String, Vec<Stmt>)>>()
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map_with(|labels, __e| { let sp = __e.span().into_range();
            let sp = Span::from(sp);

            let pc = format!("__pc_{}", sp.start);
            let done = "__done".to_string();

            let entry = labels
                .first()
                .map(|(n, _)| n.clone())
                .unwrap_or_else(|| "start".to_string());

            // Build per-label functions.
            let mut dispatch: Vec<(String, String)> = vec![];
            for (name, _) in &labels {
                dispatch.push((name.clone(), format!("__goto_{}_{}", sp.start, name)));
            }

            let mut fixed: Vec<Stmt> = vec![];
            for (i, (name, body)) in labels.into_iter().enumerate() {
                let fn_name = format!("__goto_{}_{}", sp.start, name);
                let fallthrough = if i + 1 < dispatch.len() {
                    dispatch[i + 1].0.clone()
                } else {
                    done.clone()
                };

                let mut fbody = body;
                fbody.push(Stmt {
                    kind: StmtKind::Return(Some(Expr {
                        kind: ExprKind::Str(fallthrough),
                        span: sp,
                    })),
                    span: sp,
                });

                fixed.push(Stmt {
                    kind: StmtKind::Fn(fn_name, vec![], None, fbody),
                    span: sp,
                });
            }

            // pc initialization
            let let_pc = Stmt {
                kind: StmtKind::Let(
                    pc.clone(),
                    None,
                    Expr {
                        kind: ExprKind::Str(entry),
                        span: sp,
                    },
                ),
                span: sp,
            };

            let pc_expr = Expr {
                kind: ExprKind::Var(pc.clone()),
                span: sp,
            };

            let done_cond = Expr {
                kind: ExprKind::Eq(
                    Box::new(pc_expr.clone()),
                    Box::new(Expr {
                        kind: ExprKind::Str(done.clone()),
                        span: sp,
                    }),
                ),
                span: sp,
            };

            let ret_nil = Stmt {
                kind: StmtKind::Return(Some(Expr {
                    kind: ExprKind::Nil,
                    span: sp,
                })),
                span: sp,
            };

            // Build dispatch if-chain.
            let mut else_body: Option<Vec<Stmt>> = Some(vec![ret_nil.clone()]);
            for (lname, fname) in dispatch.into_iter().rev() {
                let cond = Expr {
                    kind: ExprKind::Eq(
                        Box::new(pc_expr.clone()),
                        Box::new(Expr {
                            kind: ExprKind::Str(lname),
                            span: sp,
                        }),
                    ),
                    span: sp,
                };
                let call = Expr {
                    kind: ExprKind::Call(
                        Box::new(Expr {
                            kind: ExprKind::Var(fname),
                            span: sp,
                        }),
                        vec![],
                    ),
                    span: sp,
                };
                let then_body = vec![Stmt {
                    kind: StmtKind::Assign(pc.clone(), call),
                    span: sp,
                }];
                let ifs = Stmt {
                    kind: StmtKind::If(cond, then_body, else_body.take()),
                    span: sp,
                };
                else_body = Some(vec![ifs]);
            }

            // If done -> return nil, else -> dispatch
            let loop_body = vec![Stmt {
                kind: StmtKind::If(done_cond, vec![ret_nil], else_body),
                span: sp,
            }];

            let while_true = Stmt {
                kind: StmtKind::While(
                    Expr {
                        kind: ExprKind::Bool(true),
                        span: sp,
                    },
                    loop_body,
                ),
                span: sp,
            };

            let mut stmts: Vec<Stmt> = vec![let_pc];
            stmts.extend(fixed);
            stmts.push(while_true);

            Stmt {
                kind: StmtKind::Expr(Expr {
                    kind: ExprKind::BlockExpr(stmts, None),
                    span: sp,
                }),
                span: sp,
            }
        });

    // switch statement (sugar):
    // switch expr {
    //   case v { ... }
    //   case v2 { ... }
    //   default { ... }
    // }
    // Desugars to:
    // { let __switch_<sp.start> = expr; if __switch_.. == v { ... } else if ... else { ... } }
    let switch_case = just("case")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .then(block_stmt.clone().map(|s| match s.kind {
            StmtKind::Block(body) => body,
            _ => unreachable!("block parser must produce StmtKind::Block"),
        }));

    let switch_default = just("default")
        .then_ignore(ws0)
        .ignore_then(block_stmt.clone().map(|s| match s.kind {
            StmtKind::Block(body) => body,
            _ => unreachable!("block parser must produce StmtKind::Block"),
        }))
        .or_not();

    let switch_stmt = just("switch")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .then_ignore(ws0)
        .then(
            switch_case
                .repeated()
                .collect::<Vec<(Expr, Vec<Stmt>)>>()
                .then(switch_default)
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map_with(|(scrutinee, (cases, default_body)), __e| { let sp = __e.span().into_range();
            let sp = Span::from(sp);
            let tmp = format!("__switch_{}", sp.start);
            let tmp_expr = Expr {
                kind: ExprKind::Var(tmp.clone()),
                span: sp,
            };

            let let_stmt = Stmt {
                kind: StmtKind::Let(tmp.clone(), None, scrutinee),
                span: sp,
            };

            let mut else_body = default_body;
            for (case_expr, body) in cases.into_iter().rev() {
                let cond_sp = Span::join(tmp_expr.span, case_expr.span);
                let cond = Expr {
                    kind: ExprKind::Eq(Box::new(tmp_expr.clone()), Box::new(case_expr)),
                    span: cond_sp,
                };
                let if_stmt = Stmt {
                    kind: StmtKind::If(cond, body, else_body.take()),
                    span: sp,
                };
                else_body = Some(vec![if_stmt]);
            }

            let mut body = vec![let_stmt];
            if let Some(stmts) = else_body {
                // else_body now contains the top if-chain as a single stmt
                body.extend(stmts);
            }
            Stmt {
                kind: StmtKind::Block(body),
                span: sp,
            }
        });

    // IMPORTANT: keep set_stmt LAST so it doesn't steal `print`, `return`, `fn`, etc.
    // Split into two groups to stay within chumsky's choice() limit (24 parsers max).
    let stmt_group_a = choice((
        fn_stmt,
        printf_stmt,
        print_stmt,
        return_stmt,
        break_stmt,
        continue_stmt,
        throw_stmt,
        loop_stmt,
        defer_stmt,
        import_stmt,
        export_stmt,
        class_stmt,
        goto_block_stmt,
    ));

    let stmt_group_b = choice((
        const_stmt,          // must come before let_stmt
        array_destruct_stmt, // must come before let_stmt (starts with `let`)
        map_destruct_stmt,   // must come before let_stmt (starts with `let`)
        let_stmt,
        if_stmt.clone(),
        while_stmt,
        for_in_kv_stmt,
        for_in_stmt,
        for_stmt,
        switch_stmt,
        block_stmt.clone(), // must come before assign_stmt
        aug_assign_stmt,    // must come before set_stmt
        set_stmt,
        expr_stmt,
    ));

    let stmt_impl = choice((stmt_group_a, stmt_group_b)).padded();

    stmt.define(stmt_impl);

    // -------------------- expressions --------------------
    let paren_expr = expr.clone().delimited_by(just('('), just(')')).padded();

    let args = expr
        .clone()
        .separated_by(just(',').padded())
        .allow_trailing()
        .collect::<Vec<Expr>>()
        .delimited_by(just('('), just(')'))
        .map_with(|args, __e| { let sp = __e.span().into_range(); (args, Span::from(sp)) })
        .padded();

    let ident_expr = ident.map(ExprKind::Var).map_with(mk_expr_w);

    // Function literal expression:
    // fn (a, b) { ... }
    // fn name(a, b) { ... }   (optional name, mainly for debugging/stack traces later)
    let fn_lit_name = ws1.ignore_then(text::ident().map(|s: &str| s.to_string()).padded()).or_not();

    let fn_lit_expr = just("fn")
        .ignore_then(fn_lit_name)
        .then(params.clone())
        .then(ret_ann.clone())
        .then(block_body.clone())
        .map(|(((name_opt, params), ret), body)| ExprKind::FnLit(name_opt, params, ret, body))
        .map_with(mk_expr_w)
        .padded();

    // Expression block: { stmt; stmt; expr? }
    // Inner statements must end with ';' so the tail expression (if any) can be unambiguous.
    let inner_stmt = stmt.clone().then_ignore(just(';').padded());
    let block_expr = inner_stmt
        .repeated()
        .collect::<Vec<Stmt>>()
        .then(expr.clone().or_not())
        .delimited_by(just('{'), just('}'))
        .map(|(stmts, tail)| ExprKind::BlockExpr(stmts, tail.map(Box::new)))
        .map_with(mk_expr_w)
        .padded();

    // `if cond { then } else { else }` — expression-level conditional (both arms are blocks).
    let if_expr = just("if")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .then(block_expr.clone())
        .then_ignore(just("else").padded())
        .then(block_expr.clone())
        .map(|((cond, then_br), else_br)| {
            ExprKind::IfExpr(Box::new(cond), Box::new(then_br), Box::new(else_br))
        })
        .map_with(mk_expr_w)
        .padded();

    let array_lit = expr
        .clone()
        .separated_by(just(',').padded())
        .allow_trailing()
        .collect::<Vec<Expr>>()
        .delimited_by(just('['), just(']'))
        .map(ExprKind::Array)
        .map_with(mk_expr_w)
        .padded();

    let map_key = choice((
        // allow: foo: 1
        text::ident().map(|s: &str| s.to_string()).padded(),
        // allow: "foo": 1
        string_raw,
    ));

    let map_pair = map_key
        .then_ignore(just(':').padded())
        .then(expr.clone())
        .map(|(k, v)| (k, v));

    let map_lit = just("map")
        .then_ignore(just('{').padded())
        .ignore_then(
            map_pair
                .separated_by(just(',').padded())
                .allow_trailing()
                .collect::<Vec<(String, Expr)>>(),
        )
        .then_ignore(just('}').padded())
        .map(ExprKind::Map)
        .map_with(mk_expr_w)
        .padded();

    let try_expr = just("try")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .then_ignore(just("catch").padded())
        .then(expr.clone())
        .map(|(a, b)| ExprKind::Try(Box::new(a), Box::new(b)))
        .map_with(mk_expr_w)
        .padded();

    // match expression: match expr { case pattern: expr; ... else: expr }
    let match_pattern = choice((
        just("_").to(MatchArm::Wildcard),
        number.map(MatchArm::Literal),
        string.map(MatchArm::Literal),
        bool_lit.clone().map(MatchArm::Literal),
        nil_lit.clone().map(MatchArm::Literal),
        ident.map(MatchArm::Bind),
    ))
    .padded();

    // Guarded match arms use `~` (canonical) or `#` (legacy) as the body
    // separator. `ws0_guard` absorbs the separator and any surrounding
    // whitespace in one step. See `guard_expr` (declared near the top of
    // `parser()` and defined just below `cmp_expr`) for the restricted
    // sub-parser that lets the chain stop cleanly before `~`.
    //
    // Syntax: `case IDENT if GUARD ~ BODY;`  (~ is the canonical separator)
    //         `case IDENT if GUARD # BODY;`  (# is legacy, kept for compat)
    let ws0_guard = one_of(" \t\r\n~#").repeated().ignored();

    // Guarded arm: `case ident if guard_expr ~ body`
    //
    // The guard uses the **restricted** `guard_expr` parser declared near
    // the top of `parser()` — it bottoms out at `cmp_expr`, so the full
    // expression's logical/ternary/postfix layers (which trigger the
    // chumsky-0.8 `cmp_op.padded().repeated()` cascade after `n < 0 ~`)
    // never run. `not`, `and`, `or` inside a guard need parentheses around
    // the whole guard, e.g. `case x if (a < 0 and b > 0) ~ body`.
    let guarded_match_arm = just("case")
        .then_ignore(ws1)
        .ignore_then(ident)
        // `ident` is already padded (trailing whitespace consumed), so we
        // can't require ws1 before "if" — use `just("if").padded()` which
        // handles whitespace on both sides of the keyword cleanly.
        .then_ignore(just("if").padded())
        .then(guard_expr.clone()) // restricted parser — stops cleanly before `~`
        .then(
            // The ~ or # separator is absorbed by ws0_guard.
            ws0_guard.ignore_then(expr.clone()),
        )
        .map(|((name, guard), body)| (MatchArm::BindGuard(name, guard), body));

    // Unguarded arm: `case pattern : body` — mandatory `:` separator.
    let unguarded_match_arm = just("case")
        .then_ignore(ws1)
        .ignore_then(match_pattern)
        .then_ignore(just(':').padded())
        .then(
            ws0
                .ignore_then(expr.clone())
                .then_ignore(ws0),
        );

    // match_arm = guarded (guard absorbed in ws) | unguarded (with `:`)
    let match_arm = choice((guarded_match_arm, unguarded_match_arm));
    let match_else = just("else")
        .then_ignore(ws0)
        .then_ignore(just(':').padded().or_not())
        .then_ignore(ws0)
        .ignore_then(expr.clone())
        .then_ignore(just(';').padded().or_not())
        .or_not();
    let match_expr = just("match")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .then_ignore(ws0)
        .then(
            match_arm
                .then_ignore(just(';').padded().or_not())
                .repeated()
                .collect::<Vec<(MatchArm, Expr)>>()
                .then(match_else)
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map(|(scrutinee, (arms, else_expr))| {
            ExprKind::Match(Box::new(scrutinee), arms, else_expr.map(Box::new))
        })
        .map_with(mk_expr_w)
        .padded();

    // switch expression (value-returning):
    // let y = switch x { case 1 { 10 } case 2 { 20 } default { 0 } };
    // Desugars to:
    // { let __switch_<sp.start> = x;
    //   let __switch_out_<sp.start> = nil;
    //   if __switch_.. == 1 { __switch_out_.. = <expr>; } else if ... else { __switch_out_.. = <expr>; }
    //   __switch_out_..
    // }
    let switch_case_expr = just("case")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .then_ignore(ws0)
        .then_ignore(just(':').padded().or_not())
        .then(expr.clone());

    let switch_default_expr = just("default")
        .then_ignore(ws0)
        .then_ignore(just(':').padded().or_not())
        .ignore_then(expr.clone())
        .or_not();

    let switch_expr = just("switch")
        .then_ignore(ws1)
        .ignore_then(expr.clone())
        .then_ignore(ws0)
        .then(
            switch_case_expr
                .repeated()
                .collect::<Vec<(Expr, Expr)>>()
                .then(switch_default_expr)
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map_with(|(scrutinee, (cases, default_expr)), __e| { let sp = __e.span().into_range();
            let sp = Span::from(sp);
            let tmp = format!("__switch_{}", sp.start);
            let out = format!("__switch_out_{}", sp.start);

            let tmp_expr = Expr {
                kind: ExprKind::Var(tmp.clone()),
                span: sp,
            };

            let out_expr = Expr {
                kind: ExprKind::Var(out.clone()),
                span: sp,
            };

            let nil_expr = Expr {
                kind: ExprKind::Nil,
                span: sp,
            };

            let let_tmp = Stmt {
                kind: StmtKind::Let(tmp.clone(), None, scrutinee),
                span: sp,
            };
            let let_out = Stmt {
                kind: StmtKind::Let(out.clone(), None, nil_expr),
                span: sp,
            };

            let mut else_body: Option<Vec<Stmt>> = default_expr.map(|de| {
                vec![Stmt {
                    kind: StmtKind::Assign(out.clone(), de),
                    span: sp,
                }]
            });

            for (case_expr, body_expr) in cases.into_iter().rev() {
                let cond_sp = Span::join(tmp_expr.span, case_expr.span);
                let cond = Expr {
                    kind: ExprKind::Eq(Box::new(tmp_expr.clone()), Box::new(case_expr)),
                    span: cond_sp,
                };

                let then_body = vec![Stmt {
                    kind: StmtKind::Assign(out.clone(), body_expr),
                    span: sp,
                }];

                let if_stmt = Stmt {
                    kind: StmtKind::If(cond, then_body, else_body.take()),
                    span: sp,
                };
                else_body = Some(vec![if_stmt]);
            }

            let mut stmts = vec![let_tmp, let_out];
            if let Some(sts) = else_body {
                stmts.extend(sts);
            }

            Expr {
                kind: ExprKind::BlockExpr(stmts, Some(Box::new(out_expr))),
                span: sp,
            }
        })
        .padded();

    // f-string: f"Hello {name}, count = {n}!"
    // Parses into FString([Literal("Hello "), Interp(name_expr), Literal(", count = "), ...])
    let fstr_literal_part = none_of("{\"\\")
        .or(just('\\').ignore_then(choice((
            just('n').to('\n'),
            just('t').to('\t'),
            just('r').to('\r'),
            just('"').to('"'),
            just('\\').to('\\'),
            just('{').to('{'),
            just('}').to('}'),
        ))))
        .repeated()
        .at_least(1)
        .collect::<String>()
        .map(FStringPart::Literal);
    let fstr_interp_part = just('{')
        .ignore_then(expr.clone())
        .then_ignore(just('}'))
        .map(|e| FStringPart::Interp(Box::new(e)));
    let f_string_expr = just("f\"")
        .ignore_then(
            choice((fstr_interp_part, fstr_literal_part))
                .repeated()
                .collect::<Vec<FStringPart>>(),
        )
        .then_ignore(just('"'))
        .map(ExprKind::FString)
        .map_with(mk_expr_w)
        .padded();

    let primary = choice((
        bool_lit.clone(),
        nil_lit.clone(),
        number,
        f_string_expr,
        string,
        array_lit,
        map_lit,
        match_expr,
        switch_expr,
        try_expr,
        fn_lit_expr,
        if_expr,
        block_expr,
        ident_expr,
        paren_expr,
    ))
    .padded();

    #[derive(Clone, Debug)]
    enum Postfix {
        Call(Vec<Expr>, Span),
        Attr(String, Span),
        Index(Expr, Span),
        Method(String, Vec<Expr>, Span),
        /// `?.field` — nil-safe attribute access
        SafeAttr(String, Span),
        /// `?[idx]` — nil-safe index access
        SafeIndex(Expr, Span),
        /// `?` alone (postfix) — Result unwrap or nil propagation
        ResultUnwrap(Span),
        /// `?? expr` — null coalescing (if left is nil, use right)
        NullCoal(Expr, Span),
    }

    let attr = just('.')
        .ignore_then(text::ident().map(|s: &str| s.to_string()).padded())
        .map_with(|name, __e| Postfix::Attr(name, Span::from(span_of(__e))))
        .padded();

    // `?.field` — nil-safe attribute access
    let safe_attr = just("?.")
        .ignore_then(text::ident().map(|s: &str| s.to_string()).padded())
        .map_with(|name, __e| Postfix::SafeAttr(name, Span::from(span_of(__e))))
        .padded();

    // `?[expr]` — nil-safe index access
    // To avoid embedding the full expr parser inside postfix (depth issue),
    // we reuse the existing `index` parser but prefix it with `?`.
    let safe_index = just('?')
        .ignore_then(expr.clone().delimited_by(just('['), just(']')))
        .map_with(|e, __e| Postfix::SafeIndex(e, Span::from(span_of(__e))))
        .padded();

    // `?` alone — Result unwrap. Must not steal `?` from ternary (`cond ? a : b`).
    // After `?`, unwrap only when the next token is `.`, `[`, `)`, `,`, `;`, or EOF.
    let result_unwrap_pf = just('?')
        .ignore_then(ws0)
        .ignore_then(choice((
            just('.').to(()),
            just('[').to(()),
            just(')').to(()),
            just(',').to(()),
            just(';').to(()),
            end().to(()),
        )))
        .rewind()
        .map_with(|_, __e| { let sp = __e.span().into_range(); Postfix::ResultUnwrap(Span::from(sp)) })
        .padded();

    // `?? expr` — null coalescing as postfix (parsed before single `?` to avoid ambiguity)
    // We define it as a postfix so it binds tighter than logical operators.
    // just("??") must come before result_unwrap_pf in choice to prevent consuming first ?
    let null_coal_pf = just("??")
        .ignore_then(ws0)
        .ignore_then(expr.clone())
        .map_with(|right, __e| { let sp = __e.span().into_range(); Postfix::NullCoal(right, Span::from(sp)) })
        .padded();

    let call = args.clone().map(|(a, sp)| Postfix::Call(a, sp));

    let method = just(':')
        .ignore_then(text::ident().map(|s: &str| s.to_string()).padded())
        .then(args.clone())
        .map_with(|(name, (a, _)), __e| { let sp = __e.span().into_range(); Postfix::Method(name, a, Span::from(sp)) })
        .padded();

    let index = expr
        .clone()
        .delimited_by(just('['), just(']'))
        .map_with(|e, __e| { let sp = __e.span().into_range(); Postfix::Index(e, Span::from(sp)) })
        .padded();

    // postfix: calls, indexing, member access, safe access, result-unwrap (left-to-right)
    // Order matters:
    //   - safe_attr/safe_index must precede result_unwrap_pf (both start with `?`)
    //   - null_coal_pf (??) must precede result_unwrap_pf (single ?) to avoid eating first ?
    let postfix = choice((
        call,
        method,
        index,
        safe_attr,
        safe_index,
        null_coal_pf,
        attr,
        result_unwrap_pf,
    ));

    let call_expr = primary
        .clone()
        .foldl(postfix.repeated(), |left: Expr, pf: Postfix| match pf {
            Postfix::Call(args, arg_sp) => {
                let sp = Span {
                    start: left.span.start,
                    end: arg_sp.end,
                };
                Expr {
                    kind: ExprKind::Call(Box::new(left), args),
                    span: sp,
                }
            }
            Postfix::Index(ix, i_sp) => {
                let sp = Span {
                    start: left.span.start,
                    end: i_sp.end,
                };
                Expr {
                    kind: ExprKind::Index(Box::new(left), Box::new(ix)),
                    span: sp,
                }
            }
            Postfix::Attr(name, a_sp) => {
                let sp = Span {
                    start: left.span.start,
                    end: a_sp.end,
                };
                Expr {
                    kind: ExprKind::GetAttr(Box::new(left), name),
                    span: sp,
                }
            }
            Postfix::SafeAttr(name, a_sp) => {
                let sp = Span {
                    start: left.span.start,
                    end: a_sp.end,
                };
                Expr {
                    kind: ExprKind::SafeGetAttr(Box::new(left), name),
                    span: sp,
                }
            }
            Postfix::SafeIndex(ix, i_sp) => {
                let sp = Span {
                    start: left.span.start,
                    end: i_sp.end,
                };
                Expr {
                    kind: ExprKind::SafeIndex(Box::new(left), Box::new(ix)),
                    span: sp,
                }
            }
            Postfix::ResultUnwrap(u_sp) => {
                let sp = Span {
                    start: left.span.start,
                    end: u_sp.end,
                };
                Expr {
                    kind: ExprKind::ResultUnwrap(Box::new(left)),
                    span: sp,
                }
            }
            Postfix::NullCoal(right, nc_sp) => {
                let sp = Span {
                    start: left.span.start,
                    end: nc_sp.end,
                };
                Expr {
                    kind: ExprKind::NullCoalesce(Box::new(left), Box::new(right)),
                    span: sp,
                }
            }
            Postfix::Method(name, args, m_sp) => {
                let sp = Span {
                    start: left.span.start,
                    end: m_sp.end,
                };

                // Desugar: obj:method(a,b) =>
                // { let __tmp = obj;
                //   let __recv = try __tmp.__class catch __tmp;
                //   __recv.method(__tmp, a, b)
                // }
                let tmp = format!("__self_{}", sp.start);
                let recv = format!("__recv_{}", sp.start);

                let tmp_expr = Expr {
                    kind: ExprKind::Var(tmp.clone()),
                    span: sp,
                };

                let let_tmp = Stmt {
                    kind: StmtKind::Let(tmp.clone(), None, left),
                    span: sp,
                };

                let class_attr = Expr {
                    kind: ExprKind::GetAttr(Box::new(tmp_expr.clone()), "__class".to_string()),
                    span: sp,
                };

                let recv_expr = Expr {
                    kind: ExprKind::Try(Box::new(class_attr), Box::new(tmp_expr.clone())),
                    span: sp,
                };

                let let_recv = Stmt {
                    kind: StmtKind::Let(recv.clone(), None, recv_expr),
                    span: sp,
                };

                let recv_var = Expr {
                    kind: ExprKind::Var(recv),
                    span: sp,
                };

                let callee = Expr {
                    kind: ExprKind::GetAttr(Box::new(recv_var), name),
                    span: sp,
                };

                let mut call_args = vec![tmp_expr];
                for a in args {
                    call_args.push(a);
                }

                let call = Expr {
                    kind: ExprKind::Call(Box::new(callee), call_args),
                    span: sp,
                };

                Expr {
                    kind: ExprKind::BlockExpr(vec![let_tmp, let_recv], Some(Box::new(call))),
                    span: sp,
                }
            }
        });

    // unary: -expr and !expr handled via a mapped prefix option (avoids tripling call_expr depth)
    // `!` maps to ExprKind::Not; `-` maps to 0-sub; absence = pass-through
    #[derive(Clone, Copy)]
    enum Prefix {
        Neg,
        Not,
    }
    let prefix = choice((
        just('!').to(Prefix::Not),
        just("not").to(Prefix::Not),
        just('-').to(Prefix::Neg),
    ))
    .or_not();

    let unary_expr = prefix
        .then(call_expr.clone())
        .map_with(|(pfx, e), __e| { let sp = __e.span().into_range();
            let sp = Span::from(sp);
            match pfx {
                Some(Prefix::Not) => Expr {
                    kind: ExprKind::Not(Box::new(e)),
                    span: sp,
                },
                Some(Prefix::Neg) => {
                    let zero = Expr {
                        kind: ExprKind::Number(0.0),
                        span: sp,
                    };
                    Expr {
                        kind: ExprKind::Sub(Box::new(zero), Box::new(e)),
                        span: sp,
                    }
                }
                None => e,
            }
        });

    // multiplicative: * / % and ** (power)
    // Note: ** must be tried before * to avoid consuming first * of **
    #[derive(Clone, Copy)]
    enum MulExtOp {
        Mul,
        Div,
        Mod,
        Pow,
    }
    let mul_ext_op = choice((
        just("**").to(MulExtOp::Pow), // MUST come before single *
        just('*').to(MulExtOp::Mul),
        just('/').to(MulExtOp::Div),
        just('%').to(MulExtOp::Mod),
    ))
    .padded();

    let mul_expr = unary_expr
        .clone()
        .foldl(mul_ext_op.then(unary_expr.clone()).repeated(), |left: Expr, (op, right): (MulExtOp, Expr)| {
            let sp = Span::join(left.span, right.span);
            let kind = match op {
                MulExtOp::Mul => ExprKind::Mul(Box::new(left), Box::new(right)),
                MulExtOp::Div => ExprKind::Div(Box::new(left), Box::new(right)),
                MulExtOp::Mod => ExprKind::Mod(Box::new(left), Box::new(right)),
                MulExtOp::Pow => ExprKind::Pow(Box::new(left), Box::new(right)),
            };
            Expr { kind, span: sp }
        });

    // additive: + -
    let add_op = choice((just('+').to(AddOp::Add), just('-').to(AddOp::Sub))).padded();

    let add_expr = mul_expr
        .clone()
        .foldl(add_op.then(mul_expr).repeated(), |left: Expr, (op, right): (AddOp, Expr)| {
            let sp = Span::join(left.span, right.span);
            let kind = match op {
                AddOp::Add => ExprKind::Add(Box::new(left), Box::new(right)),
                AddOp::Sub => ExprKind::Sub(Box::new(left), Box::new(right)),
            };
            Expr { kind, span: sp }
        });

    // comparison: == != >= <= > <
    let cmp_op = choice((
        just("==").to(CmpOp::Eq),
        just("!=").to(CmpOp::NotEq),
        just(">=").to(CmpOp::Ge),
        just("<=").to(CmpOp::Le),
        just(">").to(CmpOp::Gt),
        just("<").to(CmpOp::Lt),
    ))
    .padded();

    let cmp_expr = add_expr
        .clone()
        .foldl(cmp_op.then(add_expr).repeated(), |left: Expr, (op, right): (CmpOp, Expr)| {
            let sp = Span::join(left.span, right.span);
            let kind = match op {
                CmpOp::Eq => ExprKind::Eq(Box::new(left), Box::new(right)),
                CmpOp::NotEq => ExprKind::NotEq(Box::new(left), Box::new(right)),
                CmpOp::Gt => ExprKind::Gt(Box::new(left), Box::new(right)),
                CmpOp::Ge => ExprKind::Ge(Box::new(left), Box::new(right)),
                CmpOp::Lt => ExprKind::Lt(Box::new(left), Box::new(right)),
                CmpOp::Le => ExprKind::Le(Box::new(left), Box::new(right)),
            };
            Expr { kind, span: sp }
        });

    // SP1-gamma: define the restricted `guard_expr` parser used by
    // `case <ident> if <guard> ~ <body>` arms.
    //
    // The full `expr` parser triggers a chumsky-0.8 cascade at every
    // arithmetic / comparison layer (`op.padded().repeated()` doesn't
    // stop cleanly when the next token — here `~` — isn't an operator).
    // Using `cmp_expr` alone is not enough either: it still contains
    // `cmp_op.then(add_expr).repeated()` and inherits the same failure.
    //
    // This minimal parser uses **only** `.or_not()` for the optional
    // operator (no `.repeated()`), so guards are limited to the single
    // shape `<atom> [cmp_op <atom>]` where atom = literal | ident |
    // (guard_expr). That covers every guard the test suite needs
    // (`n < 0`, `n == 0`, `n >= 10`) without touching the arithmetic /
    // logical / postfix ladders. For composite guards, wrap in parens
    // and use the `(guard_expr)` recursion. Logical and/or are not
    // supported inside guards (compute outside the match if needed).
    let guard_cmp_op = choice((
        just("==").to(CmpOp::Eq),
        just("!=").to(CmpOp::NotEq),
        just(">=").to(CmpOp::Ge),
        just("<=").to(CmpOp::Le),
        just(">").to(CmpOp::Gt),
        just("<").to(CmpOp::Lt),
    ))
    .padded();
    let guard_atom = choice((
        bool_lit.clone(),
        nil_lit.clone(),
        number,
        string,
        ident.map(ExprKind::Var).map_with(mk_expr_w),
        just('(')
            .padded()
            .ignore_then(guard_expr.clone())
            .then_ignore(just(')').padded()),
    ));
    guard_expr.define(
        guard_atom
            .clone()
            .then(guard_cmp_op.then(guard_atom).or_not())
            .map(|(left, opt)| match opt {
                Some((op, right)) => {
                    let sp = Span::join(left.span, right.span);
                    let kind = match op {
                        CmpOp::Eq => ExprKind::Eq(Box::new(left), Box::new(right)),
                        CmpOp::NotEq => ExprKind::NotEq(Box::new(left), Box::new(right)),
                        CmpOp::Gt => ExprKind::Gt(Box::new(left), Box::new(right)),
                        CmpOp::Ge => ExprKind::Ge(Box::new(left), Box::new(right)),
                        CmpOp::Lt => ExprKind::Lt(Box::new(left), Box::new(right)),
                        CmpOp::Le => ExprKind::Le(Box::new(left), Box::new(right)),
                    };
                    Expr { kind, span: sp }
                }
                None => left,
            }),
    );

    // logical: and/or
    let logical_op =
        choice((just("and").to(LogicalOp::And), just("or").to(LogicalOp::Or))).padded();

    let logical_expr = cmp_expr
        .clone()
        .foldl(logical_op.then(cmp_expr).repeated(), |left: Expr, (op, right): (LogicalOp, Expr)| {
            let sp = Span::join(left.span, right.span);
            Expr {
                kind: ExprKind::Logical {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
                span: sp,
            }
        });

    // Ternary: `cond ? then : else` (desugars to IfExpr with block arms).
    let expr_top = logical_expr
        .clone()
        .then(
            just('?')
                .padded()
                .ignore_then(logical_expr.clone())
                .then_ignore(just(':').padded())
                .then(logical_expr.clone())
                .map(|(then_e, else_e)| (then_e, else_e))
                .or_not(),
        )
        .map(|(cond, opt)| {
            if let Some((then_e, else_e)) = opt {
                let sp = Span::join(cond.span, else_e.span);
                let then_block = Expr {
                    kind: ExprKind::BlockExpr(vec![], Some(Box::new(then_e))),
                    span: sp,
                };
                let else_block = Expr {
                    kind: ExprKind::BlockExpr(vec![], Some(Box::new(else_e))),
                    span: sp,
                };
                Expr {
                    kind: ExprKind::IfExpr(
                        Box::new(cond),
                        Box::new(then_block),
                        Box::new(else_block),
                    ),
                    span: sp,
                }
            } else {
                cond
            }
        });

    expr.define(expr_top);

    let stmt = stmt.then_ignore(just(';').padded().or_not());

    stmt.repeated().collect::<Vec<Stmt>>().then_ignore(end())

    // end parser()
}
