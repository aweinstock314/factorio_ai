use nom::{
    branch::alt,
    bytes::complete::{is_a, is_not, tag},
    character::complete::{alpha1, alphanumeric1, multispace0},
    combinator::{map, opt, recognize},
    error::{context, ContextError, ParseError},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, tuple},
    IResult,
};
use serde::{Serialize, Deserialize};

use std::{collections::HashMap, str::FromStr};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LuaObject {
    Map(HashMap<String, LuaObject>),
    Array(Vec<LuaObject>),
    Bool(bool),
    Str(String),
    Int(u64),
    Float(f64),
}

pub fn whitespace<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, (), E> {
    let (input, _) = multispace0(input)?;
    let (input, _) = opt(tuple((tag("--"), is_not("\r\n"))))(input)?;
    let (input, _) = multispace0(input)?;
    Ok((input, ()))
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
    alt((
        map(tag("true"), |_| LuaObject::Bool(true)),
        map(tag("false"), |_| LuaObject::Bool(false)),
    ))(input)
}

pub fn parse_object<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaObject, E> {
    let (input, ret) = alt((
        context("num", parse_num),
        context("bool", parse_bool),
        context("str", parse_str),
        context("map", parse_map),
        context("array", parse_array),
    ))(input)?;
    //println!("obj: {:?}", ret);
    Ok((input, ret))
}

pub fn parse_str<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, LuaObject, E> {
    map(
        delimited(tag("\""), recognize(many0(is_not("\"\\"))), tag("\"")),
        |s: &'a str| LuaObject::Str(s.to_string()),
    )(input)
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
        LuaObject::Int(u64::from_str(s).unwrap())
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
    map(recognize(many1(is_a("0123456789."))), |s: &'a str| {
        if let Ok(int) = u64::from_str(s) {
            LuaObject::Int(int)
        } else {
            LuaObject::Float(f64::from_str(s).unwrap())
        }
    })(input)
}

pub fn parse_identifier<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, &'a str, E> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ))(input)
}
pub fn parse_field<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, (String, LuaObject), E> {
    let (input, ident) = parse_identifier(input)?;
    let (input, _) = whitespace(input)?;
    let (input, _) = tag("=")(input)?;
    let (input, _) = whitespace(input)?;
    let (input, rhs) = parse_object(input)?;
    Ok((input, (ident.to_string(), rhs)))
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
    let (input, objects) = separated_list1(commaspace, parse_object)(input)?;
    let (input, _) = whitespace(input)?;
    let (input, _) = opt(commaspace)(input)?;
    let (input, _) = tag("}")(input)?;
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
    Ok((input, object))
}
