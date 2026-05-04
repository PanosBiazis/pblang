// G:\panag\Documents\projects\AAP\pl-dev\sapphire\crates\sapphire-core\src\lexer.rs
use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
pub enum Token {
    #[token("print")]
    Print,
    #[token("let")]
    Let,
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,
    #[regex(r#""([^"\\]|\\.)*""#)]
    String,
    #[regex(r"[0-9]+")]
    Number,
    #[token("+")]
    Plus,
    #[token("=")]
    Equal,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error, // optional, no #[error] attribute
}
