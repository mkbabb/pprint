//! The pprint semantic oracle (sk-v27 LW1.PP-3).
//!
//! A scalar *tree* printer, transcribed literally from
//!
//! - [W]  Philip Wadler, "A prettier printer", in *The Fun of Programming*,
//!   2003. §3 (the algebra: `group`, `flatten`, `<|>`), §4 (the
//!   efficient implementation: `pretty`, `best`, `be`, `better`,
//!   `fits`, `layout`) and §5's utility combinators (`<+>`, `</>`,
//!   `folddoc`, `spread`, `stack`, `bracket`, `<+/>`, `fillwords`, `fill`).
//! - [L]  Daan Leijen, `Text.PrettyPrint.Leijen` (wl-pprint, 2001), for
//!   `column`, `nesting`, `align`, `hang`, `indent`, `softline`,
//!   `softbreak`, `linebreak`, `fillSep`, `width` and the padding `fill`.
//! - [P]  `prettyprinter` (the maintained descendant of [L]), for the two
//!   constructs neither paper names but pprint needs: `flatAlt` and the
//!   failing flatten of a hard line (`flatten hardline = Fail`,
//!   `fits _ SFail = False`).
//!
//! Every function below names the equation it transcribes, in the source's
//! own notation, in its doc comment. Nothing here shares code with
//! `rust/src/render.rs`: the oracle reads `FmtOp` only through its public
//! enum (in `lower.rs`), and renders through its own `be`/`fits`/`layout`.
//!
//! Width is Wadler's `length s` read as display columns (sk-v27 LW3.PP-1b):
//! [`length`] measures a `TEXT` with `unicode-width` directly (UAX #11, East
//! Asian Wide = 2, combining marks = 0), sharing no code with pprint's
//! `width.rs`. Through LW2 it was the count of `Char`s, which agrees on the
//! ASCII corpora of LW1 and LW2 only.
//!
//! The implementation is deliberately the naive, lazy one: `be` is an
//! iterator over a persistent work list, and `better` clones that iterator to
//! ask `fits` about the first line of one alternative. That is exactly the
//! Haskell evaluation order of [W] §4 (`fits` forces `be` only up to the first
//! `Line`), at the price of exponential worst cases that the oracle's corpora
//! (depth <= 12) never reach.

#![allow(dead_code)]

pub mod lower;

use std::rc::Rc;

/// A document. [W] §3 `data DOC = NIL | DOC :<> DOC | NEST Int DOC | TEXT
/// String | LINE | DOC :<|> DOC`, extended by [L] (`Column`, `Nesting`,
/// `Line Bool`) and [P] (`FlatAlt`, `Fail`).
pub enum Doc {
    /// [W] §3 `NIL`.
    Nil,
    /// [W] §3 `TEXT s` (no newline inside `s`; invariant of the paper).
    Text(Rc<str>),
    /// [W] §3 `LINE`: newline, and `flatten LINE = TEXT " "`.
    Line,
    /// [L] `linebreak = Line True`: newline, and `flatten (Line True) = Empty`.
    LineBreak,
    /// [P] `hardline = Line`: newline, and `flatten Line = Fail`.
    HardLine,
    /// [W] §3 `x :<> y`.
    Cat(D, D),
    /// [W] §3 `NEST i x`.
    Nest(isize, D),
    /// [W] §3 `x :<|> y`. Invariant [W] §3: every first line of `x` is at
    /// least as long as every first line of `y` (it holds for every union
    /// `group` builds, which is the only constructor of unions here).
    Union(D, D),
    /// [P] `FlatAlt x y`: render `x`, but `flatten (FlatAlt x y) = flatten y`.
    FlatAlt(D, D),
    /// [L] `Column (Int -> Doc)`: the document as a function of the column.
    Column(Rc<dyn Fn(isize) -> D>),
    /// [L] `Nesting (Int -> Doc)`: the document as a function of the nesting.
    Nesting(Rc<dyn Fn(isize) -> D>),
    /// [P] `Fail`: the flattening of a hard line. Never laid out.
    Fail,
}

