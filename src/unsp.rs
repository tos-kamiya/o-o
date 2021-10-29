use std::convert::AsRef;

#[derive(Debug, PartialEq)]
pub enum Arg {
    Value(String),
    ValueOption(String, String),
    FlagOption(String),
    Separator(String),
}

impl Arg {
    pub fn value<S: Into<String>>(v: S) -> Self {
        Arg::Value(v.into())
    }
    pub fn value_option<S: Into<String>, T: Into<String>>(n: S, v: T) -> Self {
        Arg::ValueOption(n.into(), v.into())
    }
    pub fn flag_option<S: Into<String>>(n: S) -> Self {
        Arg::FlagOption(n.into())
    }
    pub fn separator<S: Into<String>>(s: S) -> Self {
        Arg::Separator(s.into())
    }
}

pub fn parse<T: AsRef<str>>(arguments: &[T], index: usize) -> Vec<(Arg, usize)> {
    let a: &str = arguments[index].as_ref();
    if a == "-" {
        vec![
            (Arg::value(a), 1),
        ]
    } else if a == "--" {
        vec![
            (Arg::separator(a), 1),
        ]
    } else if a.starts_with("--") {
        if let Some(i) = a.find("=") {
            let name: &str = a[..i].as_ref();
            let value: &str = a[i+1..].as_ref();
            vec![
                (Arg::value_option(name, value), 1),
            ]
        } else if index + 1 < arguments.len() {
            let a2: &str = arguments[index + 1].as_ref();
            if a2 == "-" || ! a2.starts_with("-") {
                vec![
                    (Arg::value_option(a, a2), 2),
                    (Arg::flag_option(a), 1),
                ]
            } else {
                vec![
                    (Arg::flag_option(a), 1),
                ]
            }
        } else {
            vec![
                (Arg::flag_option(a), 1),
            ]
        }
    } else if a.starts_with("-") {
        if a.len() > 2 {
            let name: &str = a[..2].as_ref();
            let value: &str = a[2..].as_ref();
            vec![
                (Arg::value_option(name, value), 1),
            ]
        } else if index + 1 < arguments.len() {
            let a2: &str = arguments[index + 1].as_ref();
            if a2 == "-" || ! a2.starts_with("-") {
                vec![
                    (Arg::value_option(a, a2), 2),
                    (Arg::flag_option(a), 1),
                ]
            } else {
                vec![
                    (Arg::flag_option(a), 1),
                ]
            }
        } else {
            vec![
                (Arg::flag_option(a), 1),
            ]
        }
    } else {
        vec![
            (Arg::value(a), 1),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_option_simple() {
        let args = vec!["-a", "1", "-f", "-g3"];
        let v = parse(&args, 0);
        assert_eq!(v, vec![
            (Arg::value_option("-a", "1"), 2 as usize),
            (Arg::flag_option("-a"), 1 as usize),
        ]);
        let v = parse(&args, 1);
        // eprintln!("{:?}", v);
        assert_eq!(v, vec![
            (Arg::value("1"), 1 as usize),
        ]);
        let v = parse(&args, 2);
        assert_eq!(v, vec![
            (Arg::flag_option("-f"), 1 as usize),
        ]);
        let v = parse(&args, 3);
        assert_eq!(v, vec![
            (Arg::value_option("-g", "3"), 1 as usize),
        ]);
    }

    #[test]
    fn short_option_complicated() {
        let args = vec!["-a=1", "-f", "-", "-g", "--", "-h"];
        let v = parse(&args, 0);
        assert_eq!(v, vec![
            (Arg::value_option("-a", "=1"), 1 as usize),
        ]);
        let v = parse(&args, 1);
        assert_eq!(v, vec![
            (Arg::value_option("-f", "-"), 2 as usize),
            (Arg::flag_option("-f"), 1 as usize),
        ]);
        let v = parse(&args, 2);
        assert_eq!(v, vec![
            (Arg::value("-"), 1 as usize),
        ]);
        let v = parse(&args, 3);
        assert_eq!(v, vec![
            (Arg::flag_option("-g"), 1 as usize),
        ]);
        let v = parse(&args, 4);
        assert_eq!(v, vec![
            (Arg::separator("--"), 1 as usize),
        ]);
        let v = parse(&args, 5);
        assert_eq!(v, vec![
            (Arg::flag_option("-h"), 1 as usize),
        ]);
    }

    #[test]
    fn long_option_simple() {
        let args = vec!["--aa", "1", "--ff", "--gg=3"];
        let v = parse(&args, 0);
        assert_eq!(v, vec![
            (Arg::value_option("--aa", "1"), 2 as usize),
            (Arg::flag_option("--aa"), 1 as usize),
        ]);
        let v = parse(&args, 1);
        // eprintln!("{:?}", v);
        assert_eq!(v, vec![
            (Arg::value("1"), 1 as usize),
        ]);
        let v = parse(&args, 2);
        assert_eq!(v, vec![
            (Arg::flag_option("--ff"), 1 as usize),
        ]);
        let v = parse(&args, 3);
        assert_eq!(v, vec![
            (Arg::value_option("--gg", "3"), 1 as usize),
        ]);
    }
}
