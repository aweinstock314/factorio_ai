use nom::{
    branch::alt,
    bytes::complete::{is_a, is_not, tag, take_until},
    character::complete::{alpha1, alphanumeric1, multispace0},
    combinator::{map, opt, recognize},
    error::{context, ContextError, ParseError},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, tuple},
    IResult,
};
use serde::{Deserialize, Serialize};

use std::convert::{TryFrom, TryInto};
use std::fmt::Display;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    str::FromStr,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LuaObject {
    Map(HashMap<String, LuaObject>),
    Array(Vec<LuaObject>),
    Bool(bool),
    Str(String),
    Int(i64),
    Float(f64),
    Expr(Box<LuaExpr>),
}

impl LuaObject {
    pub fn simplify(self) -> Self {
        use LuaObject::*;
        match self {
            Map(map) => Map(map.into_iter().map(|(k, v)| (k, v.simplify())).collect()),
            Array(array) => Array(array.into_iter().map(|x| x.simplify()).collect()),
            Expr(x) => match *x {
                LuaExpr::Literal(x) => x.simplify(),
                _ => Expr(x),
            },
            _ => self,
        }
    }
}

impl TryFrom<LuaObject> for LuaExpr {
    type Error = String;

    fn try_from(value: LuaObject) -> Result<Self, Self::Error> {
        match value {
            LuaObject::Expr(x) => Ok(*x),
            _ => Err("Not an Expr".into()),
        }
    }
}

impl<T: TryFrom<LuaObject>> TryFrom<LuaObject> for HashMap<String, T>
where
    <T as TryFrom<LuaObject>>::Error: Display,
{
    type Error = String;

    fn try_from(value: LuaObject) -> Result<Self, Self::Error> {
        match value {
            LuaObject::Map(m) => m
                .into_iter()
                .map(|(i, l)| {
                    T::try_from(l)
                        .map_err(|e| format!("Could not convert child '{}': {}", &i, &e))
                        .map(|l| (i, l))
                })
                .collect(),
            _ => Err("Not a Map".into()),
        }
    }
}

impl<T: Hash + Eq + TryFrom<LuaObject>> TryFrom<LuaObject> for HashSet<T> {
    type Error = String;

    fn try_from(value: LuaObject) -> Result<Self, Self::Error> {
        match value {
            LuaObject::Array(m) => m
                .into_iter()
                .enumerate()
                .map(|(i, l)| T::try_from(l).map_err(|_| format!("Could not convert '{}'", &i)))
                .collect(),
            _ => Err("Not an Array".into()),
        }
    }
}

impl<T: TryFrom<LuaObject>> TryFrom<LuaObject> for Vec<T>
where
    <T as TryFrom<LuaObject>>::Error: Display,
{
    type Error = String;

    fn try_from(value: LuaObject) -> Result<Vec<T>, Self::Error> {
        match value {
            LuaObject::Array(a) => a
                .into_iter()
                .enumerate()
                .map(|(idx, i)| {
                    i.try_into()
                        .map_err(|e| format!("Could not convert child {}: {}", idx, &e))
                })
                .collect(),
            _ => Err("Not an Array".into()),
        }
    }
}

impl<T1: TryFrom<LuaObject>, T2: TryFrom<LuaObject>> TryFrom<LuaObject> for (T1, T2) {
    type Error = String;

    fn try_from(value: LuaObject) -> Result<(T1, T2), Self::Error> {
        match value {
            LuaObject::Array(mut ar) => {
                if ar.len() == 2 {
                    let a = ar.remove(0);
                    let b = ar.remove(0);
                    Ok((
                        T1::try_from(a).map_err(|_| String::from("Couldn't convert first arg"))?,
                        T2::try_from(b).map_err(|_| String::from("Couldn't convert second arg"))?,
                    ))
                } else {
                    return Err("Invalid sized array".into());
                }
            }
            _ => Err("Not an Array".into()),
        }
    }
}

impl TryFrom<LuaObject> for bool {
    type Error = String;

