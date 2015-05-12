#![feature(collections)]

#[macro_use]
extern crate nom;

use nom::{IResult, digit};

use std::str;
use std::string::String;

#[derive(Debug)]
enum Bencode {
    Text(String),
    Int(i64),
    List(Vec<Bencode>),
    Dict(Vec<(Bencode, Bencode)>),
}

fn text(i:&[u8]) -> IResult<&[u8], Bencode> {
    match digit(i) {
        IResult::Error(err) => IResult::Error(err),
        IResult::Incomplete(u) => IResult::Incomplete(u),
        IResult::Done(rest, digbytes) => {
            let n = str::from_utf8(digbytes).unwrap()
                .parse::<usize>().unwrap();
            let text = String::from_str(
                str::from_utf8(&rest[1..n+1]).unwrap());
            IResult::Done(&rest[n+1..], Bencode::Text(text))
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
