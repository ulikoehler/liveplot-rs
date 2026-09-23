use liveplot::data::expr::{parse, BinOp, Expr, Func};
use liveplot::data::traces::TraceRef;
use std::collections::HashMap;

fn eval_with(ast: &Expr, t: f64, vals: &[(&str, f64)]) -> f64 {
    let map: HashMap<TraceRef, f64> = vals.iter().map(|(n, v)| (TraceRef::new(*n), *v)).collect();
    ast.eval(t, &mut |name: &TraceRef| map.get(name).copied())
}

fn eval_str(src: &str, t: f64, vals: &[(&str, f64)]) -> f64 {
    let ast = parse(src).unwrap();
    eval_with(&ast, t, vals)
}

#[test]
fn test_precedence() {
    assert_eq!(eval_str("1 + 2 * 3", 0.0, &[]), 7.0);
    assert_eq!(eval_str("(1 + 2) * 3", 0.0, &[]), 9.0);
    assert_eq!(eval_str("10 - 4 - 3", 0.0, &[]), 3.0); // left assoc
    assert_eq!(eval_str("8 / 4 / 2", 0.0, &[]), 1.0);
}

#[test]
fn test_power_right_assoc_and_unary() {
    assert_eq!(eval_str("2^3^2", 0.0, &[]), 512.0); // 2^(3^2)
    assert_eq!(eval_str("-2^2", 0.0, &[]), -4.0); // -(2^2)
    assert_eq!(eval_str("2^-1", 0.0, &[]), 0.5);
    assert_eq!(eval_str("(-2)^2", 0.0, &[]), 4.0);
}

#[test]
fn test_implicit_multiplication() {
    assert_eq!(eval_str("2 {a}", 0.0, &[("a", 3.0)]), 6.0);
    assert_eq!(eval_str("{a}{b}", 0.0, &[("a", 3.0), ("b", 4.0)]), 12.0);
    assert_eq!(eval_str("2(1+1)", 0.0, &[]), 4.0);
    assert_eq!(eval_str("(1+1)(1+2)", 0.0, &[]), 6.0);
    assert_eq!(eval_str("2 t", 3.0, &[]), 6.0);
    assert!((eval_str("2pi", 0.0, &[]) - std::f64::consts::TAU).abs() < 1e-12);
}

#[test]
fn test_trace_refs_and_time() {
    assert_eq!(eval_str("{a} + {b}", 0.0, &[("a", 1.5), ("b", 2.5)]), 4.0);
    // Names with spaces and dots
    assert_eq!(
        eval_str(
            "{my signal} + {ch.1}",
            0.0,
            &[("my signal", 1.0), ("ch.1", 2.0)]
        ),
        3.0
    );
    // t is the sample timestamp
    assert_eq!(eval_str("t * 2", 1.5, &[]), 3.0);
    // A trace literally named "t" is still reachable via braces
    assert_eq!(eval_str("{t}", 5.0, &[("t", 42.0)]), 42.0);
}

