#![feature(collections)]

#[macro_use]
extern crate nom;

use nom::{IResult, Needed, digit};

use std::str;
use std::string::String;

#[derive(Debug)]
enum Bencode {
    Text(String),
    Int(i64),
    List(Vec<Bencode>),
    Dict(Vec<(Bencode, Bencode)>),
}

named!(i64_utf8<&[u8], i64>, chain!(
        bytes: digit,
        || {str::from_utf8(bytes).unwrap().parse::<i64>().unwrap()}
));

named!(u64_utf8<&[u8], u64>, chain!(
        bytes: digit,
        || {str::from_utf8(bytes).unwrap().parse::<u64>().unwrap()}
));

fn text(i:&[u8]) -> IResult<&[u8], Bencode> {
    match u64_utf8(i) {
        IResult::Error(err) => IResult::Error(err),
        IResult::Incomplete(u) => IResult::Incomplete(u),
        IResult::Done(rest, n) => {
            let n = n as usize;
            if rest.len() < n+1 {
                IResult::Incomplete(Needed::Size((n+1) as u32))
            } else {
                let text = String::from_str(
                    str::from_utf8(&rest[1..n+1]).unwrap());
                IResult::Done(&rest[n+1..], Bencode::Text(text))
            }
        }
    }
}

fn main() {
    match text(b"10:12345678905:12345") {
        IResult::Done(rest, first) => {
            match text(rest) {
                IResult::Done(rest, second) => {
                    println!("{:?}", first);
                    println!("{:?}", second);
                    println!("{:?}", rest);
                }
                _ => {}
            }
        }
        _ => {}
    }
}
