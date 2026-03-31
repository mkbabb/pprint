fn main() {
    eprintln!("sizeof(Doc) = {}", std::mem::size_of::<pprint::Doc>());
    eprintln!("align(Doc) = {}", std::mem::align_of::<pprint::Doc>());
    eprintln!("sizeof(Box<Doc>) = {}", std::mem::size_of::<Box<pprint::Doc>>());
    eprintln!("sizeof(Vec<Doc>) = {}", std::mem::size_of::<Vec<pprint::Doc>>());
    eprintln!("sizeof(Cow<str>) = {}", std::mem::size_of::<std::borrow::Cow<str>>());
}