    fn try_from(value: LuaObject) -> Result<bool, Self::Error> {
        match value {
            LuaObject::Bool(b) => Ok(b),
            _ => Err("Not a Bool".into()),
        }
    }
}

impl TryFrom<LuaObject> for String {
    type Error = String;

    fn try_from(value: LuaObject) -> Result<String, Self::Error> {
        match value {
            LuaObject::Str(s) => Ok(s),
            _ => Err("Not a Str".into()),
        }
    }
}

impl TryFrom<LuaObject> for i64 {
    type Error = String;

    fn try_from(value: LuaObject) -> Result<i64, Self::Error> {
        match value {
            LuaObject::Int(i) => Ok(i),
            _ => Err("Not a Int".into()),
        }
    }
}

impl TryFrom<LuaObject> for f64 {
    type Error = String;

    fn try_from(value: LuaObject) -> Result<f64, Self::Error> {
        match value {
            LuaObject::Float(f) => Ok(f),
            LuaObject::Int(i) => Ok(i as f64),
            _ => Err("Not a Float".into()),
        }
    }
}

pub fn whitespace<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    mut input: &'a str,
) -> IResult<&'a str, (), E> {
    loop {
        let (input0, _) = multispace0(input)?;
        let (input1, _) = opt(tuple((tag("--"), is_not("\r\n"))))(input0)?;
        if input1.len() == input.len() {
            break Ok((input, ()));
        }
        input = input1;
    }
}

pub fn commaspace<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, (), E> {
    let (input, _) = tag(",")(input)?;
    let (input, _) = whitespace(input)?;
    Ok((input, ()))
}

pub fn parse_bool<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaObject, E> {
    let (input, ret) = alt((
        map(tag("true"), |_| LuaObject::Bool(true)),
        map(tag("false"), |_| LuaObject::Bool(false)),
    ))(input)?;
    let (input, _) = whitespace(input)?;
    Ok((input, ret))
}

pub fn parse_namespaced<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, Vec<String>, E> {
    let (input, names) = map(separated_list1(tag("."), parse_identifier), |t| {
        t.into_iter().map(String::from).collect()
    })(input)?;
    let (input, _) = whitespace(input)?;
    Ok((input, names))
}

pub fn parse_subscript<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, (LValue, LuaExpr), E> {
    // a[0][0][0][0]
    // LValue::Subscript(LValue::Dotted(["a"]), 0)
    if let Some(idx) = input.find("[") {
        let (pre_input, lvalue) = parse_lvalue(&input[..idx])?;
        let (input, _) = tag("[")(&input[idx - pre_input.len()..])?;
        let (input, expr) = parse_expr(input)?;
        let (input, _) = tag("]")(input)?;
        let (input, _) = whitespace(input)?;
        Ok((input, (lvalue, expr)))
    } else {
        Err(nom::Err::Error(E::from_error_kind(
            input,
            nom::error::ErrorKind::Satisfy,
        )))
    }
}

pub fn parse_lvalue<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LValue, E> {
    alt((
        map(parse_subscript, |(x, y)| {
            LValue::Subscript(Box::new(x), Box::new(y))
        }),
        map(parse_namespaced, |x| LValue::Dotted(x)),
    ))(input)
}

pub fn parse_object<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaObject, E> {
    //println!("parse_object: {:?}", &input[0..20]);
    let (input, ret) = alt((
        context("num", parse_num),
        context("bool", parse_bool),
        context("str", parse_str),
        context("array", parse_array),
        context("map", parse_map),
        map(parse_namespaced, |t| {
            LuaObject::Expr(Box::new(LuaExpr::Var(t)))
        }),
    ))(input)?;
    //println!("obj: {:?}", ret);
    Ok((input, ret))
}

pub fn parse_str<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaObject, E> {
    let (input, ret) = map(
        delimited(tag("\""), recognize(many0(is_not("\"\\"))), tag("\"")),
        |s: &'a str| LuaObject::Str(s.to_string()),
    )(input)?;
    let (input, _) = whitespace(input)?;
    Ok((input, ret))
}