pub type D = Rc<Doc>;

/// [W] §4 `length s`, in display columns (LW3.PP-1b).
pub fn length(s: &str) -> isize {
    unicode_width::UnicodeWidthStr::width(s) as isize
}

// ── [W] §3: the combinators ─────────────────────────────────────────────────

/// [W] §3 `nil = NIL`.
pub fn nil() -> D {
    Rc::new(Doc::Nil)
}

/// [W] §3 `text s = TEXT s`.
pub fn text(s: &str) -> D {
    debug_assert!(!s.contains('\n'), "[W] §3: TEXT carries no newline");
    Rc::new(Doc::Text(Rc::from(s)))
}

/// [W] §3 `line = LINE`.
pub fn line() -> D {
    Rc::new(Doc::Line)
}

/// [L] `linebreak = Line True` (flattens to `empty`).
pub fn linebreak() -> D {
    Rc::new(Doc::LineBreak)
}

/// [P] `hardline` (flattens to `Fail`).
pub fn hardline() -> D {
    Rc::new(Doc::HardLine)
}

/// [W] §3 `x <> y = x :<> y`.
pub fn cat(x: D, y: D) -> D {
    Rc::new(Doc::Cat(x, y))
}

/// [W] §3 `nest i x = NEST i x`.
pub fn nest(i: isize, x: D) -> D {
    Rc::new(Doc::Nest(i, x))
}

/// [P] `flatAlt x y = FlatAlt x y`.
pub fn flat_alt(x: D, y: D) -> D {
    Rc::new(Doc::FlatAlt(x, y))
}

/// [L] `column f = Column f`.
pub fn column(f: impl Fn(isize) -> D + 'static) -> D {
    Rc::new(Doc::Column(Rc::new(f)))
}

/// [L] `nesting f = Nesting f`.
pub fn nesting(f: impl Fn(isize) -> D + 'static) -> D {
    Rc::new(Doc::Nesting(Rc::new(f)))
}

/// [W] §3 `group x = flatten x :<|> x`.
pub fn group(x: D) -> D {
    Rc::new(Doc::Union(flatten(&x), x))
}

/// [W] §3, the six `flatten` equations, plus [L] and [P] for the extensions:
///
/// ```text
/// flatten NIL        = NIL
/// flatten (x :<> y)  = flatten x :<> flatten y
/// flatten (NEST i x) = NEST i (flatten x)
/// flatten (TEXT s)   = TEXT s
/// flatten LINE       = TEXT " "
/// flatten (x :<|> y) = flatten x
/// flatten (Line True)   = Empty                      -- [L]
/// flatten (Column f)    = Column (flatten . f)       -- [L]
/// flatten (Nesting f)   = Nesting (flatten . f)      -- [L]
/// flatten (FlatAlt _ y) = flatten y                  -- [P]
/// flatten Line{-hard-}  = Fail                       -- [P]
/// ```
pub fn flatten(x: &D) -> D {
    match &**x {
        Doc::Nil => nil(),
        Doc::Cat(a, b) => cat(flatten(a), flatten(b)),
        Doc::Nest(i, a) => nest(*i, flatten(a)),
        Doc::Text(_) => x.clone(),
        Doc::Line => text(" "),
        Doc::Union(a, _) => flatten(a),
        Doc::LineBreak => nil(),
        Doc::Column(f) => {
            let f = f.clone();
            column(move |k| flatten(&f(k)))
        }
        Doc::Nesting(f) => {
            let f = f.clone();
            nesting(move |i| flatten(&f(i)))
        }
        Doc::FlatAlt(_, b) => flatten(b),
        Doc::HardLine => Rc::new(Doc::Fail),
        Doc::Fail => x.clone(),
    }
}

// ── [W] / [L] utility combinators ───────────────────────────────────────────

