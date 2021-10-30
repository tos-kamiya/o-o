#[derive(Debug, PartialEq)]
pub enum Arg<'a> {
    Value,
    Option(&'a str),
    Separator(&'a str),
}

macro_rules! some_pair {
    ( $v1:expr, $v2:expr ) => ( Some(($v1, $v2)) )
}

pub fn arg_parse<'a>(arguments: &'a [&str], index: usize) -> (Arg<'a>, Option<usize>, Option<(usize, &'a str)>) {
    let a = arguments[index];
    if a == "-" {
        (Arg::Value, None, some_pair!(1, a))
    } else if a == "--" {
        (Arg::Separator(a), Some(1), None)
    } else if a.starts_with("--") {
        if let Some(i) = a.find("=") {
            (Arg::Option(&a[..i]), None, some_pair!(1, &a[i+1..]))
        } else if index + 1 < arguments.len() {
            let a2 = arguments[index + 1];
            if a2 == "-" || ! a2.starts_with("-") {
                (Arg::Option(a), Some(1), some_pair!(2, a2))
            } else {
                (Arg::Option(a), Some(1), None)
            }
        } else {
            (Arg::Option(a), Some(1), None)
        }
    } else if a.starts_with("-") {
        if a.len() > 2 {
            (Arg::Option(&a[..2]), None, some_pair!(1, &a[2..]))
        } else if index + 1 < arguments.len() {
            let a2 = arguments[index + 1];
            if a2 == "-" || ! a2.starts_with("-") {
                (Arg::Option(a), Some(1), some_pair!(2, a2))
            } else {
                (Arg::Option(a), Some(1), None)
            }
        } else {
            (Arg::Option(a), Some(1), None)
        }
    } else {
        (Arg::Value, None, some_pair!(1, a))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    
    #[test]
    fn short_option_simple() {
        let args = vec!["-a", "1", "-f", "-g3"];
        let v = arg_parse(&args, 0);
        assert_eq!(v, (Arg::Option("-a"), Some(1), some_pair!(2, "1")));
        let v = arg_parse(&args, 1);
        // eprintln!("{:?}", v);
        assert_eq!(v, (Arg::Value, None, some_pair!(1, "1")));
        let v = arg_parse(&args, 2);
        assert_eq!(v, (Arg::Option("-f"), Some(1), None));
        let v = arg_parse(&args, 3);
        assert_eq!(v, (Arg::Option("-g"), None, some_pair!(1, "3")));
    }
    
    #[test]
    fn short_option_complicated() {
        let args = vec!["-a=1", "-f", "-", "-g", "--", "-h"];
        let v = arg_parse(&args, 0);
        assert_eq!(v, (Arg::Option("-a"), None, some_pair!(1, "=1")));
        let v = arg_parse(&args, 1);
        assert_eq!(v, (Arg::Option("-f"), Some(1), some_pair!(2, "-")));
        let v = arg_parse(&args, 2);
        assert_eq!(v, (Arg::Value, None, some_pair!(1, "-")));
        let v = arg_parse(&args, 3);
        assert_eq!(v, (Arg::Option("-g"), Some(1), None));
        let v = arg_parse(&args, 4);
        assert_eq!(v, (Arg::Separator("--"), Some(1), None));
        let v = arg_parse(&args, 5);
        assert_eq!(v, (Arg::Option("-h"), Some(1), None));
    }
    
    #[test]
    fn long_option_simple() {
        let args = vec!["--aa", "1", "--ff", "--gg=3"];
        let v = arg_parse(&args, 0);
        assert_eq!(v, (Arg::Option("--aa"), Some(1), some_pair!(2, "1")));
        let v = arg_parse(&args, 1);
        // eprintln!("{:?}", v);
        assert_eq!(v, (Arg::Value, None, some_pair!(1, "1")));
        let v = arg_parse(&args, 2);
        assert_eq!(v, (Arg::Option("--ff"), Some(1), None));
        let v = arg_parse(&args, 3);
        assert_eq!(v, (Arg::Option("--gg"), None, some_pair!(1, "3")));
    }
    
    #[test]
    fn match_test() {
        let args = vec!["--aa", "1", "--bb=3"];
        let v = arg_parse(&args, 0);
        match v {
            (Arg::Option("--aa"), Some(1), Some((2, "1"))) => { }
            _ => { panic!("match fails.") }
        }
        match v {
            (Arg::Option("--aa"), _, Some((eat, "1"))) => { 
                assert_eq!(eat, 2 as usize);
            }
            _ => { panic!("match fails.") }
        }
    
        let v = arg_parse(&args, 2);
        match v {
            (Arg::Option("--bb"), None, Some((eat, value))) => { 
                assert_eq!(eat, 1 as usize);
                assert_eq!(value, "3");
            }
            _ => { panic!("match fails.") }
        }
    }
}