#[test]
pub fn parse_tests() {
    assert_eq!(is_not::<_, _, ()>("\"\\")("recipe"), Ok(("", "recipe")));
    assert_eq!(
        parse_str::<()>("\"recipe\""),
        Ok(("", LuaObject::Str("recipe".to_string())))
    );
}

/*pub fn parse_int<'a, E: ParseError<&'a str> + ContextError<&'a str>>(input: &'a str) -> IResult<&'a str, LuaObject, E> {
    map(recognize(many1(is_a("0123456789"))), |s: &'a str| {
        LuaObject::Int(i64::from_str(s).unwrap())
    })(input)
}

pub fn parse_float<'a, E: ParseError<&'a str> + ContextError<&'a str>>(input: &'a str) -> IResult<&'a str, LuaObject, E> {
    map(recognize(many1(is_a("0123456789."))), |s: &'a str| {
        LuaObject::Float(f64::from_str(s).unwrap())
    })(input)
}*/

pub fn parse_num<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaObject, E> {
    let (input, obj) = map(recognize(many1(is_a("-0123456789."))), |s: &'a str| {
        if let Ok(int) = i64::from_str(s) {
            LuaObject::Int(int)
        } else {
            LuaObject::Float(f64::from_str(s).unwrap())
        }
    })(input)?;
    let (input, _) = whitespace(input)?;
    Ok((input, obj))
}

pub fn parse_identifier<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, &'a str, E> {
    let (input, ident) = recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ))(input)?;
    let (input, _) = whitespace(input)?;
    if ["return", "true", "false", "if", "then", "else", "end"].contains(&ident) {
        return Err(nom::Err::Error(E::from_error_kind(
            input,
            nom::error::ErrorKind::Satisfy,
        )));
    }

    Ok((input, ident))
}

pub fn parse_field<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, (String, LuaObject), E> {
    //println!("\tparse_field: {:?}", &input[0..20]);
    let (input, ident) = parse_identifier(input)?;
    let (input, _) = whitespace(input)?;
    let (input, _) = tag("=")(input)?;
    let (input, _) = whitespace(input)?;
    let (input, rhs) = alt((
        map(parse_expr, |e| LuaObject::Expr(Box::new(e))),
        parse_object,
    ))(input)?;
    Ok((input, (ident.to_string(), rhs)))
}

pub fn parse_assign<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaStmt, E> {
    //println!("parse_assign: {:?}", &input[0..20]);
    let (input, lvalue) = parse_lvalue(input)?;
    //println!("\tlvalue: {:?}", lvalue);
    let (input, _) = whitespace(input)?;
    let (input, _) = tag("=")(input)?;
    let (input, _) = whitespace(input)?;
    let (input, rhs) = parse_expr(input)?;
    //println!("\trhs: {:?}", rhs);
    Ok((input, LuaStmt::Assign(lvalue, rhs)))
}

pub fn parse_map<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaObject, E> {
    let (input, _) = tag("{")(input)?;
    let (input, _) = whitespace(input)?;
    let (input, fields) = separated_list0(commaspace, parse_field)(input)?;
    let (input, _) = whitespace(input)?;
    let (input, _) = opt(commaspace)(input)?;
    let (input, _) = tag("}")(input)?;
    let (input, _) = whitespace(input)?;
    Ok((input, LuaObject::Map(fields.into_iter().collect())))
}

pub fn parse_array<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaObject, E> {
    let (input, _) = tag("{")(input)?;
    let (input, _) = whitespace(input)?;
    let (input, objects) = separated_list1(
        commaspace,
        map(parse_expr, |e| LuaObject::Expr(Box::new(e))),
    )(input)?;
    let (input, _) = whitespace(input)?;
    let (input, _) = opt(commaspace)(input)?;
    let (input, _) = tag("}")(input)?;
    let (input, _) = whitespace(input)?;
    Ok((input, LuaObject::Array(objects)))
}

