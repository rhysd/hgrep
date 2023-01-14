use std::io;

pub trait IgnoreBrokenPipe {
    fn ignore_broken_pipe(self) -> Self;
}

impl<T: Default> IgnoreBrokenPipe for io::Result<T> {
    fn ignore_broken_pipe(self) -> Self {
        self.or_else(|err| {
            if err.kind() == io::ErrorKind::BrokenPipe {
                Ok(T::default())
            } else {
                Err(err)
            }
        })
    }
}

impl<T: Default> IgnoreBrokenPipe for anyhow::Result<T> {
    fn ignore_broken_pipe(self) -> Self {
        self.or_else(|err| match err.downcast_ref::<io::Error>() {
            Some(err) if err.kind() == io::ErrorKind::BrokenPipe => Ok(T::default()),
            _ => Err(err),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Error;

    #[test]
    fn test_io_ignore_broken_pipe() {
        let err = Error::new(io::ErrorKind::BrokenPipe, "oops");
        let res = io::Result::<i32>::Err(err);
        let res = res.ignore_broken_pipe();
        assert_eq!(res.unwrap(), 0);
    }

    #[test]
    fn test_io_do_not_ignore_other_errors() {
        let err = Error::new(io::ErrorKind::Other, "oops");
        let res = io::Result::<i32>::Err(err);
        let res = res.ignore_broken_pipe();
        res.unwrap_err();
    }

    #[test]
    fn test_io_do_nothing_on_ok() {
        let res = io::Result::Ok(0i32);
        let res = res.ignore_broken_pipe();
        assert_eq!(res.unwrap(), 0);
    }
}