/// [W] `x <+> y = x <> text " " <> y`.
pub fn space_cat(x: D, y: D) -> D {
    cat(x, cat(text(" "), y))
}

/// [W] `x </> y = x <> line <> y` (Wadler's, not Leijen's, `</>`).
pub fn w_line_cat(x: D, y: D) -> D {
    cat(x, cat(line(), y))
}

/// [W] `folddoc f [] = nil; folddoc f [x] = x; folddoc f (x:xs) = f x (folddoc f xs)`.
pub fn folddoc(f: impl Fn(D, D) -> D + Copy, xs: &[D]) -> D {
    match xs {
        [] => nil(),
        [x] => x.clone(),
        [x, rest @ ..] => f(x.clone(), folddoc(f, rest)),
    }
}

/// [W] `spread = folddoc (<+>)`.
pub fn spread(xs: &[D]) -> D {
    folddoc(space_cat, xs)
}

/// [W] `stack = folddoc (</>)` (Wadler's `</>`, a plain `line`).
pub fn stack(xs: &[D]) -> D {
    folddoc(w_line_cat, xs)
}

/// [W] `bracket l x r = group (text l <> nest 2 (line <> x) <> line <> text r)`.
pub fn bracket(l: &str, x: D, r: &str) -> D {
    group(cat(
        text(l),
        cat(nest(2, cat(line(), x)), cat(line(), text(r))),
    ))
}

/// [W] `x <+/> y = x <> (text " " :<|> line) <> y`. The union is built
/// directly; it keeps [W] §3's invariant (the first line of `text " "` is
/// at least as long as that of `line`).
pub fn space_or_line(x: D, y: D) -> D {
    cat(x, cat(Rc::new(Doc::Union(text(" "), line())), y))
}

/// [W] `fillwords = folddoc (<+/>) . map text . words`.
pub fn fillwords(s: &str) -> D {
    let ws: Vec<D> = s.split_whitespace().map(text).collect();
    folddoc(space_or_line, &ws)
}

/// [W] the list `fill`:
///
/// ```text
/// fill []       = nil
/// fill [x]      = x
/// fill (x:y:zs) = (flatten x <+> fill (flatten y : zs))
///                 :<|>
///                 (x </> fill (y : zs))
/// ```
pub fn w_fill(xs: &[D]) -> D {
    match xs {
        [] => nil(),
        [x] => x.clone(),
        [x, y, zs @ ..] => {
            let mut flat_tail = vec![flatten(y)];
            flat_tail.extend(zs.iter().cloned());
            let left = space_cat(flatten(x), w_fill(&flat_tail));
            let right = w_line_cat(x.clone(), w_fill(&xs[1..]));
            Rc::new(Doc::Union(left, right))
        }
    }
}

/// [L] `softline = group line`.
pub fn softline() -> D {
    group(line())
}

/// A line feed inside a leaf: a hard line at column 0, whatever the nesting,
/// [L] `nesting (\i -> nest (-i) hardline)`. Its flattening fails ([P]
/// `flatten hardline = Fail`), so a group holding it breaks.
pub fn leaf_newline() -> D {
    nesting(|i| nest(-i, hardline()))
}

/// A leaf that may hold line feeds: its lines as `text`, joined by
/// [`leaf_newline`] (LW3.PP-1b).
pub fn leaf_text(s: &str) -> D {
    let mut parts = s.split('\n');
    let mut d = text(parts.next().unwrap_or(""));
    for line in parts {
        d = cat(d, cat(leaf_newline(), text(line)));
    }
    d
}

/// [L] `softbreak = group linebreak`.
pub fn softbreak() -> D {
    group(linebreak())
}

/// [L] `x </> y = x <> softline <> y`.
pub fn l_soft_cat(x: D, y: D) -> D {
    cat(x, cat(softline(), y))
}

/// [L] `x <$> y = x <> line <> y`.
pub fn l_line_cat(x: D, y: D) -> D {
    cat(x, cat(line(), y))
}

