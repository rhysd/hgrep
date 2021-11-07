use std::io::{ErrorKind, Result};

pub trait IgnoreBrokenPipe {
    fn ignore_broken_pipe(self) -> Self;
}

impl<T: Default> IgnoreBrokenPipe for Result<T> {
    fn ignore_broken_pipe(self) -> Self {
        self.or_else(|err| {
            if err.kind() == ErrorKind::BrokenPipe {
                Ok(T::default())
            } else {
                Err(err)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Error;

    #[test]
    fn test_ignore_broken_pipe() {
        let err = Error::new(ErrorKind::BrokenPipe, "oops");
        let res = Result::<i32>::Err(err);
        let res = res.ignore_broken_pipe();
        assert_eq!(res.unwrap(), 0);
    }

    #[test]
    fn test_do_not_ignore_other_errors() {
        let err = Error::new(ErrorKind::Other, "oops");
        let res = Result::<i32>::Err(err);
        let res = res.ignore_broken_pipe();
        res.unwrap_err();
    }

    #[test]
    fn test_do_nothing_on_ok() {
        let res = Result::Ok(0i32);
        let res = res.ignore_broken_pipe();
        assert_eq!(res.unwrap(), 0);
    }
}
