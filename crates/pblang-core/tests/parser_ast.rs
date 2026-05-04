use pblang_core::{parse, ExprKind, StmtKind};

#[test]
fn parse_print_add() {
    let ast = parse("print 1 + 2;").unwrap();
    assert_eq!(ast.len(), 1);
    match &ast[0].kind {
        StmtKind::Print(e) => match &e.kind {
            ExprKind::Add(a, b) => {
                assert!(matches!(a.kind, ExprKind::Number(n) if n == 1.0));
                assert!(matches!(b.kind, ExprKind::Number(n) if n == 2.0));
            }
            _ => panic!("expected add expr"),
        },
        _ => panic!("expected print stmt"),
    }
}

#[test]
fn parse_let_and_var() {
    let ast = parse("let x = 40; print x + 2;").unwrap();
    assert_eq!(ast.len(), 2);

    match &ast[0].kind {
        StmtKind::Let(name, _, init) => {
            assert_eq!(name, "x");
            assert!(matches!(init.kind, ExprKind::Number(n) if n == 40.0));
        }
        _ => panic!("expected let stmt"),
    }

    match &ast[1].kind {
        StmtKind::Print(e) => match &e.kind {
            ExprKind::Add(a, b) => {
                assert!(matches!(a.kind, ExprKind::Var(ref s) if s == "x"));
                assert!(matches!(b.kind, ExprKind::Number(n) if n == 2.0));
            }
            _ => panic!("expected add expr"),
        },
        _ => panic!("expected print stmt"),
    }
}

#[test]
fn parse_fn_return_and_call() {
    let ast = parse("fn five() { return 5; } print five();").unwrap();
    assert_eq!(ast.len(), 2);

    match &ast[0].kind {
        StmtKind::Fn(name, params, _, body) => {
            assert_eq!(name, "five");
            assert!(params.is_empty());
            assert_eq!(body.len(), 1);
            match &body[0].kind {
                StmtKind::Return(Some(e)) => {
                    assert!(matches!(e.kind, ExprKind::Number(n) if n == 5.0));
                }
                _ => panic!("expected return 5"),
            }
        }
        _ => panic!("expected fn stmt"),
    }

    match &ast[1].kind {
        StmtKind::Print(e) => match &e.kind {
            ExprKind::Call(callee, args) => {
                assert!(args.is_empty());
                assert!(matches!(callee.kind, ExprKind::Var(ref s) if s == "five"));
            }
            _ => panic!("expected call expr"),
        },
        _ => panic!("expected print stmt"),
    }
}

#[test]
fn parse_if_while_for_shapes() {
    // These are parsed today even if compilation is not implemented yet.
    let ast = parse("if true { print 1; } while true { print 1; } for i 3 { print i; }").unwrap();
    assert_eq!(ast.len(), 3);

    match &ast[0] {
        s if matches!(s.kind, StmtKind::If(_, _, _)) => {
            if let StmtKind::If(cond, body, else_body) = &s.kind {
                assert!(matches!(cond.kind, ExprKind::Bool(true)));
                assert_eq!(body.len(), 1);
                assert!(else_body.is_none());
            }
        }
        _ => panic!("expected if"),
    }

    match &ast[1] {
        s if matches!(s.kind, StmtKind::While(_, _)) => {
            if let StmtKind::While(cond, body) = &s.kind {
                assert!(matches!(cond.kind, ExprKind::Bool(true)));
                assert_eq!(body.len(), 1);
            }
        }
        _ => panic!("expected while"),
    }

    match &ast[2] {
        s if matches!(s.kind, StmtKind::For(_, _, _)) => {
            if let StmtKind::For(name, count, body) = &s.kind {
                assert_eq!(name, "i");
                assert!(matches!(count.kind, ExprKind::Number(n) if n == 3.0));
                assert_eq!(body.len(), 1);
            }
        }
        _ => panic!("expected for"),
    }
}