/// [L] `fillSep = fold (</>)`.
pub fn fill_sep(xs: &[D]) -> D {
    folddoc(l_soft_cat, xs)
}

/// [L] `vsep = fold (<$>)`.
pub fn vsep(xs: &[D]) -> D {
    folddoc(l_line_cat, xs)
}

/// [L] `sep = group . vsep`.
pub fn sep(xs: &[D]) -> D {
    group(vsep(xs))
}

/// [L] `align d = column (\k -> nesting (\i -> nest (k - i) d))`.
pub fn align(d: D) -> D {
    column(move |k| {
        let d = d.clone();
        nesting(move |i| nest(k - i, d.clone()))
    })
}

/// [L] `hang i d = align (nest i d)`.
pub fn hang(i: isize, d: D) -> D {
    align(nest(i, d))
}

/// [L] `indent i d = hang i (text (spaces i) <> d)`.
pub fn indent(i: isize, d: D) -> D {
    hang(i, cat(text(&" ".repeat(i.max(0) as usize)), d))
}

/// [L] `width d f = column (\k1 -> d <> column (\k2 -> f (k2 - k1)))`.
pub fn width(d: D, f: impl Fn(isize) -> D + 'static) -> D {
    let f: Rc<dyn Fn(isize) -> D> = Rc::new(f);
    column(move |k1| {
        let f = f.clone();
        cat(d.clone(), column(move |k2| f(k2 - k1)))
    })
}

/// [L] the padding `fill`:
/// `fill f d = width d (\w -> if w >= f then empty else text (spaces (f - w)))`.
pub fn l_fill(f: isize, d: D) -> D {
    width(d, move |w| {
        if w >= f {
            nil()
        } else {
            text(&" ".repeat((f - w) as usize))
        }
    })
}

// ── [W] §4: the efficient implementation ────────────────────────────────────

/// [W] §4 `data Doc = Nil | String `Text` Doc | Int `Line` Doc`, as a stream
/// item, plus [P] `SFail`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SItem {
    Text(Rc<str>),
    Line(isize),
    Fail,
}

/// [W] §4 the work list `[(Int, DOC)]`, as a persistent cons list so that
/// `better` can hold both alternatives without copying.
#[derive(Clone)]
enum List {
    Nil,
    Cons(isize, D, Rc<List>),
}

/// [W] §4 `be w k z`, as a lazy stream: each `next` yields the head of the
/// `Doc` the equation would build, and leaves the tail unevaluated.
#[derive(Clone)]
pub struct Be {
    w: isize,
    k: isize,
    z: Rc<List>,
}

impl Be {
    /// [W] §4 `best w k x = be w k [(0, x)]`.
    pub fn best(w: isize, k: isize, x: D) -> Self {
        Be {
            w,
            k,
            z: Rc::new(List::Cons(0, x, Rc::new(List::Nil))),
        }
    }

    fn push(&mut self, i: isize, x: D) {
        self.z = Rc::new(List::Cons(i, x, self.z.clone()));
    }
}

impl Iterator for Be {
    type Item = SItem;

