#![feature(collections, collections_drain)]

#[macro_use]
extern crate nom;
use nom::{IResult, Needed, digit};

extern crate sha1;
use sha1::Sha1;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path; use std::str;

#[derive(Clone, Debug)]
enum Bencode {
    Str(Vec<u8>),
    Int(i64),
    List(Vec<Bencode>),
    Dict(HashMap<Vec<u8>, Bencode>),
}

named!(i64_utf8<&[u8], i64>, chain!(
 bytes: digit ,
        ||{ str::from_utf8(bytes).unwrap().parse::<i64>().unwrap() }
));

named!(u64_utf8<&[u8], u64>, chain!(
 bytes: digit ,
        ||{ str::from_utf8(bytes).unwrap().parse::<u64>().unwrap() }
));

fn text(i:&[u8]) -> IResult<&[u8], Vec<u8>> {
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
                IResult::Done(&rest[n+1..], v)
            }
        }
    }
}

named!(int<&[u8], i64>, chain!(
        tag!("i") ~
     n: i64_utf8  ~
        tag!("e") ,
        ||{ n }
));

named!(list<&[u8], (Vec<Bencode>)>, chain!(
        tag!("l")       ~
    bs: many0!(bencode) ~
        tag!("e")       ,
        ||{ bs }
));

fn dict(i:&[u8]) -> IResult<&[u8], HashMap<Vec<u8>, Bencode>> {
named!(helper<&[u8], (Vec<(Vec<u8>, Bencode)>)>, chain!(
        tag!("d")~
        ps: many0!(pair!(text, bencode))~
        tag!("e"),
        ||{ ps }
));
    
    match helper(i) {
        IResult::Error(err) => IResult::Error(err),
        IResult::Incomplete(u) => IResult::Incomplete(u),
        IResult::Done(rest, mut ps) => {
            let mut h = HashMap::new();
            for (k, v) in ps.drain(..) {
                h.insert(k, v);
            }
            IResult::Done(rest, h)
        }
    }
}

named!(bencode<&[u8], Bencode>, alt!(
        chain!(t:text, ||{Bencode::Str(t)})  |
        chain!(i: int, ||{Bencode::Int(i)})  |
        chain!(l:list, ||{Bencode::List(l)}) |
        chain!(d:dict, ||{Bencode::Dict(d)})
));

#[derive(Clone, Debug)]
struct Metainfo {
    announce: String,
    name: String,
    piecelen: usize,
    pieces: Vec<[u32; 5]>,
    files: Vec<(usize, String)>,
}

fn main() {
    let mut f = File::open(&Path::new("/home/jagus/code/bittorrent/archlinux-2015.05.01-dual.iso.torrent")).unwrap();
    println!("{:?}", f);
    let mut v = Vec::new();
    f.read_to_end(&mut v).ok();
    println!("{:?}", bencode(&v[..]));
}
