//"G:\panag\Documents\projects\AAP\pl-dev\sapphire\crates\sapphire-core\src\parser.rs"

use chumsky::prelude::*;

use crate::{Expr, ExprKind, LogicalOp, MatchArm, Param, Span, Stmt, StmtKind, TypeExpr, TypeKind};

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

#[derive(Clone, Copy, Debug)]
enum MulOp {
    Mul,
    Div,
}

pub fn parser() -> impl Parser<char, Vec<Stmt>, Error = Simple<char>> {
    let ws0 = one_of(" \t\r\n").repeated().ignored();
    let ws1 = one_of(" \t\r\n").repeated().at_least(1).ignored();

    let number = text::int(10)
        .then(just('.').ignore_then(text::digits(10)).or_not())
        .map(|(int_part, frac_part): (String, Option<String>)| {
            let s = match frac_part {
                Some(frac) => format!("{int_part}.{frac}"),
                None => int_part,
            };
            ExprKind::Number(s.parse::<f64>().unwrap())
        })
        .map_with_span(|k, sp| mk_expr(k, sp))
        .padded();

    // String content: either non-escape char or \n \t \r \" \\
    let escape = just('\\').then(choice((
        just('n').to('\n'),
        just('t').to('\t'),
        just('r').to('\r'),
        just('"').to('"'),
        just('\\').to('\\'),
    )));
    let string_char = choice((
        none_of("\"\\").map(|c| c),
        escape.map(|(_, c)| c),
    ));
    let string = just('"')
        .ignore_then(string_char.clone().repeated())
        .then_ignore(just('"'))
        .map(|chars| ExprKind::Str(chars.into_iter().collect()))
        .map_with_span(|k, sp| mk_expr(k, sp))
        .padded();

    let escape_raw = just('\\').then(choice((
        just('n').to('\n'),
        just('t').to('\t'),
        just('r').to('\r'),
        just('"').to('"'),
        just('\\').to('\\'),
    )));
    let string_char_raw = choice((
        none_of("\"\\").map(|c| c),
        escape_raw.map(|(_, c)| c),
    ));
    let string_raw = just('"')
        .ignore_then(string_char_raw.repeated())
        .then_ignore(just('"'))
        .map(|chars| chars.into_iter().collect::<String>())
        .padded();

    let bool_lit = choice((
        just("true").to(ExprKind::Bool(true)),
        just("false").to(ExprKind::Bool(false)),
    ))
    .map_with_span(|k, sp| mk_expr(k, sp))
    .padded();

    let nil_lit = just("nil")
        .to(ExprKind::Nil)
        .map_with_span(|k, sp| mk_expr(k, sp))
        .padded();

    let ident = text::ident().padded();

    // Mutual recursion: expression blocks need statements, and statements need expressions.
    let mut expr = Recursive::declare();
    let mut stmt = Recursive::declare();

    // -------------------- statements --------------------
    let inner_stmt0 = stmt.clone().then_ignore(just(';').padded().or_not());

    let block_stmt = just('{')
        .ignore_then(ws0.clone())
        .ignore_then(
            inner_stmt0
                .then_ignore(ws0.clone())
                .repeated()
                .collect::<Vec<Stmt>>(),
        )
        .then_ignore(just('}'))
        .map(StmtKind::Block)
        .map_with_span(|k, sp| mk_stmt(k, sp))
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
    .map_with_span(|k, sp| mk_type(k, sp))
    .padded();

    let ty_array = just("array")
        .ignore_then(just('<').padded())
        .ignore_then(ty.clone())
        .then_ignore(just('>').padded())
        .map(|inner| TypeKind::Array(Box::new(inner)))
        .map_with_span(|k, sp| mk_type(k, sp))
        .padded();

    let ty_map = just("map")
        .ignore_then(just('<').padded())
        .ignore_then(ty.clone())
        .then_ignore(just('>').padded())
        .map(|inner| TypeKind::Map(Box::new(inner)))
        .map_with_span(|k, sp| mk_type(k, sp))
        .padded();

    ty.define(choice((ty_array, ty_map, ty_atom)));

    let type_ann = just(':').padded().ignore_then(ty.clone()).or_not();

    let param = text::ident()
        .padded()
        .then(type_ann.clone())
        .map(|(name, ty)| (name, ty));

    let params = param
        .separated_by(just(',').padded())
        .allow_trailing()
        .collect::<Vec<Param>>()
        .delimited_by(just('('), just(')'))
        .padded();

    let ret_ann = just("->").padded().ignore_then(ty.clone()).or_not();

    let fn_stmt = just("fn")
        .then_ignore(ws1.clone())
        .ignore_then(text::ident().padded())
        .then(params.clone())
        .then(ret_ann.clone())
        .then(block_stmt.clone().map(|s| match s.kind {
            StmtKind::Block(body) => body,
            _ => unreachable!("block parser must produce StmtKind::Block"),
        }))
        .map(|(((name, params), ret), body)| StmtKind::Fn(name, params, ret, body))
        .map_with_span(|k, sp| mk_stmt(k, sp));

    let printf_stmt = just("printf")
        .then_ignore(ws1.clone())
        .ignore_then(text::int(10).from_str::<i64>().unwrapped())
        .then_ignore(ws1.clone())
        .then(expr.clone())
        .map(|(p, e)| StmtKind::PrintF(p as usize, e))
        .map_with_span(|k, sp| mk_stmt(k, sp));

    let print_stmt = just("print")
        .then_ignore(ws1.clone())
        .ignore_then(expr.clone())
        .map(StmtKind::Print)
        .map_with_span(|k, sp| mk_stmt(k, sp));

    let return_stmt = just("return")
        .ignore_then(expr.clone().or_not())
        .map(StmtKind::Return)
        .map_with_span(|k, sp| mk_stmt(k, sp))
        .padded();

    let defer_stmt = just("defer")
        .then_ignore(ws1.clone())
        .then(expr.clone())
        .map(|(_, e)| StmtKind::Defer(e))
        .map_with_span(|k, sp| mk_stmt(k, sp))
        .padded();

    let import_stmt = just("import")
        .then_ignore(ws1.clone())
        .ignore_then(string_raw.clone())
        .map(StmtKind::Import)
        .map_with_span(|k, sp| mk_stmt(k, sp));

    let export_stmt = just("export")
        .then_ignore(ws1.clone())
        .ignore_then(
            text::ident()
                .padded()
                .separated_by(just(',').padded())
                .allow_trailing()
                .collect::<Vec<String>>(),
        )
        .map(StmtKind::Export)
        .map_with_span(|k, sp| mk_stmt(k, sp));

    let let_stmt = just("let")
        .then_ignore(ws1.clone())
        .ignore_then(text::ident())
        .then(type_ann.clone())
        .then_ignore(just('=').padded())
        .then(expr.clone())
        .map(|((name, ty), expr)| StmtKind::Let(name, ty, expr))
        .map_with_span(|k, sp| mk_stmt(k, sp));

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
        .ignore_then(text::ident().padded())
        .map_with_span(|name, sp| LPostfix::Attr(name, Span::from(sp)))
        .padded();

    let l_index = expr
        .clone()
        .delimited_by(just('['), just(']'))
        .map_with_span(|e, sp| LPostfix::Index(e, Span::from(sp)))
        .padded();

    let lvalue = text::ident()
        .padded()
        .map(ExprKind::Var)
        .map_with_span(|k, sp| mk_expr(k, sp))
        .then(choice((l_attr, l_index)).repeated())
        .foldl(|left, pf| match pf {
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

    let set_stmt = lvalue
        .then_ignore(just('=').padded())
        .then(expr.clone())
        .map(|(lhs, rhs)| match lhs.kind {
            ExprKind::Var(name) => StmtKind::Assign(name, rhs),
            ExprKind::GetAttr(obj, name) => StmtKind::SetAttr(*obj, name, rhs),
            ExprKind::Index(obj, ix) => StmtKind::SetIndex(*obj, *ix, rhs),
            _ => unreachable!("lvalue parser only builds Var/GetAttr/Index"),
        })
        .map_with_span(|k, sp| mk_stmt(k, sp));

    let expr_stmt = expr
        .clone()
        .map(StmtKind::Expr)
        .map_with_span(|k, sp| mk_stmt(k, sp));

    // if/elif/else needs self-reference for `else if`
    let mut if_stmt = Recursive::declare();
    let block_body = block_stmt.clone().map(|s| match s.kind {
        StmtKind::Block(body) => body,
        _ => unreachable!("block parser must produce StmtKind::Block"),
    });

    let elif_clause = just("elif")
        .then_ignore(ws1.clone())
        .ignore_then(expr.clone())
        .then(block_body.clone());

    let else_clause = just("else")
        .then_ignore(ws1.clone())
        .ignore_then(choice((
            block_body.clone(),
            if_stmt.clone().map(|s| vec![s]),
        )))
        .or_not();

    let if_stmt_impl = just("if")
        .then_ignore(ws1.clone())
        .ignore_then(expr.clone())
        .then(block_body.clone())
        .then(elif_clause.repeated().collect::<Vec<(Expr, Vec<Stmt>)>>())
        .then(else_clause)
        .map_with_span(|(((cond, then_body), elifs), else_body), sp| {
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
        .then_ignore(ws1.clone())
        .ignore_then(expr.clone())
        .then(block_stmt.clone().map(|s| match s.kind {
            StmtKind::Block(body) => body,
            _ => unreachable!("block parser must produce StmtKind::Block"),
        }))
        .map(|(cond, body)| StmtKind::While(cond, body))
        .map_with_span(|k, sp| mk_stmt(k, sp));

    let for_stmt = just("for")
        .then_ignore(ws1.clone())
        .ignore_then(text::ident())
        .then_ignore(ws1.clone())
        .then(expr.clone())
        .then(block_stmt.clone().map(|s| match s.kind {
            StmtKind::Block(body) => body,
            _ => unreachable!("block parser must produce StmtKind::Block"),
        }))
        .map(|((name, count), body)| StmtKind::For(name, count, body))
        .map_with_span(|k, sp| mk_stmt(k, sp));

    // class statement (OOP sugar):
    // class Name {
    //   fn method(self, x) { ... }
    //   fn other() { ... }
    // }
    // Desugars to:
    // let Name = map{ method: fn method(self, x) { ... }, other: fn other() { ... } };
    let class_method = just("fn")
        .then_ignore(ws1.clone())
        .ignore_then(text::ident().padded())
        .then(params.clone())
        .then(ret_ann.clone())
        .then(block_body.clone())
        .map_with_span(|(((name, params), ret), body), sp| {
            let sp = Span::from(sp);
            let f = Expr {
                kind: ExprKind::FnLit(Some(name.clone()), params, ret, body),
                span: sp,
            };
            (name, f)
        });

    let class_stmt = just("class")
        .then_ignore(ws1.clone())
        .ignore_then(text::ident().padded())
        .then_ignore(ws0.clone())
        .then(
            class_method
                .then_ignore(just(';').padded().or_not())
                .repeated()
                .collect::<Vec<(String, Expr)>>()
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map_with_span(|(name, methods), sp| {
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
                    params.push((format!("a{i}"), None));
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
                            "new0".to_string(),
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

            let mut block_stmts =
                vec![let_class, set_new0, set_new1, set_new2, set_new3, set_new];
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
        .ignore_then(ws0.clone())
        .ignore_then(
            g_inner_stmt0
                .then_ignore(ws0.clone())
                .repeated()
                .collect::<Vec<Stmt>>(),
        )
        .then_ignore(just('}'))
        .map(StmtKind::Block)
        .map_with_span(|k, sp| mk_stmt(k, sp))
        .padded();

    let gblock_body = gblock_stmt.clone().map(|s| match s.kind {
        StmtKind::Block(body) => body,
        _ => unreachable!("gblock parser must produce StmtKind::Block"),
    });

    let goto_jump_stmt = just("goto")
        .then_ignore(ws1.clone())
        .ignore_then(text::ident().padded())
        .map_with_span(|label, sp| {
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

    let end_stmt = just("end").map_with_span(|_, sp| {
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
        .then_ignore(ws1.clone())
        .ignore_then(expr.clone())
        .then(gblock_body.clone());

    let g_else_clause = just("else")
        .then_ignore(ws1.clone())
        .ignore_then(choice((
            gblock_body.clone(),
            if_stmt.clone().map(|s| vec![s]),
        )))
        .or_not();

    let g_if_stmt = just("if")
        .then_ignore(ws1.clone())
        .ignore_then(expr.clone())
        .then(gblock_body.clone())
        .then(g_elif_clause.repeated().collect::<Vec<(Expr, Vec<Stmt>)>>())
        .then(g_else_clause)
        .map_with_span(|(((cond, then_body), elifs), else_body), sp| {
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
        .then_ignore(ws1.clone())
        .ignore_then(expr.clone())
        .then(gblock_body.clone())
        .map(|(cond, body)| StmtKind::While(cond, body))
        .map_with_span(|k, sp| mk_stmt(k, sp));

    let g_for_stmt = just("for")
        .then_ignore(ws1.clone())
        .ignore_then(text::ident())
        .then_ignore(ws1.clone())
        .then(expr.clone())
        .then(gblock_body.clone())
        .map(|((name, count), body)| StmtKind::For(name, count, body))
        .map_with_span(|k, sp| mk_stmt(k, sp));

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
        g_for_stmt,
        gblock_stmt.clone(),
        set_stmt.clone(),
        expr_stmt.clone(),
    ))
    .padded();

    gstmt.define(g_stmt_impl);

    let goto_label = just("label")
        .then_ignore(ws1.clone())
        .ignore_then(text::ident().padded())
        .then(gblock_body.clone());

    let goto_block_stmt = just("goto")
        .then_ignore(ws0.clone())
        .ignore_then(
            goto_label
                .repeated()
                .at_least(1)
                .collect::<Vec<(String, Vec<Stmt>)>>()
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map_with_span(|labels, sp| {
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
        .then_ignore(ws1.clone())
        .ignore_then(expr.clone())
        .then(block_stmt.clone().map(|s| match s.kind {
            StmtKind::Block(body) => body,
            _ => unreachable!("block parser must produce StmtKind::Block"),
        }));

    let switch_default = just("default")
        .then_ignore(ws0.clone())
        .ignore_then(block_stmt.clone().map(|s| match s.kind {
            StmtKind::Block(body) => body,
            _ => unreachable!("block parser must produce StmtKind::Block"),
        }))
        .or_not();

    let switch_stmt = just("switch")
        .then_ignore(ws1.clone())
        .ignore_then(expr.clone())
        .then_ignore(ws0.clone())
        .then(
            switch_case
                .repeated()
                .collect::<Vec<(Expr, Vec<Stmt>)>>()
                .then(switch_default)
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map_with_span(|(scrutinee, (cases, default_body)), sp| {
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
    let stmt_impl = choice((
        fn_stmt,
        printf_stmt,
        print_stmt,
        return_stmt,
        defer_stmt,
        import_stmt,
        export_stmt,
        class_stmt,
        goto_block_stmt,
        let_stmt,
        if_stmt.clone(),
        while_stmt,
        for_stmt,
        switch_stmt,
        block_stmt.clone(), // <- must come before assign_stmt
        set_stmt,
        expr_stmt,
    ))
    .padded();

    stmt.define(stmt_impl);

    // -------------------- expressions --------------------
    let paren_expr = expr.clone().delimited_by(just('('), just(')')).padded();

    let args = expr
        .clone()
        .separated_by(just(',').padded())
        .allow_trailing()
        .collect::<Vec<Expr>>()
        .delimited_by(just('('), just(')'))
        .map_with_span(|args, sp| (args, Span::from(sp)))
        .padded();

    let ident_expr = ident
        .clone()
        .map(ExprKind::Var)
        .map_with_span(|k, sp| mk_expr(k, sp));

    // Function literal expression:
    // fn (a, b) { ... }
    // fn name(a, b) { ... }   (optional name, mainly for debugging/stack traces later)
    let fn_lit_name = ws1.clone().ignore_then(text::ident().padded()).or_not();

    let fn_lit_expr = just("fn")
        .ignore_then(fn_lit_name)
        .then(params.clone())
        .then(ret_ann.clone())
        .then(block_body.clone())
        .map(|(((name_opt, params), ret), body)| ExprKind::FnLit(name_opt, params, ret, body))
        .map_with_span(|k, sp| mk_expr(k, sp))
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
        .map_with_span(|k, sp| mk_expr(k, sp))
        .padded();

    let array_lit = expr
        .clone()
        .separated_by(just(',').padded())
        .allow_trailing()
        .collect::<Vec<Expr>>()
        .delimited_by(just('['), just(']'))
        .map(ExprKind::Array)
        .map_with_span(|k, sp| mk_expr(k, sp))
        .padded();

    let map_key = choice((
        // allow: foo: 1
        text::ident().padded(),
        // allow: "foo": 1
        string_raw.clone(),
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
        .map_with_span(|k, sp| mk_expr(k, sp))
        .padded();

    let try_expr = just("try")
        .then_ignore(ws1.clone())
        .ignore_then(expr.clone())
        .then_ignore(just("catch").padded())
        .then(expr.clone())
        .map(|(a, b)| ExprKind::Try(Box::new(a), Box::new(b)))
        .map_with_span(|k, sp| mk_expr(k, sp))
        .padded();

    // match expression: match expr { case pattern: expr; ... else: expr }
    let match_pattern = choice((
        just("_").to(MatchArm::Wildcard),
        number.clone().map(MatchArm::Literal),
        string.clone().map(MatchArm::Literal),
        bool_lit.clone().map(MatchArm::Literal),
        nil_lit.clone().map(MatchArm::Literal),
        ident.clone().map(MatchArm::Bind),
    ))
    .padded();
    let match_arm = just("case")
        .then_ignore(ws1.clone())
        .ignore_then(match_pattern)
        .then(
            ws1.clone()
                .ignore_then(just("if"))
                .ignore_then(ws1.clone())
                .ignore_then(expr.clone())
                .or_not(),
        )
        .then_ignore(ws0.clone())
        .then_ignore(just(':').padded().or_not())
        .then(ws0.clone().ignore_then(expr.clone()).then_ignore(ws0.clone()))
        .map(|((pattern, guard_opt), body)| {
            let arm = match (pattern, guard_opt) {
                (MatchArm::Bind(name), Some(guard)) => MatchArm::BindGuard(name, guard),
                (p, _) => p,
            };
            (arm, body)
        });
    let match_else = just("else")
        .then_ignore(ws0.clone())
        .then_ignore(just(':').padded().or_not())
        .then_ignore(ws0.clone())
        .ignore_then(expr.clone())
        .then_ignore(just(';').padded().or_not())
        .or_not();
    let match_expr = just("match")
        .then_ignore(ws1.clone())
        .ignore_then(expr.clone())
        .then_ignore(ws0.clone())
        .then(
            match_arm
                .then_ignore(just(';').padded().or_not())
                .repeated()
                .collect::<Vec<(MatchArm, Expr)>>()
                .then(match_else)
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map_with_span(|(scrutinee, (arms, else_expr)), _sp| {
            ExprKind::Match(Box::new(scrutinee), arms, else_expr.map(Box::new))
        })
        .map_with_span(|k, sp| mk_expr(k, sp))
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
        .then_ignore(ws1.clone())
        .ignore_then(expr.clone())
        .then_ignore(ws0.clone())
        .then_ignore(just(':').padded().or_not())
        .then(expr.clone());

    let switch_default_expr = just("default")
        .then_ignore(ws0.clone())
        .then_ignore(just(':').padded().or_not())
        .ignore_then(expr.clone())
        .or_not();

    let switch_expr = just("switch")
        .then_ignore(ws1.clone())
        .ignore_then(expr.clone())
        .then_ignore(ws0.clone())
        .then(
            switch_case_expr
                .repeated()
                .collect::<Vec<(Expr, Expr)>>()
                .then(switch_default_expr)
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map_with_span(|(scrutinee, (cases, default_expr)), sp| {
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

    let primary = choice((
        bool_lit.clone(),
        nil_lit.clone(),
        number.clone(),
        string.clone(),
        array_lit,
        map_lit,
        match_expr,
        switch_expr,
        try_expr,
        fn_lit_expr,
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
    }

    let attr = just('.')
        .ignore_then(text::ident().padded())
        .map_with_span(|name, sp| Postfix::Attr(name, Span::from(sp)))
        .padded();

    let call = args.clone().map(|(a, sp)| Postfix::Call(a, sp));

    let method = just(':')
        .ignore_then(text::ident().padded())
        .then(args.clone())
        .map_with_span(|(name, (a, _)), sp| Postfix::Method(name, a, Span::from(sp)))
        .padded();

    let index = expr
        .clone()
        .delimited_by(just('['), just(']'))
        .map_with_span(|e, sp| Postfix::Index(e, Span::from(sp)))
        .padded();

    // postfix: calls, indexing, member access (left-to-right)
    let postfix = choice((call, method, index, attr));

    let call_expr = primary
        .clone()
        .then(postfix.repeated())
        .foldl(|left, pf| match pf {
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

    // unary: - expr (so that "else: -1" and "-x" parse; + is no-op)
    let unary_expr = just('-')
        .or_not()
        .then(call_expr.clone())
        .map_with_span(|(neg, e), sp| {
            let sp = Span::from(sp);
            if neg.is_some() {
                let zero = Expr {
                    kind: ExprKind::Number(0.0),
                    span: sp,
                };
                Expr {
                    kind: ExprKind::Sub(Box::new(zero), Box::new(e)),
                    span: sp,
                }
            } else {
                e
            }
        })
        .padded();

    // multiplicative: * /
    let mul_op = choice((just('*').to(MulOp::Mul), just('/').to(MulOp::Div))).padded();

    let mul_expr = unary_expr
        .clone()
        .then(mul_op.then(unary_expr.clone()).repeated())
        .foldl(|left, (op, right)| {
            let sp = Span::join(left.span, right.span);
            let kind = match op {
                MulOp::Mul => ExprKind::Mul(Box::new(left), Box::new(right)),
                MulOp::Div => ExprKind::Div(Box::new(left), Box::new(right)),
            };
            Expr { kind, span: sp }
        });

    // additive: + -
    let add_op = choice((just('+').to(AddOp::Add), just('-').to(AddOp::Sub))).padded();

    let add_expr = mul_expr
        .clone()
        .then(add_op.then(mul_expr).repeated())
        .foldl(|left, (op, right)| {
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
        .then(cmp_op.then(add_expr).repeated())
        .foldl(|left, (op, right)| {
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

    // logical: and/or (lowest precedence)
    let logical_op =
        choice((just("and").to(LogicalOp::And), just("or").to(LogicalOp::Or))).padded();

    let logical_expr = cmp_expr
        .clone()
        .then(logical_op.then(cmp_expr).repeated())
        .foldl(|left, (op, right)| {
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

    expr.define(logical_expr);

    let stmt = stmt.then_ignore(just(';').padded().or_not());

    stmt.repeated().collect::<Vec<Stmt>>().then_ignore(end())

    // end parser()
}