#[test]
fn test_functions() {
    let e = std::f64::consts::E;
    assert_eq!(eval_str("sqrt(9)", 0.0, &[]), 3.0);
    assert_eq!(eval_str("root(27, 3)", 0.0, &[]), 3.0);
    assert_eq!(eval_str("root(-8, 3)", 0.0, &[]), -2.0); // odd root of negative
    assert!((eval_str("exp(1)", 0.0, &[]) - e).abs() < 1e-12);
    assert_eq!(eval_str("abs(-3)", 0.0, &[]), 3.0);
    assert!((eval_str("ln({e})", 0.0, &[("e", e)]) - 1.0).abs() < 1e-12);
    assert_eq!(eval_str("log10(1000)", 0.0, &[]), 3.0);
    assert_eq!(eval_str("log2(8)", 0.0, &[]), 3.0);
    assert_eq!(eval_str("log(100)", 0.0, &[]), 2.0); // single-arg log = base 10
    assert_eq!(eval_str("log(8, 2)", 0.0, &[]), 3.0); // two-arg = arbitrary base
    assert!((eval_str("sin(pi/2)", 0.0, &[]) - 1.0).abs() < 1e-12);
    assert!((eval_str("cos(pi)", 0.0, &[]) + 1.0).abs() < 1e-12);
    assert!((eval_str("tan(0.5)", 0.0, &[]) - 0.5f64.tan()).abs() < 1e-12);
    assert!((eval_str("asin(1)", 0.0, &[]) - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    assert_eq!(eval_str("acos(1)", 0.0, &[]), 0.0);
    assert_eq!(eval_str("atan(1)", 0.0, &[]), std::f64::consts::FRAC_PI_4);
    // atan2(y, x) argument order
    assert_eq!(
        eval_str("atan2(1, 0)", 0.0, &[]),
        std::f64::consts::FRAC_PI_2
    );
    assert!((eval_str("sinh(1)", 0.0, &[]) - 1.0f64.sinh()).abs() < 1e-12);
    assert!((eval_str("cosh(1)", 0.0, &[]) - 1.0f64.cosh()).abs() < 1e-12);
    assert!((eval_str("tanh(1)", 0.0, &[]) - 1.0f64.tanh()).abs() < 1e-12);
    assert!((eval_str("asinh(1)", 0.0, &[]) - 1.0f64.asinh()).abs() < 1e-12);
    assert!((eval_str("acosh(2)", 0.0, &[]) - 2.0f64.acosh()).abs() < 1e-12);
    assert!((eval_str("atanh(0.5)", 0.0, &[]) - 0.5f64.atanh()).abs() < 1e-12);
}

#[test]
fn test_nan_propagation() {
    // Missing trace -> NaN
    assert!(eval_str("{missing} + 1", 0.0, &[]).is_nan());
    // Domain error -> NaN
    assert!(eval_str("sqrt(-1)", 0.0, &[]).is_nan());
    // Division by zero -> inf (caller filters non-finite)
    assert!(eval_str("1/0", 0.0, &[]).is_infinite());
}

#[test]
fn test_referenced_traces() {
    let ast = parse("{b} + {a} * sin({a}) + t + pi").unwrap();
    assert_eq!(
        ast.referenced_traces(),
        vec![TraceRef::new("a"), TraceRef::new("b")]
    );
    let ast = parse("sin(2*pi*t)").unwrap();
    assert!(ast.referenced_traces().is_empty());
}

#[test]
fn test_parse_errors() {
    // Unknown bare identifier with braces hint
    let e = parse("a + 1").unwrap_err();
    assert!(e.msg.contains("{a}"), "hint missing: {}", e.msg);
    // Unclosed brace
    assert!(parse("{a").is_err());
    // Empty braces
    assert!(parse("{}").is_err());
    // Function without parens
    assert!(parse("sin t").is_err());
    // Arity errors
    assert!(parse("atan2(1)").is_err());
    assert!(parse("sqrt()").is_err());
    assert!(parse("sqrt(1,2)").is_err());
    // Unbalanced parens / trailing junk
    assert!(parse("(1+2").is_err());
    assert!(parse("1+").is_err());
    assert!(parse("1 + )").is_err());
    // Error position is reported
    let e = parse("1 + * 2").unwrap_err();
    assert!(e.pos > 0);
}

#[test]
fn test_ast_shape_for_rendering() {
    // Group nodes preserve explicit parens (fraction rule input)
    let ast = parse("({a})/({b}+1)").unwrap();
    match ast {
        Expr::Bin(BinOp::Div, l, r) => {
            assert!(matches!(*l, Expr::Group(_)));
            assert!(matches!(*r, Expr::Group(_)));
        }
        other => panic!("expected division, got {other:?}"),
    }
    // Function node kinds
    let ast = parse("atan2({y}, {x})").unwrap();
    assert!(matches!(ast, Expr::Call(Func::Atan2, _)));
}

#[test]
fn test_whitespace_and_edge_numbers() {
    assert_eq!(eval_str("  .5 + .5 ", 0.0, &[]), 1.0);
    assert_eq!(eval_str("1e3", 0.0, &[]), 1000.0);
    assert_eq!(eval_str("1.5e-1", 0.0, &[]), 0.15);
    // `2e` must not be eaten as a malformed exponent; parses as 2*e
    assert!((eval_str("2e", 0.0, &[]) - 2.0 * std::f64::consts::E).abs() < 1e-9);
}