pub fn parse_data_extend<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaObject, E> {
    let (input, _) = tag("data:extend(")(input)?;
    let (input, _) = whitespace(input)?;
    let (input, object) = parse_object(input)?;
    let (input, _) = whitespace(input)?;
    let (input, _) = tag(")")(input)?;
    let (input, _) = whitespace(input)?;
    Ok((input, object))
}

pub fn parse_local<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, (String, LuaExpr), E> {
    map(
        tuple((
            tag("local"),
            whitespace,
            parse_identifier,
            whitespace,
            tag("="),
            whitespace,
            parse_expr,
            whitespace,
        )),
        |t| (t.2.to_string(), t.6),
    )(input)
}

pub fn parse_funcall<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaExpr, E> {
    map(
        tuple((
            parse_namespaced,
            whitespace,
            tag("("),
            whitespace,
            separated_list0(commaspace, parse_expr),
            tag(")"),
            whitespace,
        )),
        |t| LuaExpr::Funcall(t.0, t.4),
    )(input)
}

pub fn parse_return<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaStmt, E> {
    map(
        tuple((tag("return"), whitespace, parse_expr, whitespace)),
        |t| LuaStmt::Return(t.2),
    )(input)
}

pub fn parse_unopkind<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, UnopKind, E> {
    map(tag("#"), |_| UnopKind::Octothorpe)(input)
}

pub fn parse_unop<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaExpr, E> {
    let (input, op) = parse_unopkind(input)?;
    let (input, _) = whitespace(input)?;
    let (input, expr) = parse_expr(input)?;
    Ok((input, LuaExpr::Unop(op, Box::new(expr))))
}

pub fn parse_binopkind<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, BinopKind, E> {
    alt((
        map(tag("+"), |_| BinopKind::Plus),
        map(tag("-"), |_| BinopKind::Minus),
        map(tag("*"), |_| BinopKind::Times),
        map(tag("/"), |_| BinopKind::Divide),
        map(tag(".."), |_| BinopKind::DotDot),
        map(tag("=="), |_| BinopKind::EqEq),
        map(tag("~="), |_| BinopKind::TildeEq),
    ))(input)
}

pub fn parse_binop<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaExpr, E> {
    //println!("parse_binop: {:?}", &input[0..20]);
    let (input, lhs) = parse_object(input)?;
    //println!("\tlhs: {:?}", lhs);
    let (input, op) = parse_binopkind(input)?;
    //println!("\top: {:?}", op);
    let (input, _) = whitespace(input)?;
    let (input, rhs) = parse_expr(input)?;
    //println!("\trhs: {:?}", rhs);
    Ok((
        input,
        LuaExpr::Binop(op, Box::new(LuaExpr::Literal(lhs)), Box::new(rhs)),
    ))
}

pub fn parse_expr<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaExpr, E> {
    //println!("parse_expr: {:?}", &input[0..20]);
    alt((
        map(parse_anon_function, |f| LuaExpr::Fundef(Box::new(f))),
        parse_funcall,
        parse_binop,
        parse_unop,
        delimited(tuple((tag("("), whitespace)), parse_expr, tag(")")),
        map(parse_object, LuaExpr::Literal),
    ))(input)
}

pub fn parse_ifthen<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaStmt, E> {
    map(
        tuple((
            tag("if"),
            whitespace,
            parse_expr,
            tag("then"),
            whitespace,
            many1(parse_stmt),
            alt((
                map(
                    tuple((tag("else"), whitespace, many1(parse_stmt), tag("end"))),
                    |t| t.2,
                ),
                map(tag("end"), |_| Vec::new()),
            )),
            whitespace,
        )),
        |t| LuaStmt::IfThen(t.2, t.5, t.6),
    )(input)
}

pub fn parse_stmt<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaStmt, E> {
    //println!("stmt: {:?}", &input[0..20]);
    context(
        "Stmt",
        alt((
            parse_return,
            parse_assign,
            map(parse_local, |(name, expr)| {
                LuaStmt::Assign(LValue::Dotted(vec![name]), expr)
            }),
            parse_ifthen,
            map(parse_expr, |expr| LuaStmt::Expr(expr)),
        )),
    )(input)
}

