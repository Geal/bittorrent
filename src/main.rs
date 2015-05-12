#![feature(collections)]

#[macro_use]
extern crate nom;
use nom::{IResult, Needed, digit};

extern crate sha1;
use sha1::Sha1;

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::str;

#[derive(Debug)]
enum Bencode {
    Str(Vec<u8>),
    Int(i64),
    List(Vec<Bencode>),
    Dict(Vec<(Bencode, Bencode)>),
}

named!(i64_utf8<&[u8], i64>, chain!(
 bytes: digit ,
        ||{ str::from_utf8(bytes).unwrap().parse::<i64>().unwrap() }
));

named!(u64_utf8<&[u8], u64>, chain!(
 bytes: digit ,
        ||{ str::from_utf8(bytes).unwrap().parse::<u64>().unwrap() }
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
                let mut v = vec!();
                v.push_all(&rest[1..n+1]);
                IResult::Done(&rest[n+1..], Bencode::Str(v))
            }
        }
    }
}

named!(int<&[u8], Bencode>, chain!(
        tag!("i") ~
     n: i64_utf8  ~
        tag!("e") ,
        ||{ Bencode::Int(n) }
));

named!(list<&[u8], Bencode>, chain!(
        tag!("l")       ~
    bs: many0!(bencode) ~
        tag!("e")       ,
        ||{ Bencode::List(bs) }
));

named!(dict<&[u8], Bencode>, chain!(
        tag!("d")              ~
    ps: many0!(pair!(bencode, bencode)) ~
        tag!("e")              ,
        ||{ Bencode::Dict(ps) }
));

named!(bencode<&[u8], Bencode>, alt!(text | int | list | dict));

fn main() {
    let mut f = File::open(&Path::new("/home/jagus/code/bittorrent/archlinux-2015.05.01-dual.iso.torrent")).unwrap();
    println!("{:?}", f);
    let mut v = Vec::new();
    f.read_to_end(&mut v).ok();
    println!("{:?}", bencode(&v[..]));
}
