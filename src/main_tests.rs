use crate::*;

#[cfg(test)]
mod fds_validate_test {
    use super::*;

    #[test]
    fn missing_fds() {
        let fds: Vec<&str> = vec!["a", "b"];
        assert!(do_validate_fds(&fds, false).is_err());
    }

    #[test]
    fn invalid_usage_of_plus() {
        let fds: Vec<&str> = vec!["a", "b", "+="];
        assert!(do_validate_fds(&fds, false).is_err());

        let fds: Vec<&str> = vec!["a", "b", "+-"];
        assert!(do_validate_fds(&fds, false).is_err());
    }

    #[test]
    fn invalid_usage_of_equal() {
        let fds: Vec<&str> = vec!["=", "b", "c"];
        assert!(do_validate_fds(&fds, false).is_err());
    }

    #[test]
    fn same_file_names() {
        let fds: Vec<&str> = vec!["a", "a", "b"];
        assert!(do_validate_fds(&fds, false).is_err());

        let fds: Vec<&str> = vec!["a", "b", "a"];
        assert!(do_validate_fds(&fds, false).is_err());

        let fds: Vec<&str> = vec!["a", "b", "b"];
        assert!(do_validate_fds(&fds, false).is_err());
    }

    #[test]
    fn force_overwrite() {
        let fds: Vec<&str> = vec!["a", "b", "c"];
        assert!(do_validate_fds(&fds, true).is_err());

        let fds: Vec<&str> = vec!["a", "=", "c"];
        assert!(do_validate_fds(&fds, true).is_ok());

        let fds: Vec<&str> = vec!["-", "=", "c"];
        assert!(do_validate_fds(&fds, true).is_err());
    }
}

#[cfg(test)]
mod main_tests {
    use super::*;

    #[test]
    fn parse_empty() {
        let argv: Vec<&str> = vec!["exec", "cmd"];
        let _err: anyhow::Error = Args::parse(&argv).unwrap_err();
    }

    #[test]
    fn parse_fds() {
        let argv: Vec<&str> = vec!["exec", "a", "b", "c", "cmd"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["a", "b", "c"],
            command_line: vec!["cmd"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: None,
        });
    }

    #[test]
    fn parse_omitted_fds() {
        let argv: Vec<&str> = vec!["exec", "a", "b", "--", "cmd"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["a", "b", "-"],
            command_line: vec!["cmd"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: None,
        });
    }

    #[test]
    fn parse_omitted_fds2() {
        let argv: Vec<&str> = vec!["exec", "a", "--", "cmd"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["a", "-", "-"],
            command_line: vec!["cmd"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: None,
        });
    }

    #[test]
    fn parse_omitted_fds3() {
        let argv: Vec<&str> = vec!["exec", "--", "cmd"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["-", "-", "-"],
            command_line: vec!["cmd"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: None,
        });
    }

    #[test]
    fn parse_shorthand_fds() {
        let argv: Vec<&str> = vec!["exec", "---", "cmd"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["-", "-", "-"],
            command_line: vec!["cmd"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: None,
        });
    }

    #[test]
    fn parse_including_tempdir() {
        let argv: Vec<&str> = vec!["exec", "---", "cat", "T/hoge.txt"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["-", "-", "-"],
            command_line: vec!["cat", "T/hoge.txt"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: None,
        });
    }

    #[test]
    fn parse_tempdir_option() {
        let argv: Vec<&str> = vec!["exec", "-t", "HOGE", "---", "cat", "HOGE/hoge.txt"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["-", "-", "-"],
            command_line: vec!["cat", "HOGE/hoge.txt"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: None,
            tempdir_placeholder: Some("HOGE"),
        });
    }

    #[test]
    fn parse_pipe_str_option() {
        let argv: Vec<&str> = vec!["exec", "--pipe", "%%", "---", "cat", "hoge.txt", "%%", "wc"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["-", "-", "-"],
            command_line: vec!["cat", "hoge.txt", "%%", "wc"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: Some("%%"),
            separator_str: None,
            tempdir_placeholder: None,
        });
    }

    #[test]
    fn parse_separator_str_option() {
        let argv: Vec<&str> = vec!["exec", "--separator", "%%", "---", "cat", "hoge.txt", "%%", "cat", "fuga.txt"];
        let a = Args::parse(&argv).unwrap();

        assert_eq!(a, Args { 
            fds: vec!["-", "-", "-"],
            command_line: vec!["cat", "hoge.txt", "%%", "cat", "fuga.txt"],
            force_overwrite: false,
            keep_going: false,
            envs: vec![],
            working_directory: None,
            debug_info: false,
            pipe_str: None,
            separator_str: Some("%%"),
            tempdir_placeholder: None,
        });
    }
}
