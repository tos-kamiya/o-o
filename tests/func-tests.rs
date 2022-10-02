#[cfg(test)]
#[allow(non_snake_case)]

#[cfg(test)]
mod func_tests {
    use o_o::*;

    #[test]
    fn command_exists_for_ls() {
        let ls_command_exists = command_exists("ls");
        assert!(ls_command_exists);
    }

    #[test]
    fn command_exists_for_hoge4() {
        let h4_command_exists = command_exists("hoge-hoge-hoge-hoge");
        assert!(!h4_command_exists);
    }
}
