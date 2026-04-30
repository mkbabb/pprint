fn main() {
    eprintln!(
        "sizeof(FmtOp)     = {}",
        std::mem::size_of::<pprint::FmtOp>()
    );
    eprintln!(
        "align(FmtOp)      = {}",
        std::mem::align_of::<pprint::FmtOp>()
    );
    eprintln!(
        "sizeof(FmtBuilder) = {}",
        std::mem::size_of::<pprint::FmtBuilder>()
    );
    eprintln!(
        "sizeof(Printer)    = {}",
        std::mem::size_of::<pprint::Printer>()
    );
}