    /// [W] §4 the seven `be` equations, plus [L] for `Column`/`Nesting`/
    /// `Line Bool` and [P] for `FlatAlt`/`Fail`:
    ///
    /// ```text
    /// be w k []                 = Nil
    /// be w k ((i,NIL):z)        = be w k z
    /// be w k ((i,x :<> y):z)    = be w k ((i,x):(i,y):z)
    /// be w k ((i,NEST j x):z)   = be w k ((i+j,x):z)
    /// be w k ((i,TEXT s):z)     = s `Text` be w (k+length s) z
    /// be w k ((i,LINE):z)       = i `Line` be w i z
    /// be w k ((i,x :<|> y):z)   = better w k (be w k ((i,x):z))
    ///                                        (be w k ((i,y):z))
    /// be w k ((i,Column f):z)   = be w k ((i,f k):z)          -- [L]
    /// be w k ((i,Nesting f):z)  = be w k ((i,f i):z)          -- [L]
    /// be w k ((i,FlatAlt x _):z)= be w k ((i,x):z)            -- [P]
    /// be w k ((i,Fail):z)       = SFail                       -- [P]
    /// ```
    fn next(&mut self) -> Option<SItem> {
        loop {
            let (i, x, z) = match &*self.z {
                List::Nil => return None,
                List::Cons(i, x, z) => (*i, x.clone(), z.clone()),
            };
            self.z = z;
            match &*x {
                Doc::Nil => {}
                Doc::Cat(a, b) => {
                    self.push(i, b.clone());
                    self.push(i, a.clone());
                }
                Doc::Nest(j, a) => self.push(i + j, a.clone()),
                Doc::Text(s) => {
                    self.k += length(s);
                    return Some(SItem::Text(s.clone()));
                }
                Doc::Line | Doc::LineBreak | Doc::HardLine => {
                    self.k = i;
                    return Some(SItem::Line(i));
                }
                Doc::Union(a, b) => {
                    let mut left = self.clone();
                    left.push(i, a.clone());
                    let mut right = self.clone();
                    right.push(i, b.clone());
                    *self = better(self.w, self.k, left, right);
                }
                Doc::Column(f) => {
                    let d = f(self.k);
                    self.push(i, d);
                }
                Doc::Nesting(f) => self.push(i, f(i)),
                Doc::FlatAlt(a, _) => self.push(i, a.clone()),
                Doc::Fail => {
                    self.z = Rc::new(List::Nil);
                    return Some(SItem::Fail);
                }
            }
        }
    }
}

/// [W] §4 `better w k x y = if fits (w-k) x then x else y`.
pub fn better(w: isize, k: isize, x: Be, y: Be) -> Be {
    if fits(w - k, x.clone()) { x } else { y }
}

/// [W] §4 the four `fits` equations, plus [P] `fits _ SFail = False`:
///
/// ```text
/// fits w x | w < 0      = False
/// fits w Nil            = True
/// fits w (s `Text` x)   = fits (w - length s) x
/// fits w (i `Line` x)   = True
/// ```
pub fn fits(mut w: isize, mut x: Be) -> bool {
    loop {
        if w < 0 {
            return false;
        }
        match x.next() {
            None => return true,
            Some(SItem::Text(s)) => w -= length(&s),
            Some(SItem::Line(_)) => return true,
            Some(SItem::Fail) => return false,
        }
    }
}

/// [W] §4 the three `layout` equations:
///
/// ```text
/// layout Nil          = ""
/// layout (s `Text` x) = s ++ layout x
/// layout (i `Line` x) = '\n' : copy i ' ' ++ layout x
/// ```
///
/// [P] `layout SFail` is an error (only reachable when the whole document is
/// a forced flattening of a hard line).
pub fn layout(x: Be) -> Result<String, &'static str> {
    let mut out = String::new();
    for item in x {
        match item {
            SItem::Text(s) => out.push_str(&s),
            SItem::Line(i) => {
                out.push('\n');
                out.extend(std::iter::repeat_n(' ', i.max(0) as usize));
            }
            SItem::Fail => return Err("[P] layout: SFail reached"),
        }
    }
    Ok(out)
}

/// [W] §4 `pretty w x = layout (best w 0 x)`.
pub fn pretty(w: isize, x: &D) -> Result<String, &'static str> {
    layout(Be::best(w, 0, x.clone()))
}

/// pprint's output convention on top of [W] `pretty`: trailing spaces and
/// tabs are removed before every newline and at the end (pprint
/// `render.rs` contract, "Strip trailing whitespace"). This is a
/// post-processing of the string, not a change to the layout algorithm.
pub fn pretty_pprint(w: isize, x: &D) -> Result<String, &'static str> {
    let raw = pretty(w, x)?;
    let mut out = String::with_capacity(raw.len());
    for (n, ln) in raw.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        out.push_str(ln.trim_end_matches([' ', '\t']));
    }
    Ok(out)
}
