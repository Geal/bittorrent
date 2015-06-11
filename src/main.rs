#![feature(collections, collections_drain)]

#[macro_use]
extern crate nom;
use nom::{IResult, Needed, be_u32, digit};

//extern crate sha1;
//use sha1::Sha1;

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

fn s2v(s:&'static str) -> Vec<u8> {
    let mut v = Vec::new();
    for &c in s.as_bytes().iter() {
        v.push(c.clone());
    }
    v
}

#[derive(Clone, Debug)]
struct Hash {
    bytes: [u32; 5], // big-endian
}

named!(hashes<&[u8], (Vec<Hash>)>, many0!(chain!(
        w0: be_u32~
        w1: be_u32~
        w2: be_u32~
        w3: be_u32~
        w4: be_u32,
        ||{Hash {
            bytes: [w0, w1, w2, w3, w4],
        }}
)));

#[derive(Clone, Debug)]
struct Metainfo {
    announce: String,
    name: String,
    piecelength: usize,
    pieces: Vec<Hash>,
    // exactly one of the two following is None
    length: Option<usize>,
    files: Option<Vec<(usize, String)>>,
}

fn metainfo(i:&[u8]) -> Metainfo {
    let b = match bencode(i) {
        IResult::Done(_, b) => b,
        _ => panic!("Not a bencoding at all"),
    };
    let mut d = match b {
        Bencode::Dict(d) => d,
        _ => panic!("Not a bencoded dictionary"),
    };
    let announce = String::from_utf8(match d.remove(&s2v("announce")).expect("No announce") {
        Bencode::Str(s) => s,
        _ => panic!("announce not a string"),
    }).unwrap();
    let mut info = match d.remove(&s2v("info")).expect("No info") {
        Bencode::Dict(d) => d,
        _ => panic!("info not a dictionary"),
    };
    let name = String::from_utf8(match info.remove(&s2v("name")).expect("No name") {
        Bencode::Str(s) => s,
        _ => panic!("name not a string"),
    }).unwrap();
    let piecelength = match info.remove(&s2v("piece length")).expect("No piece length") {
        Bencode::Int(i) if i > 0 => i as usize,
        _ => panic!("piece length not a nonnegative int"),
    };
    let piecestr = &match info.remove(&s2v("pieces")).expect("No pieces") {
        Bencode::Str(s) => s,
        _ => panic!("pieces not a string"),
    }[..];
    if piecestr.len() % 20 != 0 {
        panic!("pieces not a multiple of 20 bytes long ({})", piecestr.len());
    }
    let pieces = match hashes(piecestr) {
        IResult::Done(_, hs) => hs,
        _ => panic!("pieces invalid"),
    };
    let length = match info.remove(&s2v("length")) {
        Some(thing) => match thing {
            Bencode::Int(i) if i >= 0 => Some(i as usize),
            _ => panic!("length not a nonnegative int")
        },
        None => None,
    };
    let files = match info.remove(&s2v("files")) {
        Some(thing) => match thing {
            Bencode::List(mut l) => Some(l.iter_mut().map(|p| match p {
                &mut Bencode::Dict(ref mut d) => (
                    match d.remove(&s2v("length")).expect("No file length") {
                        Bencode::Int(i) if i >= 0 => i as usize,
                        _ => panic!("file length not a nonnegative int"),
                    },
                    match d.remove(&s2v("path")).expect("No file path") {
                        Bencode::Str(s) => String::from_utf8(s).unwrap(),
                        _ => panic!("file path not a string"),
                    },
                ),
                _ => panic!("entry in files not a dictionary"),
            }).collect()),
            _ => panic!("files not a list"),
        },
        None => None,
    };
    assert!(length != None || files != None);
    Metainfo {
        announce: announce,
        name: name,
        piecelength: piecelength,
        pieces: pieces,
        length: length,
        files: files,
    }
}

fn main() {
    let mut f = File::open(&Path::new("/home/jagus/code/bittorrent/archlinux-2015.05.01-dual.iso.torrent")).unwrap();
    println!("{:?}", f);
    let mut v = Vec::new();
    f.read_to_end(&mut v).ok();
    println!("{:?}", metainfo(&v[..]));
}
