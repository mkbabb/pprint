#[cfg(test)]
mod tests {
    use pprint::print;
    use pprint::{concat, join, pprint, Doc, Group, Indent, Pretty, Printer, Wrap};

    use std::collections::HashMap;
    use std::fmt::Debug;
    use std::fmt::Display;

    #[derive(Pretty, Debug)]
    #[pprint(verbose)]
    pub enum HeyEnum<'a> {
        There(&'a str),

        #[pprint(rename = "MyEnum::A")]
        A,

        B(regex::Regex),
    }

    #[derive(Pretty, Debug)]
    #[pprint(verbose, rename = "Inner")]
    pub struct InnerStrumct<'a> {
        x: &'a str,
        y: HeyEnum<'a>,
        z: (u128, usize, usize, usize),
    }

    #[derive(Pretty, Debug)]
    #[pprint(verbose)]
    pub struct Strumct<'a> {
        a: Vec<i32>,
        b: HashMap<String, HeyEnum<'a>>,
        c: InnerStrumct<'a>,

        #[pprint(ignore)]
        no: usize,
    }

    #[derive(Pretty, Debug)]
    #[pprint(verbose)]
    pub struct VecStruct<'a> {
        a: Vec<usize>,
        b: &'a str,
    }

    #[test]
    fn test_vec() {
        // let s = join(Doc::DoubleDoc(Doc::from(", ").into(), Doc::from(", ").into()), vec![
        //     Doc::from("a"),
        //     Doc::Hardline,
        //     Doc::Concat(vec![Doc::from(1), Doc::from(2), Doc::from(3)]),
        //     Doc::Hardline,
        //     Doc::from("b"),
        //     Doc::from("c"),
        // ])
        // .wrap(Doc::from("["), Doc::from("]"));
        let s = join(
            Doc::DoubleDoc(Doc::from(",").into(), Doc::Hardline.into()),
            vec![Doc::from(1), Doc::from(2), Doc::from(3)],
        )
        .wrap(Doc::from("["), Doc::from("]"));

        let pretty = pprint(s, None);
        println!("{}", pretty);
    }

    #[test]
    fn test_enum() {
        let s = HeyEnum::There("there");

        let pretty = pprint(s, None);
        println!("{}", pretty);
    }

    #[test]
    fn test_simple_struct() {
        let s = InnerStrumct {
            x: "hello",
            y: HeyEnum::There("there"),
            z: (u128::MAX, 2, 3, 4),
        };

        let pretty = pprint(s, None);
        println!("{}", pretty);
    }

    #[test]
    fn test_complex_struct() {
        let a = vec![
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 2, 3, 4, 5, 6, 7, 8,
            9, 10,
        ];
        let a: Vec<_> = a.into_iter().map(|x| x).collect();

        let mut b = HashMap::new();
        b.insert("hello".to_string(), HeyEnum::There("there"));
        b.insert("a".to_string(), HeyEnum::A);
        b.insert(
            "b".to_string(),
            HeyEnum::B(regex::Regex::new(".*").unwrap()),
        );

        let s = Strumct {
            a,
            b,
            c: InnerStrumct {
                x: "hello",
                y: HeyEnum::There("there"),
                z: (1, 2, 3, 4),
            },

            no: 0,
        };

        let pretty = pprint(s, None);
        println!("{}", pretty);
        // println!("{:#?}", s);
    }
}
