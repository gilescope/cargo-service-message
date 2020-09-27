fn main() {
}


#[cfg(test)]
mod tests1 {
    #[test]
    fn passing_test() {
        assert!(true);
    }

    #[test]
    fn failing_test() {
        assert!(false);
    }

    mod subtests{
        #[test]
        fn a_sub_test() {
            assert!(true);
        }
    }
}