pub fn parse_named_function<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, (String, LuaFunction), E> {
    parse_function(
        map(tuple((parse_identifier, whitespace)), |t| t.0.to_string()),
        input,
    )
}

pub fn parse_anon_function<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaFunction, E> {
    map(
        |input| parse_function(|input| Ok((input, ())), input),
        |t| t.1,
    )(input)
}

pub fn parse_unhandled_body<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaStmt, E> {
    if false {
        map(take_until("end"), |s: &str| {
            LuaStmt::Return(LuaExpr::Literal(LuaObject::Str(s.to_string())))
        })(input)
    } else {
        Err(nom::Err::Error(E::from_error_kind(
            input,
            nom::error::ErrorKind::Satisfy,
        )))
    }
}

pub fn parse_function<
    'a,
    E: ParseError<&'a str> + ContextError<&'a str>,
    T,
    F: FnMut(&'a str) -> IResult<&'a str, T, E>,
>(
    f: F,
    input: &'a str,
) -> IResult<&'a str, (T, LuaFunction), E> {
    map(
        tuple((
            tag("function"),
            whitespace,
            f,
            tag("("),
            whitespace,
            separated_list0(commaspace, parse_identifier),
            tag(")"),
            whitespace,
            alt((many1(parse_stmt), map(parse_unhandled_body, |x| vec![x]))),
            tag("end"),
            whitespace,
        )),
        |t| {
            (
                t.2,
                LuaFunction {
                    args: t.5.into_iter().map(String::from).collect(),
                    body: t.8,
                },
            )
        },
    )(input)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UnopKind {
    Octothorpe,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinopKind {
    Plus,
    Minus,
    Times,
    Divide,
    DotDot,
    EqEq,
    TildeEq,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LValue {
    Dotted(Vec<String>),
    Subscript(Box<LValue>, Box<LuaExpr>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LuaExpr {
    Var(Vec<String>),
    Literal(LuaObject),
    Funcall(Vec<String>, Vec<LuaExpr>),
    Fundef(Box<LuaFunction>),
    Unop(UnopKind, Box<LuaExpr>),
    Binop(BinopKind, Box<LuaExpr>, Box<LuaExpr>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LuaStmt {
    Return(LuaExpr),
    Assign(LValue, LuaExpr),
    IfThen(LuaExpr, Vec<LuaStmt>, Vec<LuaStmt>),
    Expr(LuaExpr),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LuaFunction {
    pub args: Vec<String>,
    pub body: Vec<LuaStmt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LuaContext {
    pub locals: HashMap<String, LuaExpr>,
    pub functions: HashMap<String, LuaFunction>,
    pub data_extends: Vec<LuaObject>,
}

impl LuaContext {
    pub fn new() -> Self {
        Self {
            locals: HashMap::new(),
            functions: HashMap::new(),
            data_extends: Vec::new(),
        }
    }
    pub fn parse_all<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
        &mut self,
        mut input: &'a str,
    ) -> IResult<&'a str, (), E> {
        loop {
            let (new_input, ()) = self.parse_toplevel(input)?;
            input = new_input;
            if input.is_empty() {
                break Ok((input, ()));
            }
        }
    }
    pub fn parse_toplevel<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
        &mut self,
        input: &'a str,
    ) -> IResult<&'a str, (), E> {
        let Self {
            ref mut locals,
            ref mut functions,
            ref mut data_extends,
        } = self;
        let (input, ()) = alt((
            map(parse_data_extend, |obj| {
                data_extends.push(obj);
            }),
            map(parse_local, |(name, expr)| {
                locals.insert(name, expr);
            }),
            map(parse_named_function, |(name, func)| {
                functions.insert(name, func);
            }),
            map(parse_stmt, |_| ()),
        ))(input)?;
        Ok((input, ()))
    }
}
