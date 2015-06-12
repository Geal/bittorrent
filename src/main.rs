#![feature(collections, collections_drain, test)]

extern crate test;

#[macro_use]
extern crate nom;
use nom::{IResult, Needed, be_u32, digit};

//extern crate sha1;
//use sha1::Sha1;

use std::collections::HashMap;
use std::str;


#[derive(Clone, Debug)]
enum Bencode {
    Str(Vec<u8>),
    Int(i64),
    List(Vec<Bencode>),
    Dict(HashMap<Vec<u8>, Bencode>),
}

#[derive(Clone, Debug)]
struct Hash {
    bytes: [u32; 5], // big-endian
}

#[derive(Clone, Debug)]
pub struct Metainfo {
    announce: String,
    name: String,
    piecelength: usize,
    pieces: Vec<Hash>,
    // exactly one of the two following is None
    length: Option<usize>,
    files: Option<Vec<(usize, Vec<String>)>>,
}


named!(i64_utf8<&[u8], i64>, chain!(
        bytes: digit,
        || { str::from_utf8(bytes).unwrap().parse::<i64>().unwrap() }
));

named!(u64_utf8<&[u8], u64>, chain!(
        bytes: digit,
        || { str::from_utf8(bytes).unwrap().parse::<u64>().unwrap() }
));

fn text(i: &[u8]) -> IResult<&[u8], Vec<u8>> {
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
        tag!("i")~
        n: i64_utf8~
        tag!("e"),
        || { n }
));

named!(list<&[u8], (Vec<Bencode>)>, chain!(
        tag!("l")~
        bs: many0!(bencode)~
        tag!("e"),
        || { bs }
));

fn dict(i: &[u8]) -> IResult<&[u8], HashMap<Vec<u8>, Bencode>> {
    named!(helper<&[u8], (Vec<(Vec<u8>, Bencode)>)>, chain!(
            tag!("d")~
            ps: many0!(pair!(text, bencode))~
            tag!("e"),
            || { ps }
    ));
    
    match helper(i) {
        IResult::Error(err) => IResult::Error(err),
        IResult::Incomplete(u) => IResult::Incomplete(u),
        IResult::Done(rest, mut ps) => {
            let mut h = HashMap::new();
            ps.drain(..).map(|(k, v)| h.insert(k, v)).last();
            IResult::Done(rest, h)
        }
    }
}

named!(bencode<&[u8], Bencode>, alt!(
        chain!(t: text, || { Bencode:: Str(t) }) |
        chain!(i:  int, || { Bencode:: Int(i) }) |
        chain!(l: list, || { Bencode::List(l) }) |
        chain!(d: dict, || { Bencode::Dict(d) })
));

named!(hashes<&[u8], (Vec<Hash>)>, many0!(chain!(
        w0: be_u32~
        w1: be_u32~
        w2: be_u32~
        w3: be_u32~
        w4: be_u32,
        || { Hash {
            bytes: [w0, w1, w2, w3, w4],
        } }
)));


pub fn metainfo(i: &[u8]) -> Metainfo {
    // Convert a string literal into a char vec
    fn s2v(s: &'static str) -> Vec<u8> {
        s.as_bytes().iter().map(|&c| c.clone()).collect()
    }

    // Parse the metainfo as a bencoding
    
    let b = match bencode(i) {
        IResult::Done(_, b) => b,
        _ => panic!("Not a bencoding at all"),
    };

    // Must have a top-level dictionary
    
    let mut d = match b {
        Bencode::Dict(d) => d,
        _ => panic!("Not a bencoded dictionary"),
    };

    // This dictionary must have two entries:
    //   'announce' -> {string}
    //   'info'     -> {dictionary}
    
    let announce = String::from_utf8(match d.remove(&s2v("announce")).expect("No announce") {
        Bencode::Str(s) => s,
        _ => panic!("announce not a string"),
    }).unwrap();

    let mut info = match d.remove(&s2v("info")).expect("No info") {
        Bencode::Dict(d) => d,
        _ => panic!("info not a dictionary"),
    };

    // info contains all the good stuff.
    //   'name'         -> string
    //   'piece length' -> int >0
    //   'pieces'       -> string (concatenation of SHA1 hashes of pieces
    //                             => multiple of 20 bytes long)
    //   'length'       -> int >=0
    //   'files'        -> list of dictionaries containing:
    //                       'length' -> int >=0
    //                       'path'   -> list of strings
    //
    // 'files' is present iff 'length' is not.
    // For a single-file torrent, 'length' specifies the file's length.
    // For a multi-file torrent, 'files' specifies the location and length of
    // each file.
    
    let name = String::from_utf8(match info.remove(&s2v("name")).expect("No name") {
        Bencode::Str(s) => s,
        _ => panic!("name not a string"),
    }).unwrap();

    let piecelength = match info.remove(&s2v("piece length")).expect("No piece length") {
        Bencode::Int(i) if i > 0 => i as usize,
        _ => panic!("piece length not a positive integer"),
    };

    let pieces = match hashes(
        &match info.remove(&s2v("pieces")).expect("No pieces") {
            Bencode::Str(ref s) if s.len() % 20 == 0 => s,
            _ => panic!("pieces not a string whose length is a multiple of 20 bytes"),
        }[..]
        ) {
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
            Bencode::List(mut l) => Some(l.drain(..).map(|p| match p {
                Bencode::Dict(mut d) => (
                    match d.remove(&s2v("length")).expect("No file length") {
                        Bencode::Int(i) if i >= 0 => i as usize,
                        _ => panic!("file length not a nonnegative int"),
                    },
                    match d.remove(&s2v("path")).expect("No file path") {
                        Bencode::List(mut l) => l.drain(..).map(|p| match p {
                            Bencode::Str(s) => String::from_utf8(s).unwrap(),
                            _ => panic!("file path contains non-string"),
                        }).collect(),
                        _ => panic!("file path not a list"),
                    },
                ),
                _ => panic!("entry in files not a dictionary"),
            }).collect()),
            _ => panic!("files not a list"),
        },
        None => None,
    };

    assert!(length != None || files != None);

    // There are probably lots of other interesting fields in the metainfo file.
    // They aren't part of the base protocol, so I don't care (yet).

    Metainfo {
        announce: announce,
        name: name,
        piecelength: piecelength,
        pieces: pieces,
        length: length,
        files: files,
    }
}


#[cfg(not(test))]
fn main() {
}


#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;
    use super::*;
    use test::Bencher;

    #[bench]
    fn bench_parse_archlinux_metainfo(b: &mut Bencher) {
        let mut f = File::open(&Path::new("archlinux-2015.05.01-dual.iso.torrent")).unwrap();
        let mut v = Vec::new();
        f.read_to_end(&mut v).ok();
        b.iter(|| metainfo(&v[..]));
    }

    #[bench]
    fn bench_parse_hitchcock_metainfo(b: &mut Bencher) {
        let mut f = File::open(&Path::new("[kat.cr]alfred.hitchcock.masterpiece.collection.hdclub.torrent")).unwrap();
        let mut v = Vec::new();
        f.read_to_end(&mut v).ok();
        b.iter(|| metainfo(&v[..]));
    }
}
